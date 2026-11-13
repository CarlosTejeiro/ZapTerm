//! Port-forwarding Tauri commands. Each command operates on a live SSH
//! session (looked up by its in-memory session UUID); the `ssh_handle` field
//! of [`ActiveSession`] is the [`core_transport::SharedSshHandle`] reused for
//! `direct-tcpip` (`-L` / `-D`) and `tcpip-forward` (`-R`).

use tauri::State;

use core_persistence::SavedForward;
use core_transport::{ForwardInfo, RemoteForwardRegistry, SharedSshHandle};

use crate::error::AppError;
use crate::state::AppState;

/// Look up the [`SharedSshHandle`] for a live SSH session, or return a clear
/// error.
fn session_handle(state: &AppState, session_id: &str) -> Result<SharedSshHandle, AppError> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get(session_id)
        .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))?;
    session
        .ssh_handle
        .clone()
        .ok_or_else(|| AppError::Internal("session is not SSH".into()))
}

/// Look up both the SSH handle and the remote-forward registry. Used by
/// [`add_remote_forward`] which needs to register a `(bind, target)` mapping
/// before issuing `tcpip-forward` so the handler can route incoming channels.
fn session_handle_and_registry(
    state: &AppState,
    session_id: &str,
) -> Result<(SharedSshHandle, RemoteForwardRegistry), AppError> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get(session_id)
        .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))?;
    let handle = session
        .ssh_handle
        .clone()
        .ok_or_else(|| AppError::Internal("session is not SSH".into()))?;
    let registry = session
        .remote_forwards
        .clone()
        .ok_or_else(|| AppError::Internal("session is not SSH".into()))?;
    Ok((handle, registry))
}

/// Keep a freshly opened forward under its session — unless the session was
/// closed while the forward was being opened (binding the port is async).
/// `close_session` removes the session before its forwards, so checking under
/// the sessions lock is enough: either the forward lands first and is dropped
/// with the rest, or the session is already gone and the controller is
/// dropped here, which stops the listener (it used to stay bound forever).
fn store_forward(
    state: &AppState,
    session_id: &str,
    controller: core_transport::ForwardController,
) -> Result<ForwardInfo, AppError> {
    let sessions = state.sessions.lock().unwrap();
    if !sessions.contains_key(session_id) {
        return Err(AppError::Internal(
            "the session was closed while the forward was opening".into(),
        ));
    }
    let info = controller.info.clone();
    state
        .forwards
        .lock()
        .unwrap()
        .entry(session_id.to_owned())
        .or_default()
        .push(controller);
    Ok(info)
}

/// A tunnel name as typed: trimmed, `None` when blank.
fn clean_name(name: Option<String>) -> Option<String> {
    name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty())
}

/// Bind a local TCP port and tunnel each connection to `(target_host,
/// target_port)` via `direct-tcpip` on the SSH session. Equivalent to
/// `ssh -L <bind_port>:<target_host>:<target_port>`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_local_forward(
    state: State<'_, AppState>,
    session_id: String,
    bind_addr: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    name: Option<String>,
) -> Result<ForwardInfo, AppError> {
    let handle = session_handle(&state, &session_id)?;
    let mut controller =
        core_transport::open_local_forward(handle, bind_addr, bind_port, target_host, target_port)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    controller.info.name = clean_name(name);
    store_forward(&state, &session_id, controller)
}

/// Bind a local TCP port as a SOCKS5 server; each accepted client's CONNECT
/// request determines the target. Equivalent to `ssh -D <bind_port>`.
#[tauri::command]
pub async fn add_dynamic_forward(
    state: State<'_, AppState>,
    session_id: String,
    bind_addr: String,
    bind_port: u16,
    name: Option<String>,
) -> Result<ForwardInfo, AppError> {
    let handle = session_handle(&state, &session_id)?;
    let mut controller = core_transport::open_dynamic_forward(handle, bind_addr, bind_port)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    controller.info.name = clean_name(name);
    store_forward(&state, &session_id, controller)
}

/// Ask the SSH server to listen on `(bind_addr, bind_port)` and tunnel every
/// inbound connection back to us; we bridge each to `(target_host,
/// target_port)` on this machine. Equivalent to
/// `ssh -R <bind_addr>:<bind_port>:<target_host>:<target_port>`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_remote_forward(
    state: State<'_, AppState>,
    session_id: String,
    bind_addr: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    name: Option<String>,
) -> Result<ForwardInfo, AppError> {
    let (handle, registry) = session_handle_and_registry(&state, &session_id)?;
    let mut controller = core_transport::open_remote_forward(
        handle,
        registry,
        bind_addr,
        bind_port,
        target_host,
        target_port,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;
    controller.info.name = clean_name(name);
    store_forward(&state, &session_id, controller)
}

/// Return the metadata for every active forward attached to `session_id`.
#[tauri::command]
pub async fn list_forwards(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<ForwardInfo>, AppError> {
    let map = state.forwards.lock().unwrap();
    Ok(map
        .get(&session_id)
        .map(|v| v.iter().map(|c| c.info.clone()).collect())
        .unwrap_or_default())
}

/// One active forward plus the live session it belongs to — for the global
/// tunnels manager.
#[derive(serde::Serialize)]
pub struct SessionForwardInfo {
    pub session_id: String,
    pub info: ForwardInfo,
}

/// Every active forward across all live sessions.
#[tauri::command]
pub async fn list_all_forwards(
    state: State<'_, AppState>,
) -> Result<Vec<SessionForwardInfo>, AppError> {
    let map = state.forwards.lock().unwrap();
    let mut out = Vec::new();
    for (sid, vec) in map.iter() {
        for c in vec {
            out.push(SessionForwardInfo {
                session_id: sid.clone(),
                info: c.info.clone(),
            });
        }
    }
    Ok(out)
}

/// Stop a single forward (by id) attached to `session_id`. Dropping the
/// controller aborts the listener task.
#[tauri::command]
pub async fn remove_forward(
    state: State<'_, AppState>,
    session_id: String,
    forward_id: String,
) -> Result<(), AppError> {
    let mut map = state.forwards.lock().unwrap();
    if let Some(vec) = map.get_mut(&session_id) {
        vec.retain(|c| c.info.id != forward_id);
    }
    Ok(())
}

// ── Persisted forwards (auto-started on connect) ─────────────────────────────

/// The port-forwards saved on a session (shown in the New/Edit Session
/// dialog and auto-started on connect).
#[tauri::command]
pub async fn get_session_forwards(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<Vec<SavedForward>, AppError> {
    state
        .db
        .list_session_forwards(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Replace the saved port-forwards of a session.
#[tauri::command]
pub async fn set_session_forwards(
    state: State<'_, AppState>,
    saved_session_id: i64,
    forwards: Vec<SavedForward>,
) -> Result<(), AppError> {
    state
        .db
        .set_session_forwards(saved_session_id, &forwards)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Open every saved forward against a freshly-connected SSH session and store
/// the controllers under `live_session_id`. Per-forward failures (e.g. a bind
/// port already in use) are logged and skipped — they must not abort the
/// session open. Called from `open_saved_session` after the session is live.
pub async fn autostart_saved_forwards(
    state: &AppState,
    live_session_id: &str,
    handle: SharedSshHandle,
    registry: RemoteForwardRegistry,
    forwards: &[SavedForward],
) {
    for f in forwards {
        let opened = match f.kind.as_str() {
            "local" => match (f.target_host.clone(), f.target_port) {
                (Some(th), Some(tp)) => {
                    core_transport::open_local_forward(
                        handle.clone(),
                        f.bind_addr.clone(),
                        f.bind_port,
                        th,
                        tp,
                    )
                    .await
                }
                _ => {
                    tracing::warn!("saved local forward missing target; skipped");
                    continue;
                }
            },
            "dynamic" => {
                core_transport::open_dynamic_forward(
                    handle.clone(),
                    f.bind_addr.clone(),
                    f.bind_port,
                )
                .await
            }
            "remote" => match (f.target_host.clone(), f.target_port) {
                (Some(th), Some(tp)) => {
                    core_transport::open_remote_forward(
                        handle.clone(),
                        registry.clone(),
                        f.bind_addr.clone(),
                        f.bind_port,
                        th,
                        tp,
                    )
                    .await
                }
                _ => {
                    tracing::warn!("saved remote forward missing target; skipped");
                    continue;
                }
            },
            other => {
                tracing::warn!("unknown saved forward kind {other:?}; skipped");
                continue;
            }
        };
        match opened {
            Ok(mut controller) => {
                controller.info.name = clean_name(f.name.clone());
                if let Err(e) = store_forward(state, live_session_id, controller) {
                    tracing::warn!("auto-start forward dropped: {e}");
                    return;
                }
            }
            Err(e) => tracing::warn!(
                "auto-start {} forward {}:{} failed: {e}",
                f.kind,
                f.bind_addr,
                f.bind_port
            ),
        }
    }
}
