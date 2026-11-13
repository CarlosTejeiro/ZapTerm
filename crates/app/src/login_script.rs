//! Automated expect/send login scripts.
//!
//! A [`LoginRunner`] watches the raw output of a session and, when it spots
//! the current step's `expect` pattern, sends the corresponding bytes via the
//! session's [`SessionCmd::Data`] command channel. It then arms the next step
//! with its own timeout. The matcher strips ANSI escape sequences before
//! searching so coloured prompts don't break literal patterns.
//!
//! The runner is fed synchronously from inside `make_on_data` (see
//! `commands/sessions.rs`), which already lives in the per-session I/O task.
//! A small mutex guards the buffer + step index; matching is a memmem-style
//! substring scan, so the data callback stays fast.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use core_transport::SessionCmd;

/// What a macro step does. Older saved scripts have no `kind` field and default
/// to [`StepKind::Expect`] — the original wait-then-send behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    /// Wait for `expect` in the output, then send `send`. `timeout_ms` bounds
    /// the wait (script aborts if it never matches).
    #[default]
    Expect,
    /// Send `send` immediately, without waiting for any output.
    Send,
    /// Pause for `timeout_ms` milliseconds, then continue. No output/send.
    Wait,
}

/// One step of a login/macro script.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoginStep {
    /// Step type. Defaults to `expect` for backwards compatibility.
    #[serde(default)]
    pub kind: StepKind,
    /// Pattern to wait for in the (ANSI-stripped) output stream (`expect` only).
    pub expect: String,
    /// If true, `expect` is a regex (matched against the ANSI-stripped output).
    /// An invalid regex falls back to literal matching with a warning.
    #[serde(default)]
    pub is_regex: bool,
    /// Bytes to send (interpreted as UTF-8). Used by `expect` and `send` steps;
    /// a trailing Enter is appended automatically when missing.
    pub send: String,
    /// For `expect`, how long to wait before giving up; for `wait`, the pause
    /// duration. Milliseconds. Ignored by `send`.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    10_000
}

/// Decode C-style backslash escapes in a `send` payload into raw bytes:
/// `\r` `\n` `\t` `\b` (backspace) `\e` (ESC) `\0` (NUL) `\\` and `\xHH`.
/// An unknown or malformed escape is kept verbatim (backslash included), so
/// ordinary text with stray backslashes is never silently dropped. This lets a
/// step typed as `cmd\r` press Enter instead of sending the two characters.
fn decode_escapes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut buf = [0u8; 4];
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('b') => out.push(0x08),
            Some('e') => out.push(0x1b),
            Some('0') => out.push(0x00),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let h1 = chars.peek().copied().filter(char::is_ascii_hexdigit);
                let hex = h1.and_then(|a| {
                    chars.next();
                    let b = chars.peek().copied().filter(char::is_ascii_hexdigit)?;
                    chars.next();
                    Some((a, b))
                });
                match hex {
                    Some((a, b)) => {
                        // Both are validated hex digits.
                        let v = (a.to_digit(16).unwrap() * 16 + b.to_digit(16).unwrap()) as u8;
                        out.push(v);
                    }
                    None => out.extend_from_slice(b"\\x"),
                }
            }
            Some(other) => {
                out.push(b'\\');
                out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
            None => out.push(b'\\'),
        }
    }
    out
}

/// Status broadcast to the frontend so the terminal toolbar can show progress.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoginProgress {
    pub session_id: String,
    /// 1-based index of the step just executed (`0` means "still running first"); equal to `total` when complete.
    pub current: usize,
    pub total: usize,
    /// `"running"`, `"complete"`, or `"timeout"`.
    pub status: &'static str,
}

const MAX_BUF: usize = 64 * 1024;

/// Precompiled matcher for one step's `expect`. Built once at construction so
/// the per-chunk data callback never recompiles a regex.
enum Matcher {
    /// Empty pattern — never matches (step is effectively a no-op wait).
    Empty,
    /// Literal substring (ANSI already stripped before searching).
    Literal(String),
    Regex(regex::Regex),
}

impl Matcher {
    fn build(expect: &str, is_regex: bool, session_id: &str) -> Self {
        if expect.is_empty() {
            return Matcher::Empty;
        }
        if is_regex {
            match regex::Regex::new(expect) {
                Ok(re) => return Matcher::Regex(re),
                Err(e) => tracing::warn!(
                    session_id = %session_id,
                    "login script: invalid regex {expect:?} ({e}); falling back to literal"
                ),
            }
        }
        Matcher::Literal(expect.to_owned())
    }

    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Matcher::Empty => false,
            Matcher::Literal(p) => haystack.contains(p.as_str()),
            Matcher::Regex(re) => re.is_match(haystack),
        }
    }
}

pub struct LoginRunner {
    steps: Vec<LoginStep>,
    matchers: Vec<Matcher>,
    cmd_tx: UnboundedSender<SessionCmd>,
    inner: Mutex<RunnerInner>,
    on_progress: Box<dyn Fn(LoginProgress) + Send + Sync>,
    session_id: String,
}

struct RunnerInner {
    index: usize,
    buffer: Vec<u8>,
    deadline: Instant,
}

impl LoginRunner {
    pub fn new(
        session_id: String,
        steps: Vec<LoginStep>,
        cmd_tx: UnboundedSender<SessionCmd>,
        on_progress: Box<dyn Fn(LoginProgress) + Send + Sync>,
    ) -> Self {
        let matchers = steps
            .iter()
            .map(|s| Matcher::build(&s.expect, s.is_regex, &session_id))
            .collect();
        let runner = Self {
            steps,
            matchers,
            cmd_tx,
            session_id,
            on_progress,
            inner: Mutex::new(RunnerInner {
                index: 0,
                buffer: Vec::with_capacity(8192),
                deadline: Instant::now(),
            }),
        };
        // Arm the first actionable step. This fires any leading `send` steps
        // immediately and parks on the first `expect`/`wait` (or completes).
        {
            let mut inner = runner.inner.lock().unwrap();
            runner.arm(&mut inner);
        }
        runner
    }

    /// Advance from the current index, firing consecutive `send` steps right
    /// away, until we hit an `expect`/`wait` step (whose deadline we arm) or run
    /// out of steps (complete). Emits the matching progress event.
    fn arm(&self, inner: &mut RunnerInner) {
        loop {
            if inner.index >= self.steps.len() {
                self.emit(inner.index, "complete");
                return;
            }
            let step = &self.steps[inner.index];
            match step.kind {
                StepKind::Send => {
                    self.fire_send(step);
                    inner.index += 1;
                    // Loop on to the next step.
                }
                StepKind::Expect | StepKind::Wait => {
                    inner.deadline = Instant::now() + Duration::from_millis(step.timeout_ms);
                    inner.buffer.clear();
                    self.emit(inner.index, "running");
                    return;
                }
            }
        }
    }

    /// Send a step's `send` payload, appending a trailing Enter when the user
    /// didn't end the line themselves (so `enable` actually executes). Backslash
    /// escapes (`\r`, `\n`, `\t`, …) are decoded first, so a step written as
    /// `cmd\r` presses Enter instead of typing the two literal characters.
    /// Empty payloads send nothing.
    fn fire_send(&self, step: &LoginStep) {
        let mut bytes = decode_escapes(&step.send);
        if !bytes.is_empty() && !bytes.ends_with(b"\n") && !bytes.ends_with(b"\r") {
            bytes.push(b'\r');
        }
        if !bytes.is_empty() {
            let _ = self.cmd_tx.send(SessionCmd::Data(bytes));
        }
    }

    fn emit(&self, current: usize, status: &'static str) {
        (self.on_progress)(LoginProgress {
            session_id: self.session_id.clone(),
            current,
            total: self.steps.len(),
            status,
        });
    }

    /// Timer tick from the watchdog: release a `wait` whose pause elapsed, or
    /// abort an `expect` that never matched in time. Drives `wait` steps and
    /// silent-server timeouts (which `feed` can't, since it only runs on
    /// output). Returns `true` once the script has finished.
    pub fn check_timeout(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.index >= self.steps.len() {
            return true;
        }
        if Instant::now() <= inner.deadline {
            return false;
        }
        match self.steps[inner.index].kind {
            StepKind::Wait => {
                // Pause elapsed → move on (firing any following `send` steps).
                inner.index += 1;
                self.arm(&mut inner);
            }
            _ => {
                tracing::warn!(
                    session_id = %self.session_id,
                    step = inner.index,
                    "login script: step timed out"
                );
                let current = inner.index;
                inner.index = self.steps.len();
                self.emit(current, "timeout");
            }
        }
        inner.index >= self.steps.len()
    }

    /// Feed a chunk of session output. Only `expect` steps consume it; `wait`
    /// is timer-driven and `send` already fired during [`arm`](Self::arm).
    pub fn feed(&self, data: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        if inner.index >= self.steps.len() {
            return;
        }
        if self.steps[inner.index].kind != StepKind::Expect {
            return;
        }
        // A silent stretch may have blown the expect deadline before this chunk.
        if Instant::now() > inner.deadline {
            tracing::warn!(
                session_id = %self.session_id,
                step = inner.index,
                "login script: step timed out"
            );
            let current = inner.index;
            inner.index = self.steps.len();
            self.emit(current, "timeout");
            return;
        }

        // Append + cap the buffer so a server flood can't make the matcher
        // walk megabytes per chunk.
        inner.buffer.extend_from_slice(data);
        if inner.buffer.len() > MAX_BUF {
            let drop = inner.buffer.len() - MAX_BUF;
            inner.buffer.drain(0..drop);
        }

        let stripped = strip_ansi(&inner.buffer);
        if self.matchers[inner.index].is_match(&stripped) {
            self.fire_send(&self.steps[inner.index]);
            inner.index += 1;
            self.arm(&mut inner);
        }
    }
}

/// Strip ANSI CSI / OSC escape sequences so literal patterns match coloured
/// prompts. Returns a lossy UTF-8 string suitable for `String::contains`.
pub(crate) fn strip_ansi(bytes: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1b && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'[' {
                // CSI: ESC '[' <params> <final 0x40-0x7e>
                i += 2;
                while i < bytes.len() && !(bytes[i] >= 0x40 && bytes[i] <= 0x7e) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
            if next == b']' {
                // OSC: ESC ']' ... (BEL | ESC '\')
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            // ESC + single byte (unsupported / 2-byte sequence) — drop both.
            i += 2;
            continue;
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_basics() {
        let s = strip_ansi(b"\x1b[32mhello\x1b[0m world");
        assert_eq!(s, "hello world");
    }

    #[test]
    fn strip_ansi_osc() {
        let s = strip_ansi(b"\x1b]0;title\x07Plain text");
        assert_eq!(s, "Plain text");
    }

    #[test]
    fn matcher_literal_and_regex() {
        assert!(Matcher::build("Password:", false, "s").is_match("Enter Password: "));
        assert!(!Matcher::build("Password:", false, "s").is_match("login:"));
        // Regex: a prompt ending in '#' or '>' (typical network device).
        let re = Matcher::build(r"[\w.-]+[#>]\s*$", true, "s");
        assert!(re.is_match("router1# "));
        assert!(re.is_match("sw-core>"));
        assert!(!re.is_match("Password:"));
        // Invalid regex falls back to literal (matches the raw pattern text).
        let bad = Matcher::build("(unclosed", true, "s");
        assert!(bad.is_match("see (unclosed here"));
    }

    fn step(kind: StepKind, expect: &str, send: &str, timeout_ms: u64) -> LoginStep {
        LoginStep {
            kind,
            expect: expect.into(),
            is_regex: false,
            send: send.into(),
            timeout_ms,
        }
    }

    /// Drain the bytes the runner pushed to the session command channel.
    fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<SessionCmd>) -> Vec<u8> {
        let mut out = Vec::new();
        while let Ok(SessionCmd::Data(d)) = rx.try_recv() {
            out.extend_from_slice(&d);
        }
        out
    }

    #[test]
    fn silent_server_times_out_via_check_timeout() {
        use std::sync::{Arc, Mutex as StdMutex};
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let statuses: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&statuses);
        let steps = vec![step(StepKind::Expect, "prompt-that-never-comes", "x", 1)];
        let runner = LoginRunner::new(
            "s".into(),
            steps,
            tx,
            Box::new(move |p| sink.lock().unwrap().push(p.status.to_string())),
        );
        // No data is ever fed. Past the 1 ms deadline, the watchdog's
        // check_timeout must still expire the step.
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            runner.check_timeout(),
            "script should be finished after timeout"
        );
        let s = statuses.lock().unwrap();
        assert_eq!(s.first().map(String::as_str), Some("running"));
        assert!(s.iter().any(|x| x == "timeout"), "expected a timeout event");
    }

    #[test]
    fn send_steps_fire_immediately_in_order() {
        // Two `send` steps with no expect: both go out the moment the runner is
        // built, in order, each with a trailing Enter.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let runner = LoginRunner::new(
            "s".into(),
            vec![
                step(StepKind::Send, "", "term len 0", 0),
                step(StepKind::Send, "", "show ver", 0),
            ],
            tx,
            Box::new(|_| {}),
        );
        assert_eq!(drain(&mut rx), b"term len 0\rshow ver\r");
        assert!(
            runner.check_timeout(),
            "all-send script finishes immediately"
        );
    }

    #[test]
    fn wait_step_advances_after_its_delay() {
        // A `wait` step pauses, then the following `send` fires once elapsed.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let runner = LoginRunner::new(
            "s".into(),
            vec![
                step(StepKind::Wait, "", "", 1),
                step(StepKind::Send, "", "go", 0),
            ],
            tx,
            Box::new(|_| {}),
        );
        // Before the delay elapses, nothing sent and not finished.
        assert!(drain(&mut rx).is_empty());
        assert!(!runner.check_timeout());
        std::thread::sleep(Duration::from_millis(5));
        assert!(runner.check_timeout(), "finished after the wait + send");
        assert_eq!(drain(&mut rx), b"go\r");
    }

    #[test]
    fn expect_then_send_chain() {
        // Classic: wait for a prompt, then send. Feeding the prompt advances and
        // fires the send (with auto-Enter).
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let runner = LoginRunner::new(
            "s".into(),
            vec![step(StepKind::Expect, "Password:", "hunter2", 1000)],
            tx,
            Box::new(|_| {}),
        );
        assert!(drain(&mut rx).is_empty(), "nothing sent before the match");
        runner.feed(b"\r\nEnter Password: ");
        assert_eq!(drain(&mut rx), b"hunter2\r");
        assert!(runner.check_timeout(), "finished after the expect matched");
    }

    #[test]
    fn send_decodes_backslash_escapes() {
        // A send written with `\r` presses Enter (one CR, not the literal two
        // chars and not a doubled CR); other escapes decode too; stray
        // backslashes survive.
        assert_eq!(decode_escapes("ssh host\\r"), b"ssh host\r");
        assert_eq!(decode_escapes("a\\tb"), b"a\tb");
        assert_eq!(decode_escapes("\\x1b[A"), b"\x1b[A");
        assert_eq!(decode_escapes("C:\\\\tmp"), b"C:\\tmp");
        assert_eq!(decode_escapes("no escapes"), b"no escapes");
    }

    #[test]
    fn send_with_explicit_cr_is_not_doubled() {
        // `cmd\r` → exactly one trailing CR (fire_send must not append a second).
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let runner = LoginRunner::new(
            "s".into(),
            vec![step(StepKind::Send, "", "cmd\\r", 0)],
            tx,
            Box::new(|_| {}),
        );
        assert_eq!(drain(&mut rx), b"cmd\r");
        assert!(runner.check_timeout());
    }
}
