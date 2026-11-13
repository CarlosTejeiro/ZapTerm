//! Python scripting: run a user's script with the bundled `zapterm` module and
//! relay its API calls to the frontend, which carries them out on real tabs.
//!
//! ```text
//!  script.py ──JSON lines over 127.0.0.1──▶ ScriptRun ──ScriptSink::request──▶ frontend
//!            ◀──────────── reply ────────── ScriptRun::reply ◀── script_reply ─┘
//! ```
//!
//! The socket (not stdin/stdout) carries the API so the script's own `print`
//! output streams unchanged to the panel. The listener only accepts a client
//! that opens with the run's random token, and only one, for the lifetime of
//! the run. This module knows nothing about Tauri: [`ScriptSink`] is the seam
//! the command layer implements and tests replace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

/// The `zapterm` module shipped to every script (written next to it on start).
pub const ZAPTERM_MODULE: &str = include_str!("zapterm.py");

/// `zapx.py`: scripts written for ZAPX (`import zapx`) keep working — the
/// name is an alias of the `zapterm` module.
pub const ZAPX_ALIAS_MODULE: &str = "\"\"\"ZAPX name of the ZapTerm scripting API: `import zapx` is `import zapterm`.\"\"\"\nimport sys\n\nimport zapterm\n\nsys.modules[__name__] = zapterm\n";

/// How long a started script has to connect before its API calls are refused
/// (a script that never imports `zapterm` still runs; it just can't call it).
const ACCEPT_WINDOW: Duration = Duration::from_secs(3600);

/// Sent to a client once its token is accepted (it waits for this line).
const HELLO_OK: &str = "{\"hello\":true}\n";

/// Where the run's events go. Implemented by the Tauri layer (events to the
/// frontend) and by tests (answers in-process).
pub trait ScriptSink: Send + Sync + 'static {
    /// The script called `method(params)`; answer with [`ScriptRun::reply`].
    fn request(&self, run_id: &str, req_id: u64, method: &str, params: Value);
    /// One line the script wrote to stdout or stderr.
    fn output(&self, run_id: &str, stream: &'static str, line: String);
    /// The script finished: exit code, or `None` when killed / not started.
    /// For a notebook connection: it was turned off.
    fn exit(&self, run_id: &str, code: Option<i32>);
    /// Notebook connection only: `"connected"` or `"waiting"` (for a client).
    fn status(&self, _run_id: &str, _status: &'static str) {}
}

/// A running script. Held in the app state so `script_reply` / `script_stop`
/// can reach it by id.
pub struct ScriptRun {
    pub id: String,
    pub dry_run: bool,
    replies: mpsc::UnboundedSender<String>,
    kill: Mutex<Option<oneshot::Sender<()>>>,
}

impl ScriptRun {
    /// Answer request `req_id`: `Ok(result)` or `Err(error object)` where the
    /// error object is `{"kind": "...", "message": "...", ...}`.
    pub fn reply(&self, req_id: u64, outcome: Result<Value, Value>) {
        let line = match outcome {
            Ok(result) => serde_json::json!({ "id": req_id, "ok": true, "result": result }),
            Err(error) => serde_json::json!({ "id": req_id, "ok": false, "error": error }),
        };
        let _ = self.replies.send(line.to_string());
    }

    /// Kill the script's process. Idempotent.
    pub fn stop(&self) {
        if let Some(tx) = self.kill.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
}

/// Live runs by id.
pub type Runs = Arc<Mutex<HashMap<String, Arc<ScriptRun>>>>;

/// The Python 3 interpreter to use: `configured` when set, else the first of
/// `python3`, `python` (and `py -3` on Windows) that reports Python 3.
pub fn find_python(configured: Option<&str>) -> Result<Vec<String>, String> {
    let mut candidates: Vec<Vec<String>> = Vec::new();
    if let Some(c) = configured.map(str::trim).filter(|c| !c.is_empty()) {
        candidates.push(vec![c.to_string()]);
    } else {
        if cfg!(windows) {
            candidates.push(vec!["py".into(), "-3".into()]);
        }
        candidates.push(vec!["python3".into()]);
        candidates.push(vec!["python".into()]);
    }
    for cand in &candidates {
        let out = std::process::Command::new(&cand[0])
            .args(&cand[1..])
            .arg("--version")
            .output();
        if let Ok(out) = out {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if out.status.success() && text.trim_start().starts_with("Python 3") {
                return Ok(cand.clone());
            }
        }
    }
    Err(match configured.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("\"{c}\" is not a working Python 3 interpreter"),
        None => "Python 3 not found — install it (python.org) or set its path in \
                 the Run Python script dialog"
            .into(),
    })
}

/// Example scripts offered in Tools → Script examples (file name, content).
pub const EXAMPLES: &[(&str, &str)] = &[
    ("hello.py", include_str!("examples/hello.py")),
    ("sase_reset.py", include_str!("examples/sase_reset.py")),
    ("explore.ipynb", include_str!("examples/explore.ipynb")),
];

/// Write the example scripts into `dir`, keeping any the user already edited
/// (only missing files are written). A copy of `zapterm.py` — always refreshed —
/// sits next to them so editors can autocomplete the API.
pub fn install_examples(dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    for (name, content) in EXAMPLES {
        let path = dir.join(name);
        if !path.exists() {
            std::fs::write(&path, content)?;
        }
    }
    std::fs::write(dir.join("zapterm.py"), ZAPTERM_MODULE)?;
    Ok(dir.to_path_buf())
}

/// Write the bundled `zapterm` module into `dir` (created if needed).
pub fn install_module(dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("zapterm.py");
    std::fs::write(&path, ZAPTERM_MODULE)?;
    std::fs::write(dir.join("zapx.py"), ZAPX_ALIAS_MODULE)?;
    Ok(dir.to_path_buf())
}

/// Start `script` with `python` (argv prefix from [`find_python`]). The
/// `module_dir` holding `zapterm.py` is prepended to `PYTHONPATH`. Returns once
/// the process is spawned; everything after is reported through `sink`.
pub async fn start(
    python: &[String],
    script: &Path,
    module_dir: &Path,
    dry_run: bool,
    read_only: bool,
    sink: Arc<dyn ScriptSink>,
    runs: Runs,
) -> Result<Arc<ScriptRun>, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("could not open the script channel: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = uuid::Uuid::new_v4().to_string();
    let id = uuid::Uuid::new_v4().to_string();

    let pythonpath = match std::env::var_os("PYTHONPATH") {
        Some(existing) => {
            let mut paths = vec![module_dir.to_path_buf()];
            paths.extend(std::env::split_paths(&existing));
            std::env::join_paths(paths).map_err(|e| e.to_string())?
        }
        None => module_dir.as_os_str().to_owned(),
    };
    let mut cmd = Command::new(&python[0]);
    cmd.args(&python[1..])
        .arg("-u") // unbuffered: prints reach the panel as they happen
        .arg(script)
        .env("ZAPTERM_PORT", port.to_string())
        .env("ZAPTERM_TOKEN", &token)
        .env("ZAPTERM_DRY_RUN", if dry_run { "1" } else { "0" })
        // Informational: ZapTerm itself enforces read-only (the frontend host
        // refuses to type disallowed commands); scripts can adapt to it.
        .env("ZAPTERM_READ_ONLY", if read_only { "1" } else { "0" })
        .env("PYTHONPATH", pythonpath)
        .env("PYTHONIOENCODING", "utf-8")
        // No __pycache__ littering the user's script folders.
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(dir) = script.parent().filter(|d| !d.as_os_str().is_empty()) {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        // No console window flashing up behind ZapTerm.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", python.join(" ")))?;

    let (reply_tx, reply_rx) = mpsc::unbounded_channel::<String>();
    let (kill_tx, kill_rx) = oneshot::channel::<()>();
    let run = Arc::new(ScriptRun {
        id: id.clone(),
        dry_run,
        replies: reply_tx,
        kill: Mutex::new(Some(kill_tx)),
    });
    runs.lock().unwrap().insert(id.clone(), run.clone());

    // stdout / stderr → panel, line by line.
    for (stream, pipe) in [
        ("stdout", child.stdout.take().map(PipeKind::Out)),
        ("stderr", child.stderr.take().map(PipeKind::Err)),
    ] {
        let Some(pipe) = pipe else { continue };
        let sink = sink.clone();
        let id = id.clone();
        tokio::spawn(async move {
            let mut lines = match pipe {
                PipeKind::Out(p) => LinesOf::Out(BufReader::new(p).lines()),
                PipeKind::Err(p) => LinesOf::Err(BufReader::new(p).lines()),
            };
            while let Some(line) = lines.next().await {
                sink.output(&id, stream, line);
            }
        });
    }

    // The API channel: one authenticated client, JSON lines both ways.
    {
        let sink = sink.clone();
        let id = id.clone();
        tokio::spawn(async move {
            serve(listener, token, id, sink, reply_rx).await;
        });
    }

    // Wait for exit or a stop request; then tell the sink and forget the run.
    tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()),
            _ = kill_rx => {
                let _ = child.kill().await;
                None
            }
        };
        runs.lock().unwrap().remove(&id);
        sink.exit(&id, code);
    });

    Ok(run)
}

enum PipeKind {
    Out(tokio::process::ChildStdout),
    Err(tokio::process::ChildStderr),
}

enum LinesOf {
    Out(tokio::io::Lines<BufReader<tokio::process::ChildStdout>>),
    Err(tokio::io::Lines<BufReader<tokio::process::ChildStderr>>),
}

impl LinesOf {
    async fn next(&mut self) -> Option<String> {
        let r = match self {
            LinesOf::Out(l) => l.next_line().await,
            LinesOf::Err(l) => l.next_line().await,
        };
        r.ok().flatten()
    }
}

/// Read the client's opening line and accept it only if it carries `token`
/// (then confirm with [`HELLO_OK`]). Anything else is dropped.
async fn handshake(stream: TcpStream, token: &str) -> Option<(Reader, OwnedWriteHalf)> {
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);
    let mut first = String::new();
    let hello = tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut first)).await;
    let ok = matches!(hello, Ok(Ok(n)) if n > 0)
        && serde_json::from_str::<Value>(&first)
            .ok()
            .and_then(|v| v.get("token").and_then(|t| t.as_str()).map(|t| t == token))
            .unwrap_or(false);
    if !ok || w.write_all(HELLO_OK.as_bytes()).await.is_err() {
        return None;
    }
    Some((reader, w))
}

type Reader = BufReader<OwnedReadHalf>;

/// Hand one request line from the client to the sink (malformed lines are
/// ignored).
fn forward_request(line: &str, run_id: &str, sink: &Arc<dyn ScriptSink>) {
    let Ok(req) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(req_id) = req.get("id").and_then(Value::as_u64) else {
        return;
    };
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    sink.request(run_id, req_id, method, params);
}

async fn serve(
    listener: TcpListener,
    token: String,
    run_id: String,
    sink: Arc<dyn ScriptSink>,
    mut replies: mpsc::UnboundedReceiver<String>,
) {
    // Accept until a client presents the token (anything else is dropped).
    let (mut reader, mut writer) = loop {
        let Ok(Ok((stream, _))) = tokio::time::timeout(ACCEPT_WINDOW, listener.accept()).await
        else {
            return;
        };
        if let Some(client) = handshake(stream, &token).await {
            break client;
        }
    };
    drop(listener); // one client per run

    // Replies from the frontend → script.
    let writer_task = tokio::spawn(async move {
        while let Some(line) = replies.recv().await {
            if writer
                .write_all(format!("{line}\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Requests from the script → sink.
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => forward_request(&line, &run_id, &sink),
        }
    }
    writer_task.abort();
}

// ── notebook connection ─────────────────────────────────────────────────────

/// Where a notebook finds the connection (`{"port": …, "token": …}`), written
/// readable by the owner only.
pub fn write_connection_file(path: &Path, port: u16, token: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = std::fs::remove_file(path);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(
        serde_json::json!({ "port": port, "token": token })
            .to_string()
            .as_bytes(),
    )
}

/// Turn on the notebook connection: listen on 127.0.0.1 with a new token,
/// write `connection_file`, and serve one client at a time (a second one is
/// refused while the first is connected) until [`ScriptRun::stop`] or `idle`
/// passes without any request. Then the file is removed and the sink gets
/// `exit(None)`. The returned run answers requests like a script's.
pub async fn start_notebook(
    connection_file: &Path,
    idle: Option<Duration>,
    sink: Arc<dyn ScriptSink>,
    runs: Runs,
) -> Result<Arc<ScriptRun>, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("could not open the notebook channel: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = uuid::Uuid::new_v4().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    write_connection_file(connection_file, port, &token)
        .map_err(|e| format!("could not write {}: {e}", connection_file.display()))?;

    let (reply_tx, mut replies) = mpsc::unbounded_channel::<String>();
    let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
    let run = Arc::new(ScriptRun {
        id: id.clone(),
        dry_run: false,
        replies: reply_tx,
        kill: Mutex::new(Some(kill_tx)),
    });
    runs.lock().unwrap().insert(id.clone(), run.clone());

    let connection_file = connection_file.to_path_buf();
    tokio::spawn(async move {
        let mut reader: Option<Reader> = None;
        let mut writer: Option<OwnedWriteHalf> = None;
        let mut line = String::new();
        let mut last_activity = tokio::time::Instant::now();
        loop {
            let idle_deadline = idle.map(|d| last_activity + d);
            tokio::select! {
                _ = &mut kill_rx => break,
                _ = async {
                    match idle_deadline {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                } => {
                    sink.output(&id, "stderr", "Notebook connection turned off after inactivity.".into());
                    break;
                }
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { continue };
                    if writer.is_some() {
                        // One notebook at a time: tell the newcomer why.
                        let mut stream = stream;
                        let refusal = serde_json::json!({
                            "hello": false,
                            "error": "another notebook is already connected to ZapTerm",
                        });
                        let _ = stream.write_all(format!("{refusal}\n").as_bytes()).await;
                        continue;
                    }
                    if let Some((r, w)) = handshake(stream, &token).await {
                        reader = Some(r);
                        writer = Some(w);
                        last_activity = tokio::time::Instant::now();
                        sink.status(&id, "connected");
                    }
                }
                read = async {
                    match reader.as_mut() {
                        Some(r) => {
                            line.clear();
                            r.read_line(&mut line).await
                        }
                        None => std::future::pending().await,
                    }
                } => {
                    match read {
                        Ok(0) | Err(_) => {
                            reader = None;
                            writer = None;
                            sink.status(&id, "waiting");
                        }
                        Ok(_) => {
                            last_activity = tokio::time::Instant::now();
                            forward_request(&line, &id, &sink);
                        }
                    }
                }
                Some(reply) = replies.recv() => {
                    if let Some(w) = writer.as_mut() {
                        if w.write_all(format!("{reply}\n").as_bytes()).await.is_err() {
                            reader = None;
                            writer = None;
                            sink.status(&id, "waiting");
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&connection_file);
        runs.lock().unwrap().remove(&id);
        sink.exit(&id, None);
    });
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// Answers every request in-process the way the frontend would, and
    /// records what the script printed and how it ended.
    struct FakeHost {
        runs: Runs,
        log: StdMutex<Vec<String>>,
        done: StdMutex<Option<oneshot::Sender<Option<i32>>>>,
        /// A `hold` request left unanswered until `cancel`.
        held: StdMutex<Option<u64>>,
    }

    impl ScriptSink for FakeHost {
        fn request(&self, run_id: &str, req_id: u64, method: &str, params: Value) {
            self.log
                .lock()
                .unwrap()
                .push(format!("req {method} {params}"));
            let run = self.runs.lock().unwrap().get(run_id).cloned().unwrap();
            if method == "hold" {
                *self.held.lock().unwrap() = Some(req_id);
                return;
            }
            if method == "cancel" {
                // The held call answers late, after the script gave up on it.
                if let Some(held) = self.held.lock().unwrap().take() {
                    run.reply(held, Ok(Value::String("late".into())));
                }
            }
            let outcome = match method {
                "session" => {
                    Ok(serde_json::json!({"id": "s1", "name": "fw1", "platform": "fortigate"}))
                }
                "run" => {
                    let cmd = params["command"].as_str().unwrap_or("");
                    if cmd == "slow" {
                        Err(
                            serde_json::json!({"kind": "timeout", "message": "no prompt", "output": "partial"}),
                        )
                    } else {
                        Ok(Value::String(format!("output of {cmd}")))
                    }
                }
                "open" => {
                    Ok(serde_json::json!({"id": "s2", "name": params["name"], "platform": null}))
                }
                "confirm" => Ok(Value::Bool(true)),
                "mode" => Ok(serde_json::json!({"dry_run": false, "read_only": true})),
                "tab" => Ok(
                    serde_json::json!({"id": "s1", "name": params["name"], "platform": "fortigate"}),
                ),
                "tabs" => {
                    Ok(serde_json::json!([{"id": "s1", "name": "fw1", "platform": "fortigate"}]))
                }
                "report" | "send" | "close" | "note" | "cancel" => Ok(Value::Null),
                _ => Err(
                    serde_json::json!({"kind": "error", "message": format!("unknown {method}")}),
                ),
            };
            run.reply(req_id, outcome);
        }
        fn output(&self, _run_id: &str, stream: &'static str, line: String) {
            self.log.lock().unwrap().push(format!("{stream}: {line}"));
        }
        fn exit(&self, _run_id: &str, code: Option<i32>) {
            if let Some(tx) = self.done.lock().unwrap().take() {
                let _ = tx.send(code);
            }
        }
        fn status(&self, _run_id: &str, status: &'static str) {
            self.log.lock().unwrap().push(format!("status {status}"));
        }
    }

    fn fake_host(runs: &Runs, done: Option<oneshot::Sender<Option<i32>>>) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            runs: runs.clone(),
            log: StdMutex::new(Vec::new()),
            done: StdMutex::new(done),
            held: StdMutex::new(None),
        })
    }

    fn python_or_skip() -> Option<Vec<String>> {
        match find_python(None) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("skipping: {e}");
                None
            }
        }
    }

    async fn run_script(body: &str, dry_run: bool) -> (Option<i32>, Vec<String>) {
        run_script_mode(body, dry_run, false).await
    }

    async fn run_script_mode(
        body: &str,
        dry_run: bool,
        read_only: bool,
    ) -> (Option<i32>, Vec<String>) {
        let dir =
            std::env::temp_dir().join(format!("zapterm-script-test-{}", uuid::Uuid::new_v4()));
        let module_dir = install_module(&dir.join("lib")).unwrap();
        let script = dir.join("t.py");
        std::fs::write(&script, body).unwrap();
        let runs: Runs = Arc::default();
        let (tx, rx) = oneshot::channel();
        let host = fake_host(&runs, Some(tx));
        let python = python_or_skip().unwrap();
        start(
            &python,
            &script,
            &module_dir,
            dry_run,
            read_only,
            host.clone(),
            runs,
        )
        .await
        .unwrap();
        let code = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .expect("script finished")
            .unwrap();
        // Let the pipe readers flush the last lines.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let log = host.log.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(dir);
        (code, log)
    }

    #[tokio::test]
    async fn api_calls_round_trip_and_prints_stream() {
        if python_or_skip().is_none() {
            return;
        }
        let (code, log) = run_script(
            r#"
import zapterm
s = zapterm.session()
print("platform", s.platform, "dry", zapterm.dry_run)
print(s.run("get system status"))
try:
    s.run("slow")
except zapterm.Timeout as e:
    print("timeout with", e.output)
zapterm.report(s.name, "OK", "done")
"#,
            false,
        )
        .await;
        assert_eq!(code, Some(0), "log: {log:#?}");
        assert!(
            log.contains(&"stdout: platform fortigate dry False".to_string()),
            "{log:#?}"
        );
        assert!(
            log.contains(&"stdout: output of get system status".to_string()),
            "{log:#?}"
        );
        assert!(
            log.contains(&"stdout: timeout with partial".to_string()),
            "{log:#?}"
        );
        assert!(
            log.iter()
                .any(|l| l.starts_with("req report") && l.contains("\"fw1\"")),
            "{log:#?}"
        );
    }

    #[tokio::test]
    async fn errors_raise_and_exit_code_is_reported() {
        if python_or_skip().is_none() {
            return;
        }
        let (code, log) = run_script(
            "import zapterm\nzapterm.sessions()\n", // unknown method in FakeHost → zapterm.Error
            true,
        )
        .await;
        assert_eq!(code, Some(1));
        assert!(
            log.iter()
                .any(|l| l.contains("zapterm.Error: unknown sessions")),
            "{log:#?}"
        );
    }

    #[tokio::test]
    async fn dry_run_flag_reaches_the_script() {
        if python_or_skip().is_none() {
            return;
        }
        let (code, log) = run_script(
            "import zapterm\nprint(zapterm.dry_run)\nzapterm.sleep(5)\n",
            true,
        )
        .await;
        assert_eq!(code, Some(0));
        assert!(log.contains(&"stdout: True".to_string()), "{log:#?}");
        // In dry-run, sleep() is reported through the API (in order), not slept.
        assert!(
            log.iter()
                .any(|l| l.starts_with("req note") && l.contains("would wait 5 s")),
            "{log:#?}"
        );
    }

    /// Scripts written for ZAPX (`import zapx`) get the same module.
    #[tokio::test]
    async fn zapx_scripts_still_run() {
        if python_or_skip().is_none() {
            return;
        }
        let (code, log) = run_script(
            "import zapx, zapterm\nprint(zapx is zapterm, zapx.dry_run)\n",
            true,
        )
        .await;
        assert_eq!(code, Some(0), "{log:#?}");
        assert!(log.contains(&"stdout: True True".to_string()), "{log:#?}");
    }

    #[tokio::test]
    async fn a_client_without_the_token_is_refused() {
        if python_or_skip().is_none() {
            return;
        }
        // The script dials the port with a wrong token first; that connection
        // must get nothing, while its real zapterm calls still work.
        let (code, log) = run_script(
            r#"
import json, os, socket, zapterm
bad = socket.create_connection(("127.0.0.1", int(os.environ["ZAPTERM_PORT"])))
bad.sendall(b'{"token": "nope"}\n')
bad.settimeout(1)
try:
    got = bad.recv(100)
    print("bad got", got)
except socket.timeout:
    print("bad refused")
print(zapterm.session().name)
"#,
            false,
        )
        .await;
        assert_eq!(code, Some(0), "{log:#?}");
        assert!(
            log.contains(&"stdout: bad got b''".to_string())
                || log.contains(&"stdout: bad refused".to_string()),
            "{log:#?}"
        );
        assert!(log.contains(&"stdout: fw1".to_string()), "{log:#?}");
    }

    #[test]
    fn examples_are_written_once_and_keep_user_edits() {
        let dir = std::env::temp_dir().join(format!("zapterm-examples-{}", uuid::Uuid::new_v4()));
        install_examples(&dir).unwrap();
        for (name, content) in EXAMPLES {
            assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), *content);
        }
        std::fs::write(dir.join("hello.py"), "edited").unwrap();
        std::fs::write(dir.join("zapterm.py"), "stale").unwrap();
        install_examples(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("hello.py")).unwrap(),
            "edited"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("zapterm.py")).unwrap(),
            ZAPTERM_MODULE
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn examples_dry_run_cleanly() {
        // Every bundled example must survive a dry run: no real output, all
        // calls answered by the fake host. Catches syntax errors and API drift.
        if python_or_skip().is_none() {
            return;
        }
        for (name, content) in EXAMPLES.iter().filter(|(n, _)| n.ends_with(".py")) {
            // The fake host doesn't know `sessions`/`open`/`confirm`/`close`;
            // give the SASE example an explicit device and stub those calls.
            let body = if *name == "sase_reset.py" {
                patched(
                    content,
                    &[("DEVICES: list[str] = []", "DEVICES: list[str] = [\"fw1\"]")],
                )
            } else {
                content.to_string()
            };
            let (code, log) = run_script(&body, true).await;
            assert_eq!(code, Some(0), "{name}: {log:#?}");
        }
    }

    #[tokio::test]
    async fn stop_kills_the_process() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("zapterm-script-stop-{}", uuid::Uuid::new_v4()));
        let module_dir = install_module(&dir.join("lib")).unwrap();
        let script = dir.join("t.py");
        std::fs::write(&script, "import time\ntime.sleep(60)\n").unwrap();
        let runs: Runs = Arc::default();
        let (tx, rx) = oneshot::channel();
        let host = fake_host(&runs, Some(tx));
        let run = start(
            &python,
            &script,
            &module_dir,
            false,
            false,
            host,
            runs.clone(),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        run.stop();
        let code = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(code, None);
        assert!(runs.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    // ── notebook connection ─────────────────────────────────────────────────

    /// Run `body` as a plain Python process (like a notebook kernel would be:
    /// not started by ZapTerm) with the module on its path. Returns stdout.
    async fn run_outside(python: &[String], module_dir: &Path, body: &str) -> String {
        let out = Command::new(&python[0])
            .args(&python[1..])
            .arg("-c")
            .arg(body)
            .env("PYTHONPATH", module_dir)
            .env_remove("ZAPTERM_PORT")
            .env_remove("ZAPTERM_TOKEN")
            .output()
            .await
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.trim().is_empty(), "stderr: {stderr}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    struct Notebook {
        dir: PathBuf,
        file: PathBuf,
        module_dir: PathBuf,
        runs: Runs,
        host: Arc<FakeHost>,
        run: Arc<ScriptRun>,
        exited: oneshot::Receiver<Option<i32>>,
    }

    async fn notebook(idle: Option<Duration>) -> Notebook {
        let dir = std::env::temp_dir().join(format!("zapterm-nb-test-{}", uuid::Uuid::new_v4()));
        let module_dir = install_module(&dir.join("lib")).unwrap();
        let file = dir.join("data").join("connection.json");
        let runs: Runs = Arc::default();
        let (tx, exited) = oneshot::channel();
        let host = fake_host(&runs, Some(tx));
        let run = start_notebook(&file, idle, host.clone(), runs.clone())
            .await
            .unwrap();
        Notebook {
            dir,
            file,
            module_dir,
            runs,
            host,
            run,
            exited,
        }
    }

    #[tokio::test]
    async fn a_notebook_connects_through_the_file_and_can_reconnect() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let nb = notebook(None).await;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&nb.file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "connection file must be owner-only");
        }
        let body = format!(
            "import zapterm\nzapterm.connect(path={:?})\nprint(zapterm.session().name, [t.name for t in zapterm.tabs()], zapterm.read_only)\n",
            nb.file.to_string_lossy()
        );
        // `read_only` comes from the host (its "mode" reply), not the env.
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert_eq!(out.trim(), "fw1 ['fw1'] True");
        // The first kernel went away; a second one connects fine.
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert_eq!(out.trim(), "fw1 ['fw1'] True");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let log = nb.host.log.lock().unwrap().clone();
        let statuses: Vec<_> = log.iter().filter(|l| l.starts_with("status")).collect();
        assert_eq!(
            statuses,
            [
                "status connected",
                "status waiting",
                "status connected",
                "status waiting"
            ],
            "{log:#?}"
        );
        nb.run.stop();
        let code = tokio::time::timeout(Duration::from_secs(5), nb.exited)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(code, None);
        assert!(!nb.file.exists(), "turning it off removes the file");
        assert!(nb.runs.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[tokio::test]
    async fn re_running_connect_in_the_same_notebook_works() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let nb = notebook(None).await;
        // A notebook re-runs its first cell: connect() while already connected.
        let body = format!(
            "import zapterm\nfor _ in range(3):\n    zapterm.connect(path={:?})\n    print(zapterm.session().name)\n",
            nb.file.to_string_lossy()
        );
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert_eq!(out.trim(), "fw1\nfw1\nfw1", "{out}");
        nb.run.stop();
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[tokio::test]
    async fn a_second_notebook_is_refused_while_one_is_connected() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let nb = notebook(None).await;
        let body = format!(
            r#"import zapterm
zapterm.connect(path={file:?})
zapterm.session()
import zapterm as _z, importlib.util, sys
spec = importlib.util.spec_from_file_location("zapterm2", _z.__file__)
other = importlib.util.module_from_spec(spec); spec.loader.exec_module(other)
try:
    other.connect(path={file:?})
    print("second accepted")
except other.Error as e:
    print("refused:", e)
print(zapterm.session().name)
"#,
            file = nb.file.to_string_lossy()
        );
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert!(
            out.contains("refused: another notebook is already connected"),
            "{out}"
        );
        assert!(
            out.trim_end().ends_with("fw1"),
            "first one still works: {out}"
        );
        nb.run.stop();
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[tokio::test]
    async fn a_wrong_token_is_refused_with_a_message() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let nb = notebook(None).await;
        let port: u16 = serde_json::from_str::<Value>(&std::fs::read_to_string(&nb.file).unwrap())
            .unwrap()["port"]
            .as_u64()
            .unwrap() as u16;
        let body = format!(
            "import zapterm\ntry:\n    zapterm.connect(port={port}, token='nope')\nexcept zapterm.Error as e:\n    print('refused:', e)\n"
        );
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert!(
            out.starts_with("refused: ZapTerm refused the connection"),
            "{out}"
        );
        nb.run.stop();
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_interrupted_call_is_cancelled_and_its_late_reply_ignored() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let nb = notebook(None).await;
        // A stopped cell: SIGINT while waiting on a call ZapTerm hasn't answered.
        let body = format!(
            r#"import os, signal, threading, zapterm
zapterm.connect(path={file:?})
threading.Timer(0.5, lambda: os.kill(os.getpid(), signal.SIGINT)).start()
try:
    zapterm._call("hold")
except KeyboardInterrupt:
    print("interrupted")
print(zapterm.session().name)
"#,
            file = nb.file.to_string_lossy()
        );
        let out = run_outside(&python, &nb.module_dir, &body).await;
        assert_eq!(
            out.trim(),
            "interrupted\nfw1",
            "late reply must not leak: {out}"
        );
        let log = nb.host.log.lock().unwrap().clone();
        assert!(log.iter().any(|l| l.starts_with("req cancel")), "{log:#?}");
        nb.run.stop();
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[tokio::test]
    async fn the_notebook_connection_turns_itself_off_when_idle() {
        let nb = notebook(Some(Duration::from_millis(300))).await;
        let code = tokio::time::timeout(Duration::from_secs(5), nb.exited)
            .await
            .expect("turned off")
            .unwrap();
        assert_eq!(code, None);
        assert!(!nb.file.exists());
        let _ = std::fs::remove_dir_all(nb.dir);
    }

    #[tokio::test]
    async fn the_example_notebook_runs_cell_by_cell() {
        let Some(python) = python_or_skip() else {
            return;
        };
        let (_, nb_json) = EXAMPLES
            .iter()
            .find(|(n, _)| *n == "explore.ipynb")
            .unwrap();
        let nb: Value = serde_json::from_str(nb_json).unwrap();
        let conn = notebook(None).await;
        let mut body = String::new();
        for cell in nb["cells"].as_array().unwrap() {
            if cell["cell_type"] != "code" {
                continue;
            }
            for line in cell["source"].as_array().unwrap() {
                body.push_str(line.as_str().unwrap());
            }
            body.push('\n');
        }
        let path = format!("zapterm.connect(path={:?})", conn.file.to_string_lossy());
        let body = patched(&body, &[("zapterm.connect()", path.as_str())]);
        let out = run_outside(&python, &conn.module_dir, &body).await;
        assert!(out.contains("down:    none"), "{out}");
        conn.run.stop();
        let _ = std::fs::remove_dir_all(conn.dir);
    }

    /// Apply text replacements to an example, failing loudly when one no
    /// longer matches (the example changed), instead of silently testing
    /// something else.
    fn patched(body: &str, edits: &[(&str, &str)]) -> String {
        let mut out = body.to_string();
        for (from, to) in edits {
            assert!(out.contains(from), "example no longer contains {from:?}");
            out = out.replace(from, to);
        }
        out
    }

    #[tokio::test]
    async fn read_only_flag_reaches_the_script_and_the_example_adapts() {
        if python_or_skip().is_none() {
            return;
        }
        let (code, log) = run_script_mode(
            "import zapterm\nprint(zapterm.read_only, zapterm.dry_run)\n",
            false,
            true,
        )
        .await;
        assert_eq!(code, Some(0));
        assert!(log.contains(&"stdout: True False".to_string()), "{log:#?}");

        // sase_reset in read-only: reads (the fake host's output has no tunnel
        // up) and reports instead of resetting. Its device-safety checks are
        // switched off here because the fake output isn't a real FortiOS one.
        let (_, body) = EXAMPLES
            .iter()
            .find(|(n, _)| *n == "sase_reset.py")
            .unwrap();
        let body = patched(
            body,
            &[
                ("DEVICES: list[str] = []", "DEVICES: list[str] = [\"fw1\"]"),
                (
                    "not zapterm.dry_run and \"Virtual domain configuration: disable\" not in status",
                    "False",
                ),
                ("if missing and not zapterm.dry_run:", "if False:"),
            ],
        );
        let (code, log) = run_script_mode(&body, false, true).await;
        assert_eq!(code, Some(0), "{log:#?}");
        assert!(
            log.iter()
                .any(|l| l.starts_with("req report") && l.contains("READ-ONLY")),
            "{log:#?}"
        );
        assert!(
            !log.iter().any(|l| l.contains("set status")),
            "no change commands may be issued: {log:#?}"
        );
    }
}
