//! Output triggers: fire an action when a regex/literal matches a line of the
//! session's output. The closest cousin is [`crate::login_script`] — but where
//! a login script walks an ordered set of steps once, triggers are an unordered
//! set that stay armed for the whole session and may fire repeatedly.
//!
//! A [`TriggerRunner`] is fed the raw output from inside `make_on_data` (the
//! per-session I/O task), exactly like the login runner. Matching is **per
//! line**: raw bytes are accumulated and split on `\n`, each completed line is
//! ANSI-stripped and tested against every enabled trigger. The pending,
//! not-yet-terminated line is tested too after every chunk — a prompt such as
//! `Continue? (y/n)` waits for its answer without ever ending its line — and a
//! trigger that fired on it doesn't fire again when that line completes. A small per-trigger
//! cooldown stops a "send" action from melting a device in a tight loop (the
//! sent text could itself match the trigger).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use core_transport::SessionCmd;

use crate::login_script::strip_ansi;

/// One output trigger (stored as JSON on the saved session).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Trigger {
    /// Pattern tested against each ANSI-stripped output line.
    pub pattern: String,
    /// If true, `pattern` is a regex (invalid → falls back to literal).
    #[serde(default)]
    pub is_regex: bool,
    /// `"notify"` (toast), `"send"` (type text + Enter back), or `"bell"`.
    pub action: String,
    /// Payload: the text to send (`send`) or a custom toast message
    /// (`notify`; empty = show the matched line).
    #[serde(default)]
    pub text: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Event emitted to the frontend when a `notify`/`bell` trigger fires.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TriggerFired {
    pub session_id: String,
    /// `"notify"` or `"bell"`.
    pub kind: String,
    /// Message to surface (custom text or the matched line).
    pub text: String,
}

const MAX_LINE: usize = 8 * 1024;
/// Minimum gap between two fires of the SAME trigger — a cheap loop breaker.
const COOLDOWN: Duration = Duration::from_millis(300);

enum Matcher {
    Literal(String),
    Regex(regex::Regex),
}

impl Matcher {
    fn build(pattern: &str, is_regex: bool) -> Self {
        if is_regex {
            if let Ok(re) = regex::Regex::new(pattern) {
                return Matcher::Regex(re);
            }
            tracing::warn!("trigger: invalid regex {pattern:?}; falling back to literal");
        }
        Matcher::Literal(pattern.to_owned())
    }

    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Matcher::Literal(p) => !p.is_empty() && haystack.contains(p.as_str()),
            Matcher::Regex(re) => re.is_match(haystack),
        }
    }
}

pub struct TriggerRunner {
    triggers: Vec<Trigger>,
    matchers: Vec<Matcher>,
    cmd_tx: UnboundedSender<SessionCmd>,
    session_id: String,
    on_fire: Box<dyn Fn(TriggerFired) + Send + Sync>,
    inner: Mutex<Inner>,
}

struct Inner {
    /// Raw, not-yet-newline-terminated tail of the output.
    line: Vec<u8>,
    last_fired: Vec<Option<Instant>>,
    /// Triggers that already fired on the pending (unterminated) line.
    fired_on_pending: Vec<bool>,
    /// Where the pending line's current segment starts: just after its last
    /// bare `\r`. A carriage return sends the cursor back to column 0, so the
    /// text after it is a new line as far as the screen goes (a pager redraws
    /// `--More--` that way, never sending `\n`).
    pending_from: usize,
}

impl TriggerRunner {
    pub fn new(
        session_id: String,
        triggers: Vec<Trigger>,
        cmd_tx: UnboundedSender<SessionCmd>,
        on_fire: Box<dyn Fn(TriggerFired) + Send + Sync>,
    ) -> Self {
        let matchers = triggers
            .iter()
            .map(|t| Matcher::build(&t.pattern, t.is_regex))
            .collect();
        let n = triggers.len();
        Self {
            triggers,
            matchers,
            cmd_tx,
            session_id,
            on_fire,
            inner: Mutex::new(Inner {
                line: Vec::with_capacity(256),
                last_fired: vec![None; n],
                fired_on_pending: vec![false; n],
                pending_from: 0,
            }),
        }
    }

    /// Feed a chunk of raw session output. Splits into complete lines and tests
    /// each against every enabled trigger.
    pub fn feed(&self, data: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        inner.line.extend_from_slice(data);
        // Escape sequences never contain '\n', so splitting raw on '\n' is safe;
        // we ANSI-strip each line before matching.
        while let Some(pos) = inner.line.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = inner.line.drain(..=pos).collect();
            let stripped = strip_ansi(&raw);
            let line = stripped.trim_end_matches(['\r', '\n']);
            self.eval_line(line, &mut inner, false);
            // The line that was pending is complete now; the next one is new.
            inner.fired_on_pending.iter_mut().for_each(|f| *f = false);
            inner.pending_from = 0;
        }
        if inner.line.len() > MAX_LINE {
            let drop = inner.line.len() - MAX_LINE;
            inner.line.drain(0..drop);
            inner.pending_from = inner.pending_from.saturating_sub(drop);
        }
        // A bare `\r` (not the one ending the data so far, which may be the
        // first half of `\r\n`) starts a new segment of the pending line.
        let body = inner.line.strip_suffix(b"\r").unwrap_or(&inner.line);
        if let Some(cr) = body.iter().rposition(|&b| b == b'\r') {
            if cr + 1 > inner.pending_from {
                inner.pending_from = cr + 1;
                inner.fired_on_pending.iter_mut().for_each(|f| *f = false);
            }
        }
        if inner.line.len() > inner.pending_from {
            let stripped = strip_ansi(&inner.line[inner.pending_from..]);
            let pending = stripped.trim_end_matches('\r').to_owned();
            self.eval_line(&pending, &mut inner, true);
        }
    }

    /// Fire every enabled trigger matching `line`. `pending`: the line isn't
    /// terminated yet; a trigger fires at most once per such line (it already
    /// did if it is marked in `fired_on_pending`, which also covers the moment
    /// the line completes).
    fn eval_line(&self, line: &str, inner: &mut Inner, pending: bool) {
        let now = Instant::now();
        for (i, trig) in self.triggers.iter().enumerate() {
            if !trig.enabled || inner.fired_on_pending[i] || !self.matchers[i].is_match(line) {
                continue;
            }
            if let Some(last) = inner.last_fired[i] {
                if now.duration_since(last) < COOLDOWN {
                    continue;
                }
            }
            if pending {
                inner.fired_on_pending[i] = true;
            }
            inner.last_fired[i] = Some(now);
            self.fire(trig, line);
        }
    }

    fn fire(&self, trig: &Trigger, line: &str) {
        match trig.action.as_str() {
            "send" => {
                // Type the text followed by Enter (CR — what a terminal sends).
                let mut bytes = trig.text.clone().into_bytes();
                bytes.push(b'\r');
                let _ = self.cmd_tx.send(SessionCmd::Data(bytes));
            }
            "bell" => {
                (self.on_fire)(TriggerFired {
                    session_id: self.session_id.clone(),
                    kind: "bell".into(),
                    text: if trig.text.is_empty() {
                        line.to_owned()
                    } else {
                        trig.text.clone()
                    },
                });
            }
            // "notify" and anything unknown → toast.
            _ => {
                (self.on_fire)(TriggerFired {
                    session_id: self.session_id.clone(),
                    kind: "notify".into(),
                    text: if trig.text.is_empty() {
                        line.to_owned()
                    } else {
                        trig.text.clone()
                    },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    fn runner(triggers: Vec<Trigger>) -> (TriggerRunner, Arc<StdMutex<Vec<TriggerFired>>>) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let fired = Arc::new(StdMutex::new(Vec::new()));
        let f2 = Arc::clone(&fired);
        let r = TriggerRunner::new(
            "s".into(),
            triggers,
            tx,
            Box::new(move |e| f2.lock().unwrap().push(e)),
        );
        (r, fired)
    }

    fn trig(pattern: &str, is_regex: bool, action: &str, text: &str) -> Trigger {
        Trigger {
            pattern: pattern.into(),
            is_regex,
            action: action.into(),
            text: text.into(),
            enabled: true,
        }
    }

    #[test]
    fn notify_fires_on_matching_line() {
        let (r, fired) = runner(vec![trig("ERROR", false, "notify", "")]);
        r.feed(b"all good\r\nan ERROR happened\r\nstill ok\r\n");
        let f = fired.lock().unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "notify");
        assert!(f[0].text.contains("ERROR"));
    }

    #[test]
    fn regex_and_ansi_stripping() {
        let (r, fired) = runner(vec![trig(r"%\w+-\d", true, "notify", "alarm")]);
        // coloured line with a Cisco-style facility code
        r.feed(b"\x1b[31m%LINK-3-UPDOWN: down\x1b[0m\r\n");
        let f = fired.lock().unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].text, "alarm");
    }

    #[test]
    fn disabled_trigger_does_not_fire() {
        let mut t = trig("ERROR", false, "notify", "");
        t.enabled = false;
        let (r, fired) = runner(vec![t]);
        r.feed(b"ERROR here\r\n");
        assert_eq!(fired.lock().unwrap().len(), 0);
    }

    /// A prompt waits for an answer without ending its line: a `send`
    /// trigger on it must fire on the pending text, once, and not again when
    /// the line is finally completed.
    #[test]
    fn send_trigger_answers_a_prompt_without_newline() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let r = TriggerRunner::new(
            "s".into(),
            vec![trig("(y/n)", false, "send", "y")],
            tx,
            Box::new(|_| {}),
        );
        r.feed(b"Do you want to continue? ");
        assert!(rx.try_recv().is_err(), "not matched yet");
        r.feed(b"(y/n) ");
        match rx.try_recv() {
            Ok(SessionCmd::Data(d)) => assert_eq!(d, b"y\r"),
            other => panic!("expected the answer, got {other:?}"),
        }
        // The device echoes the answer and ends the line: no second send.
        r.feed(b"y\r\n");
        assert!(rx.try_recv().is_err(), "fired twice for one prompt");
        // The next prompt fires again.
        std::thread::sleep(COOLDOWN);
        r.feed(b"Overwrite? (y/n) ");
        assert!(rx.try_recv().is_ok());
    }

    /// A pager erases its prompt with `\r` and keeps printing on the same
    /// line: every `--More--` is answered, not only the first.
    #[test]
    fn pager_prompt_redrawn_after_cr_fires_again() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let r = TriggerRunner::new(
            "s".into(),
            vec![trig("--More--", false, "send", " ")],
            tx,
            Box::new(|_| {}),
        );
        r.feed(b"line 1\r\n--More-- ");
        assert!(rx.try_recv().is_ok(), "first page");
        std::thread::sleep(COOLDOWN);
        r.feed(b"\r        \rline 2\r\nline 3\r\n--More-- ");
        assert!(rx.try_recv().is_ok(), "second page");
        std::thread::sleep(COOLDOWN);
        r.feed(b"\r        \r--More-- ");
        assert!(rx.try_recv().is_ok(), "redrawn on the same line");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn partial_line_waits_for_newline() {
        let (r, fired) = runner(vec![trig("done", false, "notify", "")]);
        r.feed(b"not yet ");
        assert_eq!(fired.lock().unwrap().len(), 0);
        r.feed(b"done\r\n");
        assert_eq!(fired.lock().unwrap().len(), 1);
    }
}
