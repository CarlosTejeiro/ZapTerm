//! SFTP browser Tauri commands. Each looks up the SSH session by id, lazily
//! opens an SFTP subsystem on it the first time, then delegates to
//! [`core_transport::SftpClient`].
//!
//! Large transfers are streamed and emit `sftp-progress` events every ~100 ms
//! so the frontend can render a progress bar and offer a cancel button.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use core_transport::{new_sftp_cancel_token, SftpClient, SftpEntry, SftpSlot};

use crate::error::AppError;
use crate::state::AppState;

/// Progress event shipped to the frontend.
#[derive(serde::Serialize, Clone)]
struct SftpProgress {
    transfer_id: String,
    /// Bytes transferred so far.
    done: u64,
    /// Total file size, or `None` if not known up front.
    total: Option<u64>,
    /// `"download"` or `"upload"`.
    direction: &'static str,
}

/// Resolve (or lazily open) the [`SftpClient`] tied to a live SSH session.
async fn sftp_for(state: &AppState, session_id: &str) -> Result<Arc<SftpClient>, AppError> {
    // Pull ssh_handle + sftp slot out of the sync lock first; release before
    // any await to keep the sync mutex non-blocking.
    let (handle, slot): (_, SftpSlot) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))?;
        let h = session
            .ssh_handle
            .clone()
            .ok_or_else(|| AppError::Internal("session is not SSH".into()))?;
        let s = session
            .sftp
            .clone()
            .ok_or_else(|| AppError::Internal("session is not SSH".into()))?;
        (h, s)
    };
    let mut guard = slot.lock().await;
    if let Some(client) = &*guard {
        return Ok(Arc::clone(client));
    }
    // First-use: open the SFTP subsystem. Read-lock the handle briefly — only the
    // `channel_open_session` + `request_subsystem` calls happen under the
    // lock; the returned channel runs independently after that.
    let client = {
        let guard = handle.read().await;
        Arc::new(
            SftpClient::open(&guard)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?,
        )
    };
    *guard = Some(Arc::clone(&client));
    Ok(client)
}

#[tauri::command]
pub async fn sftp_list_dir(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<Vec<SftpEntry>, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.list_dir(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn sftp_stat(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<SftpEntry, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.stat(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Resolve `path` (typically `"."`) to an absolute remote path. Used by the UI
/// to find the initial directory (the user's `$HOME`) when opening the browser.
#[tauri::command]
pub async fn sftp_canonicalize(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<String, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.canonicalize(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn sftp_mkdir(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.create_dir(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn sftp_remove_dir(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.remove_dir(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn sftp_remove_file(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.remove_file(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, AppState>,
    session_id: String,
    from: String,
    to: String,
) -> Result<(), AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    sftp.rename(&from, &to)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Throttle helper: only emit when at least 100 ms have passed since the
/// last emission for this transfer. Last-progress events fire unconditionally
/// (the streaming function calls `progress` once after the loop exits).
const PROGRESS_INTERVAL_MS: u64 = 100;

/// Build a progress callback that emits `sftp-progress` over Tauri while
/// rate-limiting bursts to ~10 Hz. Always emits the first (done=0) and
/// final calls.
fn make_progress(
    app: AppHandle,
    transfer_id: String,
    direction: &'static str,
) -> core_transport::ProgressFn {
    let last_emit = Arc::new(AtomicU64::new(0));
    Arc::new(move |done: u64, total: Option<u64>| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let prev = last_emit.load(Ordering::Relaxed);
        let final_call = total.map(|t| done >= t).unwrap_or(false);
        if done > 0 && !final_call && now.saturating_sub(prev) < PROGRESS_INTERVAL_MS {
            return;
        }
        last_emit.store(now, Ordering::Relaxed);
        let _ = app.emit(
            "sftp-progress",
            SftpProgress {
                transfer_id: transfer_id.clone(),
                done,
                total,
                direction,
            },
        );
    })
}

/// Streaming download with per-chunk progress + cancel. The frontend supplies
/// `transfer_id` (any uuid will do) and listens for `sftp-progress` filtered
/// by that id; calling `sftp_cancel_transfer(transfer_id)` flips the cancel
/// flag the streaming loop checks after each chunk.
#[tauri::command]
pub async fn sftp_download_file(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    remote_path: String,
    local_path: String,
    transfer_id: String,
) -> Result<u64, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    let cancel = new_sftp_cancel_token();
    state
        .sftp_cancellations
        .lock()
        .unwrap()
        .insert(transfer_id.clone(), Arc::clone(&cancel));

    let progress = make_progress(app, transfer_id.clone(), "download");
    let result = sftp
        .download_streaming(&remote_path, &PathBuf::from(local_path), cancel, progress)
        .await;

    // Always remove the token, success or failure, so the map doesn't leak.
    state
        .sftp_cancellations
        .lock()
        .unwrap()
        .remove(&transfer_id);
    result.map_err(|e| AppError::Internal(e.to_string()))
}

/// Streaming upload — mirrors [`sftp_download_file`] for the opposite direction.
#[tauri::command]
pub async fn sftp_upload_file(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    local_path: String,
    remote_path: String,
    transfer_id: String,
) -> Result<u64, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;
    let cancel = new_sftp_cancel_token();
    state
        .sftp_cancellations
        .lock()
        .unwrap()
        .insert(transfer_id.clone(), Arc::clone(&cancel));

    let progress = make_progress(app, transfer_id.clone(), "upload");
    let result = sftp
        .upload_streaming(&PathBuf::from(local_path), &remote_path, cancel, progress)
        .await;

    state
        .sftp_cancellations
        .lock()
        .unwrap()
        .remove(&transfer_id);
    result.map_err(|e| AppError::Internal(e.to_string()))
}

/// Download a remote file to a temp location, open it in the OS default
/// editor, and watch it: every save is re-uploaded to the remote path until
/// the session closes. Returns the local temp path.
#[tauri::command]
pub async fn sftp_edit_file(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    remote_path: String,
) -> Result<String, AppError> {
    let sftp = sftp_for(&state, &session_id).await?;

    // Temp file in a per-edit subdir (keeps the original name for the editor).
    let name = local_edit_name(&remote_path);
    let dir = std::env::temp_dir()
        .join("zapterm-edits")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.to_string()))?;
    // The copy can be a config file with passwords in it: on a shared
    // machine the temp folder is readable by everyone unless told otherwise.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let local = dir.join(name);

    let noop: core_transport::ProgressFn = Arc::new(|_: u64, _: Option<u64>| {});
    sftp.download_streaming(&remote_path, &local, new_sftp_cancel_token(), noop)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    open_in_editor(&local)?;

    // Watch the temp file's mtime and re-upload on change. Runs until the
    // session goes away (then cleans up the temp file). Polling avoids the
    // editor-rename pitfalls of a single-file fs watcher.
    let start_mtime = std::fs::metadata(&local)
        .ok()
        .and_then(|m| m.modified().ok());
    let app2 = app.clone();
    let local2 = local.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let mut last = start_mtime;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let state = app2.state::<AppState>();
            // Session closed → stop watching and clean up.
            let Ok(sftp) = sftp_for(&state, &session_id).await else {
                break;
            };
            let m = std::fs::metadata(&local2)
                .ok()
                .and_then(|x| x.modified().ok());
            if m.is_some() && m != last {
                last = m;
                let noop: core_transport::ProgressFn = Arc::new(|_, _| {});
                match sftp
                    .upload_streaming(&local2, &remote_path, new_sftp_cancel_token(), noop)
                    .await
                {
                    Ok(_) => {
                        let _ = app2.emit("sftp-edit-saved", remote_path.clone());
                    }
                    Err(e) => {
                        let _ = app2.emit("sftp-edit-error", format!("{remote_path}: {e}"));
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&local2);
        if let Some(parent) = local2.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    });

    Ok(local.to_string_lossy().into_owned())
}

/// The local file name for editing `remote_path`: its last component, made
/// safe as a file name on this computer. The name comes from the server, so
/// it is not trusted: no path separators, no `.`/`..`, none of the characters
/// Windows refuses or that mean something to a shell.
fn local_edit_name(remote_path: &str) -> String {
    let last = remote_path.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String =
        last.chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '|' | '?' | '*' | '&' | '^' | '%' | '!' | '`' | '$'
                | ';' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();
    // Windows drops trailing dots and spaces; `.` and `..` end up empty.
    let cleaned = cleaned
        .trim_start()
        .trim_end_matches([' ', '.'])
        .to_string();
    if cleaned.is_empty() {
        "remote-file".to_string()
    } else {
        cleaned
    }
}

/// Whether opening `path` with its default application on Windows would run
/// it instead of showing it: programs, scripts and anything without an
/// extension to tell.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn runs_when_opened_on_windows(path: &std::path::Path) -> bool {
    const RUNNABLE: &[&str] = &[
        "exe",
        "com",
        "scr",
        "pif",
        "cpl",
        "msi",
        "msp",
        "msc",
        "bat",
        "cmd",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "hta",
        "ps1",
        "psm1",
        "py",
        "pyw",
        "pl",
        "rb",
        "jar",
        "lnk",
        "url",
        "reg",
        "inf",
        "appref-ms",
        "application",
        "gadget",
        "scf",
    ];
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => RUNNABLE.contains(&ext.to_ascii_lowercase().as_str()),
        None => true,
    }
}

/// Open a file for editing. Never through a shell (the name comes from the
/// server), and never in a way that would run it:
///
/// * macOS: `open -t` — the default *text editor*, whatever the file is.
/// * Windows: Notepad for anything that would run if opened (scripts,
///   programs); the default application for the rest.
/// * Linux: `xdg-open`, which picks by content type and shows scripts in an
///   editor.
fn open_in_editor(path: &std::path::Path) -> Result<(), AppError> {
    let spawn = |cmd: &str, args: &[&std::ffi::OsStr]| {
        std::process::Command::new(cmd)
            .args(args)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::Internal(e.to_string()))
    };
    let p = path.as_os_str();
    #[cfg(target_os = "macos")]
    return spawn("open", &[std::ffi::OsStr::new("-t"), p]);
    #[cfg(target_os = "windows")]
    return if runs_when_opened_on_windows(path) {
        spawn("notepad.exe", &[p])
    } else {
        spawn("explorer.exe", &[p])
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return spawn("xdg-open", &[p]);
}

/// Cooperatively cancel an in-flight SFTP transfer. The streaming loop
/// checks the flag after every chunk, so cancellation happens within at
/// most one chunk's wall-clock time (~ STREAM_CHUNK ÷ link bandwidth).
/// No-op if `transfer_id` isn't active (e.g. it already finished).
#[tauri::command]
pub async fn sftp_cancel_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> Result<(), AppError> {
    if let Some(token) = state.sftp_cancellations.lock().unwrap().get(&transfer_id) {
        token.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_name_is_the_last_component_made_safe() {
        assert_eq!(local_edit_name("/etc/nginx/nginx.conf"), "nginx.conf");
        assert_eq!(local_edit_name("C:\\temp\\a.txt"), "a.txt");
        assert_eq!(local_edit_name("/srv/"), "remote-file");
        assert_eq!(local_edit_name("/srv/.."), "remote-file");
        assert_eq!(local_edit_name("/srv/."), "remote-file");
        // Characters a shell or Windows would act on are neutralised.
        assert_eq!(local_edit_name("/tmp/a&calc.txt"), "a_calc.txt");
        assert_eq!(local_edit_name("/tmp/x|y?.log"), "x_y_.log");
        assert_eq!(local_edit_name("/home/me/.bashrc"), ".bashrc");
    }

    #[test]
    fn scripts_and_programs_are_not_opened_with_their_default_app() {
        use std::path::Path;
        for name in [
            "run.bat",
            "x.CMD",
            "a.vbs",
            "tool.py",
            "setup.exe",
            "s.ps1",
            "Makefile",
        ] {
            assert!(runs_when_opened_on_windows(Path::new(name)), "{name}");
        }
        for name in ["nginx.conf", "notes.txt", "data.json", "site.yaml", "a.log"] {
            assert!(!runs_when_opened_on_windows(Path::new(name)), "{name}");
        }
    }
}
