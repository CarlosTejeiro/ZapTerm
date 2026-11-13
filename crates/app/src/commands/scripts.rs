//! Tauri commands for Python scripting (see `crate::scripting`).
//!
//! Events to the frontend:
//! - `script-request` `{run_id, req_id, method, params}` — carry out an API
//!   call and answer with [`script_reply`];
//! - `script-output` `{run_id, stream, line}` — a line the script printed;
//! - `script-exit` `{run_id, code}` — the script ended (`code` null = killed),
//!   or the notebook connection was turned off;
//! - `script-status` `{run_id, status}` — notebook connection only:
//!   `"connected"` / `"waiting"`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::scripting::{self, ScriptSink};
use crate::state::AppState;

/// Settings key for an explicit interpreter (empty = auto-detect).
const PYTHON_SETTING: &str = "scripts.python";
/// Settings key: minutes without a notebook call before the notebook
/// connection turns itself off (`0` = never). Default [`NOTEBOOK_IDLE_DEFAULT`].
const NOTEBOOK_IDLE_SETTING: &str = "notebook.idle_minutes";
const NOTEBOOK_IDLE_DEFAULT: u64 = 60;

struct TauriSink(AppHandle);

#[derive(Serialize, Clone)]
struct RequestEvent<'a> {
    run_id: &'a str,
    req_id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Serialize, Clone)]
struct OutputEvent<'a> {
    run_id: &'a str,
    stream: &'a str,
    line: String,
}

#[derive(Serialize, Clone)]
struct ExitEvent<'a> {
    run_id: &'a str,
    code: Option<i32>,
}

impl ScriptSink for TauriSink {
    fn request(&self, run_id: &str, req_id: u64, method: &str, params: Value) {
        let _ = self.0.emit(
            "script-request",
            RequestEvent {
                run_id,
                req_id,
                method,
                params,
            },
        );
    }
    fn output(&self, run_id: &str, stream: &'static str, line: String) {
        let _ = self.0.emit(
            "script-output",
            OutputEvent {
                run_id,
                stream,
                line,
            },
        );
    }
    fn exit(&self, run_id: &str, code: Option<i32>) {
        let _ = self.0.emit("script-exit", ExitEvent { run_id, code });
    }
    fn status(&self, run_id: &str, status: &'static str) {
        let _ = self.0.emit("script-status", StatusEvent { run_id, status });
    }
}

#[derive(Serialize, Clone)]
struct StatusEvent<'a> {
    run_id: &'a str,
    status: &'a str,
}

#[derive(Serialize)]
pub struct ScriptStarted {
    pub run_id: String,
    pub dry_run: bool,
    /// The interpreter command line used, e.g. `"python3"`.
    pub python: String,
}

fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}

/// Start `path` with Python 3. The bundled `zapterm` module is (re)written to
/// `<data dir>/scripting/` and put on the script's `PYTHONPATH`.
#[tauri::command]
pub async fn script_start(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    dry_run: bool,
    read_only: Option<bool>,
) -> Result<ScriptStarted, AppError> {
    let script = PathBuf::from(&path);
    if !script.is_file() {
        return Err(AppError::Internal(format!("script not found: {path}")));
    }
    let configured = state.db.get_setting(PYTHON_SETTING).map_err(internal)?;
    let python = scripting::find_python(configured.as_deref()).map_err(AppError::Internal)?;
    let module_dir =
        scripting::install_module(&state.data_dir.dir.join("scripting")).map_err(internal)?;
    let run = scripting::start(
        &python,
        &script,
        &module_dir,
        dry_run,
        read_only.unwrap_or(false),
        Arc::new(TauriSink(app)),
        state.scripts.clone(),
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(ScriptStarted {
        run_id: run.id.clone(),
        dry_run: run.dry_run,
        python: python.join(" "),
    })
}

#[derive(Serialize)]
pub struct NotebookStarted {
    pub run_id: String,
    /// The file `zapterm.connect()` reads (port + token).
    pub connection_file: String,
    /// Folder holding `zapterm.py`, for the notebook's `sys.path`.
    pub module_dir: String,
    /// Turns itself off after this many idle minutes (`0` = never).
    pub idle_minutes: u64,
}

/// Turn on the notebook connection (see `scripting::start_notebook`). The
/// `zapterm` module and `connection.json` live in `<data dir>/scripting/`.
#[tauri::command]
pub async fn notebook_enable(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NotebookStarted, AppError> {
    let idle_minutes = state
        .db
        .get_setting(NOTEBOOK_IDLE_SETTING)
        .map_err(internal)?
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(NOTEBOOK_IDLE_DEFAULT);
    let module_dir =
        scripting::install_module(&state.data_dir.dir.join("scripting")).map_err(internal)?;
    let connection_file = module_dir.join("connection.json");
    let idle = (idle_minutes > 0).then(|| std::time::Duration::from_secs(idle_minutes * 60));
    let run = scripting::start_notebook(
        &connection_file,
        idle,
        Arc::new(TauriSink(app)),
        state.scripts.clone(),
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(NotebookStarted {
        run_id: run.id.clone(),
        connection_file: connection_file.to_string_lossy().into_owned(),
        module_dir: module_dir.to_string_lossy().into_owned(),
        idle_minutes,
    })
}

/// Answer a `script-request`. `ok` picks `result` or `error` (an object with
/// at least `kind` and `message`). A reply to a finished run is ignored.
#[tauri::command]
pub async fn script_reply(
    state: State<'_, AppState>,
    run_id: String,
    req_id: u64,
    ok: bool,
    result: Option<Value>,
    error: Option<Value>,
) -> Result<(), AppError> {
    let run = state.scripts.lock().unwrap().get(&run_id).cloned();
    if let Some(run) = run {
        let outcome = if ok {
            Ok(result.unwrap_or(Value::Null))
        } else {
            Err(error.unwrap_or_else(|| serde_json::json!({"kind": "error", "message": "failed"})))
        };
        run.reply(req_id, outcome);
    }
    Ok(())
}

/// The example scripts folder (`<data dir>/scripting/examples`), created and
/// filled on first use; user edits there are kept. With `reveal`, it is also
/// opened in the system file manager.
#[tauri::command]
pub async fn script_examples_dir(
    state: State<'_, AppState>,
    reveal: bool,
) -> Result<String, AppError> {
    let dir = scripting::install_examples(&state.data_dir.dir.join("scripting").join("examples"))
        .map_err(internal)?;
    if reveal {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
            tracing::warn!("script_examples_dir: opening the folder failed: {e}");
        }
    }
    Ok(dir.to_string_lossy().into_owned())
}

/// Kill a running script, or turn the notebook connection off. No-op when it
/// already ended.
#[tauri::command]
pub async fn script_stop(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    let run = state.scripts.lock().unwrap().get(&run_id).cloned();
    if let Some(run) = run {
        run.stop();
    }
    Ok(())
}

/// The interpreter scripts would use, e.g. `"python3 (Python 3.12.3)"`, or an
/// error saying why none works. `python` overrides the saved setting (used
/// to test a path before saving it).
#[tauri::command]
pub async fn script_python_info(
    state: State<'_, AppState>,
    python: Option<String>,
) -> Result<String, AppError> {
    let configured = match python {
        Some(p) => Some(p),
        None => state.db.get_setting(PYTHON_SETTING).map_err(internal)?,
    };
    let argv = scripting::find_python(configured.as_deref()).map_err(AppError::Internal)?;
    let out = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .arg("--version")
        .output()
        .map_err(internal)?;
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(format!("{} ({})", argv.join(" "), version.trim()))
}
