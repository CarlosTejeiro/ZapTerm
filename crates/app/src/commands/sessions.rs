use std::collections::HashMap;
use std::io::{Read, Write as _};
use std::sync::{mpsc, Arc, Mutex, RwLock};

use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use core_persistence::SavedSession;
use core_transport::{SerialTransport, SessionCmd, TelnetTransport};

use crate::error::AppError;
use crate::login_script::{LoginRunner, LoginStep};
use crate::state::{ActiveLog, ActiveSession, AppState, SharedPlatformDetector};

use core_hints::detector::PlatformDetector;
use core_hints::Platform;

/// Shared, late-filled slot so `make_on_data` can route bytes to a
/// [`LoginRunner`] that is only constructed AFTER `start_io_loop` yields its
/// command sender. For sessions without a script we use an always-empty slot.
type LoginRunnerSlot = Arc<Mutex<Option<Arc<LoginRunner>>>>;

fn empty_login_slot() -> LoginRunnerSlot {
    Arc::new(Mutex::new(None))
}

type TriggerRunnerSlot = Arc<Mutex<Option<Arc<crate::triggers::TriggerRunner>>>>;

fn empty_trigger_slot() -> TriggerRunnerSlot {
    Arc::new(Mutex::new(None))
}

/// Frontend payload for `session-platform-detected`.
#[derive(serde::Serialize, Clone)]
struct SessionPlatformDetected {
    session_id: String,
    platform: String,
    display_name: String,
}

/// Frontend payload for `connection-progress` — human-readable status lines
/// shown in the terminal while a (possibly multi-hop / ProxyJump) SSH
/// connection is being established, so the user can see it reach the jump
/// host(s) and then tunnel to the target instead of staring at a blank pane.
/// Tagged with the saved-session id the frontend is opening so the right pane
/// picks the lines up (the live session id doesn't exist yet at connect time).
#[derive(serde::Serialize, Clone)]
struct ConnectionProgress {
    saved_session_id: i64,
    line: String,
}

/// Emit a `connection-progress` line (best-effort — a dropped status line must
/// never fail a connection).
fn emit_progress(app: &AppHandle, saved_session_id: i64, line: impl Into<String>) {
    let _ = app.emit(
        "connection-progress",
        ConnectionProgress {
            saved_session_id,
            line: line.into(),
        },
    );
}

/// Build the callback the platform detector fires the first time it latches.
/// Persists the platform onto the saved-session row IF (a) the session was
/// saved and (b) it doesn't already have an explicit platform — we never
/// override a user choice. Always emits `session-platform-detected` so the
/// frontend can react even for ephemeral sessions.
fn make_detection_callback(
    app: AppHandle,
    session_id: String,
    saved_session_id: Option<i64>,
) -> Arc<dyn Fn(Platform) + Send + Sync + 'static> {
    Arc::new(move |platform: Platform| {
        if let Some(id) = saved_session_id {
            let state = app.state::<AppState>();
            let current = state.db.get_session_platform(id).ok().flatten();
            let should_write = matches!(current.as_deref(), None | Some("") | Some("generic"));
            if should_write {
                if let Err(e) = state.db.set_session_platform(id, platform.as_str()) {
                    tracing::warn!(
                        saved_session_id = id,
                        "persist detected platform failed: {e}"
                    );
                }
            }
        }
        let _ = app.emit(
            "session-platform-detected",
            SessionPlatformDetected {
                session_id: session_id.clone(),
                platform: platform.as_str().to_string(),
                display_name: platform.display_name().to_string(),
            },
        );
    })
}

/// Allocate the per-session detector. Cheap; one allocation + a small Vec.
fn new_detector() -> SharedPlatformDetector {
    Arc::new(Mutex::new(PlatformDetector::new()))
}

fn parse_login_script(json: Option<&str>) -> Option<Vec<LoginStep>> {
    let raw = json?;
    let steps: Vec<LoginStep> = serde_json::from_str(raw).ok()?;
    if steps.is_empty() {
        None
    } else {
        Some(steps)
    }
}

/// Commands a login runner sent before the session's I/O loop existed; hand
/// it to [`link_login_runner`] once `start_io_loop` returns the real sender.
type LoginLink = tokio::sync::mpsc::UnboundedReceiver<core_transport::SessionCmd>;

/// Build and store a [`LoginRunner`] into `slot` (no-op if `script_json` is
/// `None` or parses to an empty array). The runner forwards progress events
/// to the frontend as `login-script-progress`.
///
/// Call it BEFORE `start_io_loop`: the reader starts at once, and a device
/// that prints its banner straight away (a Telnet login disclaimer on a fast
/// link) would otherwise send the text the first `expect` waits for before
/// the runner is there to see it — and the login script would stall. The
/// runner's commands queue on the returned link until [`link_login_runner`]
/// connects it to the session.
fn maybe_install_login_runner(
    slot: &LoginRunnerSlot,
    script_json: Option<&str>,
    session_id: &str,
    app: &AppHandle,
) -> Option<LoginLink> {
    let steps = parse_login_script(script_json)?;
    let (cmd_tx, link) = tokio::sync::mpsc::unbounded_channel();
    let app_for_progress = app.clone();
    let runner = Arc::new(LoginRunner::new(
        session_id.to_string(),
        steps,
        cmd_tx,
        Box::new(move |p| {
            let _ = app_for_progress.emit("login-script-progress", p);
        }),
    ));
    // Watchdog: `feed` only advances the deadline when bytes arrive, so a
    // server that stays completely silent would never trip the per-step
    // timeout. Poll the runner on a timer until the script finishes
    // (completes or times out), then let the task end.
    let watch = Arc::clone(&runner);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if watch.check_timeout() {
                break;
            }
        }
    });
    *slot.lock().unwrap() = Some(runner);
    Some(link)
}

/// Forward a login runner's commands (see [`maybe_install_login_runner`]) to
/// the session. Ends when the runner is dropped or the session closes.
fn link_login_runner(
    link: Option<LoginLink>,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<core_transport::SessionCmd>,
) {
    let Some(mut link) = link else { return };
    let cmd_tx = cmd_tx.clone();
    tokio::spawn(async move {
        while let Some(cmd) = link.recv().await {
            if cmd_tx.send(cmd).is_err() {
                break;
            }
        }
    });
}

fn parse_triggers(json: Option<&str>) -> Option<Vec<crate::triggers::Trigger>> {
    let raw = json?;
    let triggers: Vec<crate::triggers::Trigger> = serde_json::from_str(raw).ok()?;
    if triggers.is_empty() {
        None
    } else {
        Some(triggers)
    }
}

/// Build and store a [`TriggerRunner`](crate::triggers::TriggerRunner) into
/// `slot` (no-op if `triggers_json` is `None`/empty). `notify`/`bell` fires are
/// forwarded to the frontend as the `trigger-fired` event.
fn maybe_install_trigger_runner(
    slot: &TriggerRunnerSlot,
    triggers_json: Option<&str>,
    session_id: &str,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<core_transport::SessionCmd>,
    app: &AppHandle,
) {
    let Some(triggers) = parse_triggers(triggers_json) else {
        return;
    };
    let app_for_fire = app.clone();
    let runner = Arc::new(crate::triggers::TriggerRunner::new(
        session_id.to_string(),
        triggers,
        cmd_tx,
        Box::new(move |e| {
            let _ = app_for_fire.emit("trigger-fired", e);
        }),
    ));
    *slot.lock().unwrap() = Some(runner);
}

/// Wrap an `on_data` callback to:
/// 1. Write raw bytes to the session log (if logging is active).
/// 2. Feed the (raw, pre-highlight) bytes to the login-script runner, if any.
/// 3. Feed the bytes to the platform detector (local shells have none); fire the
///    `on_platform_detected` callback the first time it latches.
/// 4. Apply keyword highlighting.
/// 5. Emit highlighted bytes to the frontend.
#[allow(clippy::too_many_arguments)]
fn make_on_data<F>(
    session_id: String,
    loggers: Arc<Mutex<HashMap<String, ActiveLog>>>,
    highlighter: Arc<RwLock<core_highlight::Highlighter>>,
    login_slot: LoginRunnerSlot,
    trigger_slot: TriggerRunnerSlot,
    rx_total: Arc<std::sync::atomic::AtomicU64>,
    detector: Option<crate::state::SharedPlatformDetector>,
    on_platform_detected: Option<Arc<dyn Fn(core_hints::Platform) + Send + Sync + 'static>>,
    inner: F,
) -> impl Fn(Vec<u8>) + Send + 'static
where
    F: Fn(Vec<u8>) + Send + 'static,
{
    move |data: Vec<u8>| {
        // Count raw bytes (pre-highlight) — the StatusBar shows the wire rate.
        rx_total.fetch_add(data.len() as u64, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut map) = loggers.lock() {
            if let Some(active) = map.get_mut(&session_id) {
                let _ = active.logger.write(&data);
            }
        }
        // Feed the login runner (if any) BEFORE highlight so pattern matching
        // sees the raw bytes the server actually sent.
        let runner = login_slot.lock().unwrap().clone();
        if let Some(r) = runner {
            r.feed(&data);
        }
        // Output triggers see the same raw bytes (pre-highlight).
        let trig = trigger_slot.lock().unwrap().clone();
        if let Some(t) = trig {
            t.feed(&data);
        }
        // Platform auto-detect (SSH, Telnet, serial). `feed` returns
        // `Some` ONCE, at the moment of detection, then `None` forever; we
        // pipe that single hit to the host's persistence + emit callback.
        if let Some(d) = detector.as_ref() {
            let latched = d.lock().ok().and_then(|mut d| d.feed(&data));
            if let (Some(p), Some(cb)) = (latched, on_platform_detected.as_ref()) {
                cb(p);
            }
        }
        let highlighted = highlighter.read().unwrap().apply(&data);
        inner(highlighted);
    }
}

/// Spawn a task that emits `session-stats { session_id, bytes_per_sec }`
/// every second. Returns the JoinHandle so the session close path can abort
/// it (otherwise the task would keep ticking for a dead UUID).
fn spawn_stats_emitter(
    app: AppHandle,
    session_id: String,
    rx_total: Arc<std::sync::atomic::AtomicU64>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = 0u64;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
        tick.tick().await; // discard the immediate first tick
        loop {
            tick.tick().await;
            let total = rx_total.load(std::sync::atomic::Ordering::Relaxed);
            let delta = total.saturating_sub(last);
            last = total;
            let _ = app.emit(
                "session-stats",
                SessionStatsPayload {
                    session_id: session_id.clone(),
                    bytes_per_sec: delta,
                },
            );
        }
    })
}

#[derive(serde::Serialize, Clone)]
struct SessionStatsPayload {
    session_id: String,
    bytes_per_sec: u64,
}

// ---------------------------------------------------------------------------
// Local PTY session
// ---------------------------------------------------------------------------

/// Settings key of the local shell: empty/absent = automatic, `pwsh`,
/// `powershell`, `cmd`, or a shell path/command.
pub const LOCAL_SHELL_SETTING: &str = "terminal.localShell";

/// What [`open_local_session`] returns: the session id and the shell that
/// actually started (the preferred one may have fallen back).
#[derive(serde::Serialize)]
pub struct LocalSessionInfo {
    pub id: String,
    pub shell: String,
    /// Shells that couldn't start before `shell`, as "shell: error".
    pub failed: Vec<String>,
}

/// Spawn a local PTY session with the configured shell.
#[tauri::command]
pub async fn open_local_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LocalSessionInfo, AppError> {
    let session_id = Uuid::new_v4().to_string();

    let preferred = state.db.get_setting(LOCAL_SHELL_SETTING).ok().flatten();
    let pty = core_transport::LocalPty::spawn_with(80, 24, preferred.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let shell = pty.shell().to_string();
    let failed = pty.failed_shells().to_vec();
    if !failed.is_empty() {
        tracing::warn!(shell, failed = %failed.join("; "), "local shell fell back");
    }

    start_local_pty(app, &state, session_id.clone(), pty)?;
    tracing::debug!(session_id, shell, "local session opened");
    Ok(LocalSessionInfo {
        id: session_id,
        shell,
        failed,
    })
}

/// Wire a spawned local PTY (local tab or saved local/WSL session) to the
/// frontend under `session_id`: input/resize bridge, reader thread, registry.
fn start_local_pty(
    app: AppHandle,
    state: &AppState,
    session_id: String,
    pty: core_transport::LocalPty,
) -> Result<(), AppError> {
    let reader = pty
        .take_reader()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let pty_writer = pty.writer();
    let (resize_tx, resize_rx) = mpsc::channel::<(u16, u16)>();

    // Bridge: tokio UnboundedReceiver → sync PTY writer / resize channel.
    // Small PTY writes (keystrokes) are fast enough to do synchronously here.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<SessionCmd>();
    // Closing the tab drops `cmd_tx`: then end the shell, which also unblocks
    // the reader thread below. `closed_by_app` tells that thread not to report
    // it as the shell exiting on its own.
    let mut killer = pty.killer();
    let closed_by_app = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let closed_flag = Arc::clone(&closed_by_app);
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                SessionCmd::Data(data) => {
                    if let Ok(mut w) = pty_writer.lock() {
                        let _ = w.write_all(&data);
                    }
                }
                SessionCmd::Resize { cols, rows } => {
                    let _ = resize_tx.send((cols, rows));
                }
            }
        }
        closed_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = killer.kill();
    });

    // Reader loop runs on a blocking thread (PTY reads are synchronous).
    let sid = session_id.clone();
    let app_handle = app.clone();
    tokio::task::spawn_blocking(move || {
        reader_loop(reader, pty, resize_rx, sid, app_handle, closed_by_app);
    });

    // Local PTY doesn't route through make_on_data, so we don't track
    // throughput for it yet. The StatusBar will simply show 0 B/s.
    let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stats_task: Option<tokio::task::JoinHandle<()>> = None;

    state.sessions.lock().unwrap().insert(
        session_id.clone(),
        ActiveSession {
            cmd_tx,
            cols: 80,
            rows: 24,
            ssh_handle: None,
            remote_forwards: None,
            platform_detector: None,
            tcp_mss: core_transport::TcpMss::default(),
            sftp: None,
            rx_total,
            stats_task,
            mss_task: None,
        },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// SSH session
// ---------------------------------------------------------------------------

/// How the frontend specifies SSH authentication. Discriminated by `type`.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthMethodArg {
    Password {
        password: String,
    },
    Key {
        #[serde(rename = "keyPath")]
        key_path: String,
        passphrase: Option<String>,
    },
    #[serde(rename = "keyboard-interactive")]
    KeyboardInteractive,
    Agent,
    /// Reference an existing reusable vault credential by id. No secret is
    /// carried; the saved session just points at the credential row.
    #[serde(rename = "vaultentry")]
    VaultEntry {
        #[serde(rename = "credentialId")]
        credential_id: i64,
    },
}

/// Event payload for `ssh-ki-prompt`. The frontend renders the prompts and
/// then calls `respond_keyboard_interactive` with the matching `interactionId`.
#[derive(serde::Serialize, Clone)]
struct KiPromptPayload {
    #[serde(rename = "interactionId")]
    interaction_id: String,
    name: String,
    instructions: String,
    prompts: Vec<core_transport::KiPrompt>,
}

/// Build a keyboard-interactive responder that bridges to the UI: for each
/// `InfoRequest` from the server, emit a `ssh-ki-prompt` event with a fresh
/// interaction id and await a oneshot fired by [`respond_keyboard_interactive`].
fn build_ki_responder(
    app: AppHandle,
    ki_pending: crate::state::KiPending,
) -> core_transport::KiResponder {
    Arc::new(move |req: core_transport::KiRequest| {
        let app = app.clone();
        let ki_pending = Arc::clone(&ki_pending);
        Box::pin(async move {
            let interaction_id = Uuid::new_v4().to_string();
            let (tx, rx) = tokio::sync::oneshot::channel::<Vec<String>>();
            ki_pending
                .lock()
                .unwrap()
                .insert(interaction_id.clone(), tx);
            let _ = app.emit(
                "ssh-ki-prompt",
                KiPromptPayload {
                    interaction_id: interaction_id.clone(),
                    name: req.name,
                    instructions: req.instructions,
                    prompts: req.prompts,
                },
            );
            match rx.await {
                Ok(responses) => responses,
                Err(_) => {
                    // Sender dropped (e.g. session torn down) — clean up and
                    // return an empty Vec which will fail auth cleanly.
                    ki_pending.lock().unwrap().remove(&interaction_id);
                    Vec::new()
                }
            }
        })
    })
}

/// Convert an [`AuthMethodArg`] from the frontend into a [`core_transport::SshAuth`].
fn build_ssh_auth(
    arg: AuthMethodArg,
    app: &AppHandle,
    ki_pending: &crate::state::KiPending,
    state: &AppState,
) -> Result<core_transport::SshAuth, AppError> {
    Ok(match arg {
        AuthMethodArg::Password { password } => core_transport::SshAuth::Password(password),
        AuthMethodArg::Key {
            key_path,
            passphrase,
        } => core_transport::SshAuth::PublicKey {
            key_path,
            passphrase,
        },
        AuthMethodArg::KeyboardInteractive => core_transport::SshAuth::KeyboardInteractive {
            responder: build_ki_responder(app.clone(), Arc::clone(ki_pending)),
        },
        AuthMethodArg::Agent => core_transport::SshAuth::Agent {
            priority: read_agent_priority(state),
        },
        AuthMethodArg::VaultEntry { credential_id } => {
            // Resolve the reusable credential's secret from the keyring for an
            // ephemeral connect. Saved sessions take the credential_id path
            // (resolve_ssh_auth_for_session) instead. A failed lookup is a real
            // error — don't silently auth with an empty password.
            let secret = state
                .db
                .get_credential_keyring_key(credential_id)
                .ok()
                .and_then(|key| {
                    crate::secrets::resolve(
                        &state.db,
                        &state.vault_seed,
                        &key,
                        crate::secrets::Fallback::Credential(credential_id),
                        state.data_dir.is_portable(),
                    )
                })
                .ok_or_else(|| {
                    AppError::Internal(format!(
                        "vault credential {credential_id} unavailable (keyring locked or entry deleted)"
                    ))
                })?;
            core_transport::SshAuth::Password(secret)
        }
    })
}

/// Read the persisted `ssh.agent_priority` setting and resolve to an enum.
/// Falls back to `Auto` (today's behaviour) if the row is missing or invalid.
fn read_agent_priority(state: &AppState) -> core_transport::AgentPriority {
    let raw = state
        .db
        .get_setting("ssh.agent_priority")
        .ok()
        .flatten()
        .unwrap_or_default();
    core_transport::AgentPriority::from_setting(&raw)
}

/// Fire the user's answers into the oneshot for an in-flight KI prompt.
#[tauri::command]
pub async fn respond_keyboard_interactive(
    state: State<'_, AppState>,
    interaction_id: String,
    responses: Vec<String>,
) -> Result<(), AppError> {
    let sender = state.ki_pending.lock().unwrap().remove(&interaction_id);
    if let Some(tx) = sender {
        let _ = tx.send(responses);
    }
    Ok(())
}

/// Fetch the server's host key and classify it against `~/.ssh/known_hosts`.
/// Run before [`open_ssh_session`] so the UI can prompt for TOFU/mismatch.
#[tauri::command]
pub async fn ssh_preflight_host_key(
    host: String,
    port: u16,
) -> Result<core_transport::HostKeyStatus, AppError> {
    core_transport::preflight_host_key(host, port)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Persist trust for a host key (writes `~/.ssh/known_hosts`).
///
/// `fingerprint` is the value the UI showed and the user approved at preflight;
/// the transport refuses to learn the key if the server now presents a
/// different one (TOCTOU guard).
#[tauri::command]
pub async fn ssh_trust_host_key(
    host: String,
    port: u16,
    fingerprint: String,
) -> Result<(), AppError> {
    core_transport::trust_host_key(host, port, fingerprint)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Overwrite a CHANGED host key: forget the stale `known_hosts` entry and trust
/// the key the server now presents (the "server was reinstalled / factory-reset"
/// case). `fingerprint` is the value the UI showed and the user approved; the
/// transport re-captures the key and refuses if it no longer matches (TOCTOU
/// guard, same as [`ssh_trust_host_key`]).
#[tauri::command]
pub async fn ssh_overwrite_host_key(
    host: String,
    port: u16,
    fingerprint: String,
) -> Result<(), AppError> {
    core_transport::overwrite_host_key(host, port, fingerprint)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Connect to an SSH host, open a PTY shell, and return the session UUID.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn open_ssh_session(
    app: AppHandle,
    state: State<'_, AppState>,
    host: String,
    port: u16,
    user: String,
    auth: AuthMethodArg,
    cols: u16,
    rows: u16,
) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();

    let auth = build_ssh_auth(auth, &app, &state.ki_pending, &state)?;
    let mut transport = core_transport::SshTransport::open_shell(
        host.clone(),
        port,
        user.clone(),
        auth,
        cols,
        rows,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let sid = session_id.clone();
    let app_handle = app.clone();
    let hl = Arc::clone(&state.highlighter);
    let lg = Arc::clone(&state.loggers);
    let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let detector = new_detector();
    // Ephemeral SSH (open_ssh_session, no saved id) — emit only, don't persist.
    let on_detected = make_detection_callback(app.clone(), session_id.clone(), None);
    let tcp_mss = transport.tcp_mss();
    let mss_watcher = transport.take_mss_watcher();
    let (cmd_tx, ssh_handle, remote_forwards) = transport.start_io_loop(
        make_on_data(
            session_id.clone(),
            lg,
            hl,
            empty_login_slot(),
            empty_trigger_slot(),
            Arc::clone(&rx_total),
            Some(Arc::clone(&detector)),
            Some(on_detected),
            move |data| {
                let _ = app_handle.emit(
                    "terminal-data",
                    TerminalDataPayload {
                        session_id: sid.clone(),
                        data,
                    },
                );
            },
        ),
        make_on_close(app.clone(), session_id.clone()),
    );
    let stats_task = Some(spawn_stats_emitter(
        app.clone(),
        session_id.clone(),
        Arc::clone(&rx_total),
    ));
    // Live MSS polling for the same session; aborted when the session closes.
    let mss_task = mss_watcher.map(|w| spawn_mss_emitter(app.clone(), session_id.clone(), w));

    state.sessions.lock().unwrap().insert(
        session_id.clone(),
        ActiveSession {
            cmd_tx,
            cols,
            rows,
            ssh_handle: Some(ssh_handle),
            remote_forwards: Some(remote_forwards),
            platform_detector: Some(detector),
            tcp_mss,
            sftp: Some(core_transport::empty_sftp_slot()),
            rx_total,
            stats_task,
            mss_task,
        },
    );

    tracing::debug!(session_id, host, port, user, "ssh session opened");
    Ok(session_id)
}

// ---------------------------------------------------------------------------
// Shared session commands (work for both local and SSH)
// ---------------------------------------------------------------------------

/// TCP MSS snapshot for `session_id`. Empty fields are surfaced as-is
/// (`null` over the wire) — the frontend renders a "n/a" badge for
/// platforms that don't expose a given direction.
#[tauri::command]
pub async fn get_session_tcp_mss(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<core_transport::TcpMss, AppError> {
    let sessions = state.sessions.lock().unwrap();
    sessions
        .get(&session_id)
        .map(|s| s.tcp_mss)
        .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))
}

/// Background task: poll the per-session MSS watcher every `INTERVAL` and
/// emit a `session-mss-updated { session_id, mss }` event when the value
/// changes. Aborts itself when:
///   * the value goes back to `default()` (socket closed or peer hung up),
///   * the underlying watcher rejected the query several times in a row,
///   * the host drops the JoinHandle (session close path).
fn spawn_mss_emitter(
    app: AppHandle,
    session_id: String,
    watcher: core_transport::MssWatcher,
) -> tokio::task::JoinHandle<()> {
    use core_transport::TcpMss;
    const INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
    const MAX_EMPTY: u32 = 3;

    tokio::spawn(async move {
        let mut last: TcpMss = watcher.query();
        let mut empty_streak: u32 = 0;
        let mut tick = tokio::time::interval(INTERVAL);
        // First tick fires immediately; we already emitted via the connect
        // snapshot, so skip one.
        tick.tick().await;

        loop {
            tick.tick().await;
            let now = watcher.query();
            let empty = now.send.is_none() && now.recv.is_none();
            if empty {
                empty_streak += 1;
                if empty_streak >= MAX_EMPTY {
                    break;
                }
                continue;
            }
            empty_streak = 0;
            if now.send != last.send || now.recv != last.recv {
                last = now;
                let _ = app.emit(
                    "session-mss-updated",
                    SessionMssPayload {
                        session_id: session_id.clone(),
                        mss: now,
                    },
                );
            }
        }
    })
}

#[derive(serde::Serialize, Clone)]
struct SessionMssPayload {
    session_id: String,
    mss: core_transport::TcpMss,
}

/// Write keyboard input to the session.
#[tauri::command]
pub async fn send_input(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), AppError> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))?;
    session.cmd_tx.send(SessionCmd::Data(data)).ok();
    Ok(())
}

/// Notify the session of a terminal resize.
#[tauri::command]
pub async fn resize_terminal(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| AppError::Internal(format!("unknown session: {session_id}")))?;
    session.cmd_tx.send(SessionCmd::Resize { cols, rows }).ok();
    session.cols = cols;
    session.rows = rows;
    Ok(())
}

/// Terminate a session and clean up its resources.
///
/// Dropping `cmd_tx` signals the I/O task to exit cleanly.
#[tauri::command]
pub async fn close_session(state: State<'_, AppState>, session_id: String) -> Result<(), AppError> {
    let removed = state.sessions.lock().unwrap().remove(&session_id);
    // Drop any port-forwards bound to this session — Drop on ForwardController
    // aborts each listener task.
    state.forwards.lock().unwrap().remove(&session_id);
    // Finalise a still-attached logger so closing a tab mid-capture doesn't
    // leak the file handle or leave an open `session_logs` row. Best-effort:
    // a failure here must not block the close.
    if let Err(e) = crate::commands::logging::finalize_active_log(&state, &session_id) {
        tracing::warn!(session_id, "finalize log on close failed: {e}");
    }
    if let Some(active) = removed {
        if let Some(task) = active.stats_task {
            task.abort();
        }
        if let Some(task) = active.mss_task {
            task.abort();
        }
        tracing::debug!(session_id, "session closed");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal — PTY reader loop (blocking thread)
// ---------------------------------------------------------------------------

fn reader_loop(
    mut reader: Box<dyn Read + Send>,
    mut pty: core_transport::LocalPty,
    resize_rx: mpsc::Receiver<(u16, u16)>,
    session_id: String,
    app: AppHandle,
    closed_by_app: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut buf = vec![0u8; 4096];

    loop {
        while let Ok((cols, rows)) = resize_rx.try_recv() {
            if let Err(e) = pty.resize(cols, rows) {
                tracing::warn!(session_id, "resize failed: {e}");
            }
        }

        if pty.is_done() {
            break;
        }

        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let payload = TerminalDataPayload {
                    session_id: session_id.clone(),
                    data: buf[..n].to_vec(),
                };
                if app.emit("terminal-data", payload).is_err() {
                    break;
                }
            }
            Err(e) => {
                tracing::debug!(session_id, "PTY read error: {e}");
                break;
            }
        }
    }

    pty.kill();
    tracing::debug!(session_id, "reader loop exited");
    // The shell exited by itself (`exit`, Ctrl-D): tell the tab.
    if !closed_by_app.load(std::sync::atomic::Ordering::SeqCst) {
        emit_disconnected(&app, session_id, core_transport::CloseReason::Ended);
    }
}

// ---------------------------------------------------------------------------
// Saved session CRUD (persisted in SQLite via core-persistence)
// ---------------------------------------------------------------------------

/// Save a new SSH session. Secrets (password, key passphrase) go to the OS
/// keyring; the key file path is not a secret and is stored in `options_json`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_saved_session(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<i64>,
    host: String,
    port: u16,
    username: String,
    auth: AuthMethodArg,
    via_session_id: Option<i64>,
) -> Result<i64, AppError> {
    // Sanity-check the bastion reference now so we fail fast on create, not
    // only when opening the connection.
    if let Some(via_id) = via_session_id {
        let bastion = state
            .db
            .get_session(via_id)
            .map_err(|e| AppError::Internal(format!("via_session_id {via_id}: {e}")))?;
        if bastion.protocol != "ssh" {
            return Err(AppError::Internal(format!(
                "via_session_id {via_id} is not an SSH session"
            )));
        }
    }
    // Portable installs deliberately don't depend on the OS keyring (see
    // data_dir docs); the AES-256-GCM `session_secrets` fallback is the store.
    let portable = state.data_dir.is_portable();

    // Whether the OS keyring took the secret (else the encrypted database copy
    // below holds it).
    let mut in_keyring = true;
    // (auth_method, credential_id, options_json, password_to_cache) derived per type.
    let (auth_method, credential_id, options_json, password_to_cache): (
        &str,
        Option<i64>,
        String,
        Option<String>,
    ) = match auth {
        AuthMethodArg::Password { password } => {
            let keyring_key = format!("ssh:{}", Uuid::new_v4());
            in_keyring = crate::secrets::keyring_put(&keyring_key, &password);
            let cred_id = state
                .db
                .create_credential(&name, "ssh_password", Some(&username), &keyring_key)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            ("password", Some(cred_id), "{}".to_string(), Some(password))
        }
        AuthMethodArg::Key {
            key_path,
            passphrase,
        } => {
            // Only the passphrase is a secret; store it in the vault if present.
            let (cred_id, pp_to_persist) = match passphrase {
                Some(pp) if !pp.is_empty() => {
                    let keyring_key = format!("ssh:{}", Uuid::new_v4());
                    in_keyring = crate::secrets::keyring_put(&keyring_key, &pp);
                    let id = state
                        .db
                        .create_credential(&name, "ssh_key", Some(&username), &keyring_key)
                        .map_err(|e| AppError::Internal(e.to_string()))?;
                    (Some(id), Some(pp))
                }
                _ => (None, None),
            };
            let options = serde_json::json!({ "key_path": key_path }).to_string();
            ("key", cred_id, options, pp_to_persist)
        }
        AuthMethodArg::KeyboardInteractive => {
            ("keyboard-interactive", None, "{}".to_string(), None)
        }
        AuthMethodArg::Agent => ("agent", None, "{}".to_string(), None),
        AuthMethodArg::VaultEntry { credential_id } => {
            // Reuse an existing vault credential: no keyring write, just point
            // the session at the reusable row. resolve_ssh_auth_for_session
            // pulls the secret by credential_id on connect.
            ("password", Some(credential_id), "{}".to_string(), None)
        }
    };

    let id = state
        .db
        .create_session_full(
            folder_id,
            &name,
            "ssh",
            Some(&host),
            Some(port),
            Some(&username),
            credential_id,
            &options_json,
            Some(auth_method),
            via_session_id,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Seed the in-process memory cache so the first reopen needs no keyring
    // round-trip, and keep the encrypted database copy only where the policy
    // in `crate::secrets` wants it (keyring unavailable, or a portable
    // install) — never a second at-rest copy when the keyring holds it.
    if let Some(pw) = password_to_cache {
        state.password_cache.lock().unwrap().insert(id, pw.clone());
        crate::secrets::settle_fallback(
            &state.db,
            &state.vault_seed,
            crate::secrets::Fallback::Session(id),
            &pw,
            in_keyring,
            portable,
        )
        .map_err(AppError::Internal)?;
    }

    Ok(id)
}

/// List all saved sessions.
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SavedSession>, AppError> {
    state
        .db
        .list_sessions()
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Update editable fields of a saved session. Credentials (auth method,
/// password, key passphrase) are intentionally NOT touched — change them by
/// recreating the session.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_saved_session(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    folder_id: Option<i64>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    via_session_id: Option<i64>,
    // `options_json`: protocol-specific options blob (e.g. ssh `key_path`,
    // serial `baud_rate`). Passed through to the DB when `Some`, preserved
    // otherwise — see persistence::update_session_fields.
    options_json: Option<String>,
) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Internal("name is required".into()));
    }
    // Cycle / self-reference check for via_session_id.
    if let Some(vid) = via_session_id {
        if vid == id {
            return Err(AppError::Internal(
                "session cannot use itself as a jump host".into(),
            ));
        }
        let bastion = state
            .db
            .get_session(vid)
            .map_err(|e| AppError::Internal(format!("via_session_id {vid}: {e}")))?;
        if bastion.protocol != "ssh" {
            return Err(AppError::Internal(format!(
                "via_session_id {vid} is not an SSH session"
            )));
        }
    }
    state
        .db
        .update_session_fields(
            id,
            name.trim(),
            folder_id,
            host.as_deref(),
            port,
            username.as_deref(),
            via_session_id,
            options_json.as_deref(),
        )
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Change the credential of an existing SSH session in place — the editable
/// counterpart to the credential handling in [`create_saved_session`]. Lets the
/// Edit dialog switch auth method, rotate a password, or re-point at a different
/// vault entry without deleting and recreating the session.
///
/// Secret lifecycle mirrors create: a typed password/passphrase is written to a
/// fresh private credential (keyring key `ssh:<uuid>`); a `VaultEntry` just
/// points the session at the shared row (no keyring write). Crucially, the old
/// credential is cleaned up **only when it was private** — a shared vault entry
/// (`reusable=1`) is never deleted or mutated here, so typing a new password on
/// a vault-backed session detaches it into its own private secret while the
/// vault entry stays intact for every other session using it.
#[tauri::command]
pub async fn set_session_credential(
    state: State<'_, AppState>,
    id: i64,
    auth: AuthMethodArg,
) -> Result<(), AppError> {
    let session = state
        .db
        .get_session(id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let old_cred_id = session.credential_id;
    let username = session.username.as_deref();
    let portable = state.data_dir.is_portable();

    // Derive the new (auth_method, credential_id, password_to_cache) using the
    // same rules as create_saved_session. `key_path` is not a secret and keeps
    // travelling through options_json via update_saved_session, so it is not
    // handled here.
    let mut in_keyring = true;
    let (auth_method, new_credential_id, password_to_cache): (&str, Option<i64>, Option<String>) =
        match auth {
            AuthMethodArg::Password { password } => {
                let keyring_key = format!("ssh:{}", Uuid::new_v4());
                in_keyring = crate::secrets::keyring_put(&keyring_key, &password);
                let cred_id = state
                    .db
                    .create_credential(&session.name, "ssh_password", username, &keyring_key)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                ("password", Some(cred_id), Some(password))
            }
            AuthMethodArg::Key {
                key_path: _,
                passphrase,
            } => {
                let cred_id = match passphrase {
                    Some(pp) if !pp.is_empty() => {
                        let keyring_key = format!("ssh:{}", Uuid::new_v4());
                        in_keyring = crate::secrets::keyring_put(&keyring_key, &pp);
                        Some(
                            state
                                .db
                                .create_credential(&session.name, "ssh_key", username, &keyring_key)
                                .map_err(|e| AppError::Internal(e.to_string()))?,
                        )
                    }
                    _ => None,
                };
                ("key", cred_id, None)
            }
            AuthMethodArg::KeyboardInteractive => ("keyboard-interactive", None, None),
            AuthMethodArg::Agent => ("agent", None, None),
            AuthMethodArg::VaultEntry { credential_id } => ("password", Some(credential_id), None),
        };

    state
        .db
        .set_session_auth(id, new_credential_id, auth_method)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Purge the previous credential when it is no longer referenced and was
    // private to this session. Reusable vault entries are shared — never delete
    // them here; the session simply stops pointing at the row. (Same guard as
    // delete_saved_session.)
    if let Some(old_id) = old_cred_id {
        if Some(old_id) != new_credential_id {
            let reusable = state
                .db
                .get_credential(old_id)
                .map(|c| c.reusable)
                .unwrap_or(false);
            if !reusable {
                if let Ok(key) = state.db.get_credential_keyring_key(old_id) {
                    core_vault::Vault::delete(&key).ok();
                }
                state.db.delete_credential_secret(old_id).ok();
                state.db.delete_credential(old_id).ok();
            }
        }
    }

    // Refresh caches. Drop any stale in-memory secret first so a keyring-denied
    // build can't authenticate the new credential with the old password; then
    // re-seed for the typed-password case exactly as create_saved_session does.
    state.password_cache.lock().unwrap().remove(&id);
    if let Some(pw) = password_to_cache {
        state.password_cache.lock().unwrap().insert(id, pw.clone());
        crate::secrets::settle_fallback(
            &state.db,
            &state.vault_seed,
            crate::secrets::Fallback::Session(id),
            &pw,
            in_keyring,
            portable,
        )
        .map_err(AppError::Internal)?;
    } else {
        // No session-owned secret any more (vault entry, agent, key, or KI):
        // drop a stale portable fallback blob so it can't shadow the new auth.
        state.db.delete_session_secret(id).ok();
    }

    Ok(())
}

/// Move a saved session to a different folder (or to the root with `None`).
#[tauri::command]
pub async fn move_saved_session(
    state: State<'_, AppState>,
    id: i64,
    folder_id: Option<i64>,
) -> Result<(), AppError> {
    state
        .db
        .set_session_folder(id, folder_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Reorder `id` to position `targetIndex` inside `folderId` (`null` for root).
/// Also reparents if `folderId` differs from the row's current value, so the
/// frontend can dispatch a single command for both drop-into-folder and
/// drag-to-reorder gestures.
#[tauri::command]
pub async fn reorder_saved_session(
    state: State<'_, AppState>,
    id: i64,
    folder_id: Option<i64>,
    target_index: usize,
) -> Result<(), AppError> {
    state
        .db
        .reorder_session(id, folder_id, target_index)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Delete a saved session (and its credential from the keyring).
#[tauri::command]
pub async fn delete_saved_session(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    let session = state
        .db
        .get_session(id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if let Some(cred_id) = session.credential_id {
        // Reusable vault credentials are shared across sessions — never delete
        // them here; the FK SET NULL just detaches this session. Only purge a
        // credential that belongs solely to this session.
        let reusable = state
            .db
            .get_credential(cred_id)
            .map(|c| c.reusable)
            .unwrap_or(false);
        if !reusable {
            if let Ok(key) = state.db.get_credential_keyring_key(cred_id) {
                core_vault::Vault::delete(&key).ok();
                state.db.delete_credential(cred_id).ok();
            }
        }
    }

    state
        .db
        .delete_session(id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Duplicate a saved session into an independent copy: its credential is
/// re-stored under a fresh keyring entry, and the login script, triggers and
/// port-forwards are copied too. Because nothing is shared with the original,
/// deleting either session never affects the other. Returns the new id.
#[tauri::command]
pub async fn clone_saved_session(state: State<'_, AppState>, id: i64) -> Result<i64, AppError> {
    let src = state
        .db
        .get_session(id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Give the copy its own credential when the secret is readable; otherwise
    // it starts credential-less and prompts on first connect (key/agent auth
    // with no stored secret). A shared vault entry stays shared.
    let mut cloned: Option<(String, bool)> = None; // (secret, in keyring)
    let credential_id = match src
        .credential_id
        .and_then(|cid| state.db.get_credential(cid).ok())
    {
        Some(c) if c.reusable => Some(c.id),
        Some(c) => crate::secrets::keyring_get(&c.keyring_key)
            .or_else(|| typed_password(&state.db, &state.password_cache, &state.vault_seed, src.id))
            .and_then(|secret| {
                let new_key = format!("ssh:{}", Uuid::new_v4());
                let in_keyring = crate::secrets::keyring_put(&new_key, &secret);
                let id = state
                    .db
                    .create_credential(&c.name, &c.kind, c.username.as_deref(), &new_key)
                    .ok()?;
                cloned = Some((secret, in_keyring));
                Some(id)
            }),
        None => None,
    };

    let new_id = state
        .db
        .create_session_full(
            src.folder_id,
            &format!("{} (copy)", src.name),
            &src.protocol,
            src.host.as_deref(),
            src.port,
            src.username.as_deref(),
            credential_id,
            &src.options_json,
            src.auth_method.as_deref(),
            src.via_session_id,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Per-session automation + forwards live in separate columns/tables.
    if let Some(ls) = src.login_script_json.as_deref() {
        let _ = state.db.set_login_script(new_id, Some(ls));
    }
    if let Ok(Some(tr)) = state.db.get_session_triggers(id) {
        let _ = state.db.set_session_triggers(new_id, Some(&tr));
    }
    if let Ok(forwards) = state.db.list_session_forwards(id) {
        if !forwards.is_empty() {
            let _ = state.db.set_session_forwards(new_id, &forwards);
        }
    }
    // The copied secret: an encrypted database copy too when the keyring
    // couldn't take it (or on a portable install).
    if let Some((secret, in_keyring)) = cloned {
        if let Err(e) = crate::secrets::settle_fallback(
            &state.db,
            &state.vault_seed,
            crate::secrets::Fallback::Session(new_id),
            &secret,
            in_keyring,
            state.data_dir.is_portable(),
        ) {
            tracing::warn!(new_id, "{e}");
        }
    }

    Ok(new_id)
}

/// The password typed for `session_id` in a pane's reconnect form: from the
/// in-memory cache, else from the encrypted copy in the database (which then
/// warms the cache).
fn typed_password(
    db: &core_persistence::Database,
    cache: &crate::state::PasswordCache,
    seed: &str,
    session_id: i64,
) -> Option<String> {
    // Separate statement: the cache guard must be dropped before the fallback
    // below locks the cache again (a same-expression re-lock of a std Mutex
    // deadlocks).
    let cached = cache.lock().unwrap().get(&session_id).cloned();
    if cached.is_some() {
        return cached;
    }
    let pw = db
        .get_session_secret(session_id)
        .ok()
        .flatten()
        .and_then(|blob| core_vault::decrypt_with_seed(seed, &blob).ok());
    if let Some(ref pw) = pw {
        cache.lock().unwrap().insert(session_id, pw.clone());
    }
    pw
}

/// Build the [`core_transport::SshAuth`] for a saved SSH session, pulling its
/// secret from the vault if applicable. Shared by direct connects and every
/// hop of a ProxyJump chain.
fn resolve_ssh_auth_for_session(
    session: &SavedSession,
    app: &AppHandle,
    state: &AppState,
) -> Result<core_transport::SshAuth, AppError> {
    // Resolve the secret through a fallback chain:
    //   1. OS keyring (primary; works in signed/production builds).
    //   2. Process-lifetime in-memory cache (survives Cmd+R).
    //   3. Encrypted local store in SQLite (survives app restart — used on
    //      macOS dev builds where the unsigned binary is denied keyring read).
    // For password auth, steps 2–3 also apply to a session with no stored
    // credential at all (e.g. a MobaXterm import): the password the user typed
    // in the pane's reconnect form lands there via `cache_session_password`.
    // A key's passphrase only comes from there when the session has a
    // credential (a typed *password* must never be tried as a passphrase), and
    // agent / keyboard-interactive sessions need no lookup at all.
    let lookup_secret = |without_credential: bool| -> Option<String> {
        use crate::secrets::{self, Fallback};
        let portable = state.data_dir.is_portable();
        let cred = session
            .credential_id
            .and_then(|id| state.db.get_credential(id).ok());
        if let Some(s) = cred
            .as_ref()
            .and_then(|c| secrets::keyring_get(&c.keyring_key))
        {
            return Some(s);
        }
        if cred.is_none() && !without_credential {
            return None;
        }
        // Typed in a pane, or the session's own encrypted copy. With a private
        // credential and a working keyring, that copy moves into the keyring.
        if let Some(pw) = typed_password(
            &state.db,
            &state.password_cache,
            &state.vault_seed,
            session.id,
        ) {
            if let Some(c) = cred.as_ref().filter(|c| !c.reusable && !portable) {
                if secrets::read_fallback(
                    &state.db,
                    &state.vault_seed,
                    Fallback::Session(session.id),
                )
                .is_some()
                {
                    secrets::migrate_to_keyring(
                        &state.db,
                        &c.keyring_key,
                        Fallback::Session(session.id),
                        &pw,
                    );
                }
            }
            return Some(pw);
        }
        // A vault entry's own encrypted copy (moved into the keyring when it
        // works). Before, a vault-backed session never looked here.
        cred.as_ref().filter(|c| c.reusable).and_then(|c| {
            secrets::resolve(
                &state.db,
                &state.vault_seed,
                &c.keyring_key,
                Fallback::Credential(c.id),
                portable,
            )
        })
    };

    Ok(match session.auth_method.as_deref() {
        Some("key") => {
            let opts: serde_json::Value =
                serde_json::from_str(&session.options_json).unwrap_or(serde_json::json!({}));
            let key_path = opts["key_path"]
                .as_str()
                .ok_or_else(|| AppError::Internal("missing key_path".into()))?
                .to_string();
            core_transport::SshAuth::PublicKey {
                key_path,
                passphrase: lookup_secret(false),
            }
        }
        Some("agent") => core_transport::SshAuth::Agent {
            priority: read_agent_priority(state),
        },
        Some("keyboard-interactive") => core_transport::SshAuth::KeyboardInteractive {
            responder: build_ki_responder(app.clone(), Arc::clone(&state.ki_pending)),
        },
        // None / "password" — legacy behaviour.
        _ => core_transport::SshAuth::Password(
            lookup_secret(true).ok_or_else(|| AppError::Internal("missing credential".into()))?,
        ),
    })
}

/// Persist the user-typed password for `saved_session_id`.
///
/// Writes to BOTH:
///  * the in-memory cache (fast, survives Cmd+R),
///  * the encrypted `session_secrets` table (survives app restart).
///
/// Called by the reconnect dialog after a successful re-auth so the user
/// stops being prompted on every relaunch when the OS keyring is unusable
/// (typical for macOS dev builds without code-signing).
#[tauri::command]
pub async fn cache_session_password(
    state: State<'_, AppState>,
    saved_session_id: i64,
    password: String,
) -> Result<(), AppError> {
    state
        .password_cache
        .lock()
        .unwrap()
        .insert(saved_session_id, password.clone());

    // Keep it for later restarts. A session with its own (private)
    // credential stores it like any saved password — OS keyring first, the
    // encrypted database copy only when needed; one without a credential
    // (e.g. a MobaXterm import) has no keyring entry, so it keeps the
    // encrypted copy. Best effort: a failure is a warning, not an error,
    // so the reconnect still goes ahead with the in-memory copy.
    use crate::secrets::{self, Fallback};
    let target = Fallback::Session(saved_session_id);
    let private_cred = state
        .db
        .get_session(saved_session_id)
        .ok()
        .and_then(|s| s.credential_id)
        .and_then(|id| state.db.get_credential(id).ok())
        .filter(|c| !c.reusable);
    match private_cred {
        Some(c) => {
            if let Err(e) = secrets::persist(
                &state.db,
                &state.vault_seed,
                &c.keyring_key,
                target,
                &password,
                state.data_dir.is_portable(),
            ) {
                tracing::warn!(saved_session_id, "{e}");
            }
        }
        None => {
            secrets::write_fallback(&state.db, &state.vault_seed, target, &password);
        }
    }
    Ok(())
}

/// Where an open of `saved_session_id` gets its password from right now:
/// "keyring" (the OS keyring entry of its credential), "typed" (what was typed
/// in a pane's form, kept in memory or encrypted) or "none". The pane uses it
/// after a rejected password: only a typed one is forgotten and asked again —
/// a wrong keyring entry is fixed in the session dialog.
#[tauri::command]
pub async fn session_password_source(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<&'static str, AppError> {
    let session = state
        .db
        .get_session(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let in_keyring = session.credential_id.is_some_and(|cred_id| {
        state
            .db
            .get_credential_keyring_key(cred_id)
            .ok()
            .and_then(|key| core_vault::Vault::retrieve(&key).ok())
            .is_some()
    });
    if in_keyring {
        return Ok("keyring");
    }
    let typed = typed_password(
        &state.db,
        &state.password_cache,
        &state.vault_seed,
        saved_session_id,
    );
    Ok(if typed.is_some() { "typed" } else { "none" })
}

/// Forget a password typed for `saved_session_id` (memory cache and encrypted
/// copy) — called when it turned out to be wrong, so the next open asks again
/// instead of retrying it. The OS keyring entry, if any, is left alone.
#[tauri::command]
pub async fn forget_session_password(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<(), AppError> {
    state
        .password_cache
        .lock()
        .unwrap()
        .remove(&saved_session_id);
    // A typed password for a session with its own credential may have gone to
    // the OS keyring (see `cache_session_password`): drop that too, or the
    // wrong value would keep winning. A shared vault entry is left alone.
    if let Some(c) = state
        .db
        .get_session(saved_session_id)
        .ok()
        .and_then(|s| s.credential_id)
        .and_then(|id| state.db.get_credential(id).ok())
        .filter(|c| !c.reusable)
    {
        core_vault::Vault::delete(&c.keyring_key).ok();
    }
    state
        .db
        .delete_session_secret(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// The host and port an open of `saved_session_id` dials directly: the first
/// jump host of its chain, or the session itself without one. The tab checks
/// this host's key (preflight) before connecting; hosts further in reach only
/// through the tunnel.
#[tauri::command]
pub async fn ssh_direct_endpoint(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<(String, u16), AppError> {
    let session = state
        .db
        .get_session(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let first = match session.via_session_id {
        Some(via) => {
            let chain = collect_chain_ids(via, &state)?;
            let id = *chain
                .first()
                .ok_or_else(|| AppError::Internal("empty jump host chain".into()))?;
            state
                .db
                .get_session(id)
                .map_err(|e| AppError::Internal(e.to_string()))?
        }
        None => session,
    };
    let host = first
        .host
        .ok_or_else(|| AppError::Internal(format!("«{}» has no host set", first.name)))?;
    Ok((host, first.port.unwrap_or(22)))
}

/// Walk the `via_session_id` chain starting at `start_id` and return the list
/// of saved-session ids from outermost (no `via`) to innermost (just before
/// the target). Detects cycles.
fn collect_chain_ids(start_id: i64, state: &AppState) -> Result<Vec<i64>, AppError> {
    let mut chain_ids = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current = Some(start_id);
    while let Some(id) = current {
        if !visited.insert(id) {
            return Err(AppError::Internal(format!(
                "circular via_session_id chain at session {id}"
            )));
        }
        let saved = state
            .db
            .get_session(id)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if saved.protocol != "ssh" {
            return Err(AppError::Internal(format!(
                "via_session_id {id} points to non-SSH session"
            )));
        }
        current = saved.via_session_id;
        chain_ids.push(id);
    }
    chain_ids.reverse();
    Ok(chain_ids)
}

/// Build the chain of authenticated bastion handles required to reach a
/// target whose `via_session_id` is `Some(via_start)`. The last element of
/// the returned Vec is the bastion through which the target is tunneled.
async fn build_bastion_chain(
    via_start: i64,
    tag_id: i64,
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<core_transport::SshHandle>, AppError> {
    let chain_ids = collect_chain_ids(via_start, state)?;
    let total = chain_ids.len();
    let mut handles: Vec<core_transport::SshHandle> = Vec::with_capacity(total);
    for (i, id) in chain_ids.into_iter().enumerate() {
        let saved = state
            .db
            .get_session(id)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let label = saved.name.clone();
        let host = saved
            .host
            .clone()
            .ok_or_else(|| AppError::Internal(format!("jump host «{label}» has no host set")))?;
        let port = saved.port.unwrap_or(22);
        let user = saved.username.clone().ok_or_else(|| {
            AppError::Internal(format!("jump host «{label}» has no username set"))
        })?;
        // A jump host without a stored password must not read as the target's
        // "missing credential": the pane would ask for the target's password,
        // which can never help.
        let auth = resolve_ssh_auth_for_session(&saved, app, state).map_err(|e| match e {
            AppError::Internal(m) if m == "missing credential" => AppError::Internal(format!(
                "jump host «{label}» has no stored password — open «{label}» in its own tab \
                 and enter it there, or add it to the session"
            )),
            other => other,
        })?;

        emit_progress(
            app,
            tag_id,
            format!(
                "→ [jump {}/{total}] connecting to «{label}» ({host}:{port})…",
                i + 1
            ),
        );

        let handle = if let Some(prev) = handles.last() {
            core_transport::connect_authenticated_via(prev, host.clone(), port, user, auth).await
        } else {
            core_transport::connect_authenticated(host.clone(), port, user, auth).await
        }
        .map_err(|e| {
            let msg = format!("jump host «{label}» ({host}:{port}) unreachable: {e}");
            emit_progress(app, tag_id, format!("✗ {msg}"));
            AppError::Internal(msg)
        })?;

        emit_progress(app, tag_id, format!("✓ jump host «{label}» ready"));
        handles.push(handle);
    }
    Ok(handles)
}

/// Open a saved session: load config from DB, retrieve password from keyring,
/// establish the connection, and return the live session UUID.
#[tauri::command]
pub async fn open_saved_session(
    app: AppHandle,
    state: State<'_, AppState>,
    saved_session_id: i64,
    cols: u16,
    rows: u16,
) -> Result<String, AppError> {
    let session = state
        .db
        .get_session(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Pull the (optional) login script out once so the per-protocol arms can
    // freely move fields out of `session` without holding a borrow.
    let login_script_json = session.login_script_json.clone();
    // Triggers live in a dedicated column (fetched separately, not on the
    // SavedSession struct), so grab them here too.
    let triggers_json = state
        .db
        .get_session_triggers(saved_session_id)
        .ok()
        .flatten();

    match session.protocol.as_str() {
        "ssh" => {
            let host = session
                .host
                .clone()
                .ok_or_else(|| AppError::Internal("missing host".into()))?;
            let port = session.port.unwrap_or(22);
            let username = session
                .username
                .clone()
                .ok_or_else(|| AppError::Internal("missing username".into()))?;

            let auth = resolve_ssh_auth_for_session(&session, &app, &state)?;

            let session_id = Uuid::new_v4().to_string();
            let target_label = session.name.clone();
            let mut transport = if let Some(via_id) = session.via_session_id {
                // Build the bastion chain, then tunnel to the target through it.
                let chain = build_bastion_chain(via_id, saved_session_id, &app, &state).await?;
                emit_progress(
                    &app,
                    saved_session_id,
                    format!(
                        "→ opening tunnel to «{target_label}» ({host}:{port}) via the jump host…"
                    ),
                );
                let t = core_transport::SshTransport::open_shell_via(
                    chain,
                    host.clone(),
                    port,
                    username,
                    auth,
                    cols,
                    rows,
                )
                .await
                .map_err(|e| {
                    let msg = format!(
                        "target «{target_label}» ({host}:{port}) not reachable through the jump host: {e}"
                    );
                    emit_progress(&app, saved_session_id, format!("✗ {msg}"));
                    AppError::Internal(msg)
                })?;
                emit_progress(
                    &app,
                    saved_session_id,
                    format!("✓ connected to «{target_label}»"),
                );
                t
            } else {
                emit_progress(
                    &app,
                    saved_session_id,
                    format!("→ connecting to {host}:{port}…"),
                );
                let t = core_transport::SshTransport::open_shell(
                    host.clone(),
                    port,
                    username,
                    auth,
                    cols,
                    rows,
                )
                .await
                .map_err(|e| {
                    let msg = format!("{host}:{port} — {e}");
                    emit_progress(&app, saved_session_id, format!("✗ {msg}"));
                    AppError::Internal(msg)
                })?;
                emit_progress(
                    &app,
                    saved_session_id,
                    format!("✓ connected to «{target_label}»"),
                );
                t
            };

            let sid = session_id.clone();
            let app_handle = app.clone();
            let hl = Arc::clone(&state.highlighter);
            let lg = Arc::clone(&state.loggers);
            let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let login_slot: LoginRunnerSlot = empty_login_slot();
            let login_slot_for_closure = Arc::clone(&login_slot);
            // Before the I/O loop starts, so it sees the first bytes (see the fn).
            let login_link = maybe_install_login_runner(
                &login_slot,
                login_script_json.as_deref(),
                &session_id,
                &app,
            );
            let trigger_slot: TriggerRunnerSlot = empty_trigger_slot();
            let trigger_slot_for_closure = Arc::clone(&trigger_slot);
            let detector = new_detector();
            // Saved SSH — also persists the detected platform if the row
            // doesn't already have a user-chosen one.
            let on_detected =
                make_detection_callback(app.clone(), session_id.clone(), Some(saved_session_id));
            let tcp_mss = transport.tcp_mss();
            let mss_watcher = transport.take_mss_watcher();
            let (cmd_tx, ssh_handle, remote_forwards) = transport.start_io_loop(
                make_on_data(
                    session_id.clone(),
                    lg,
                    hl,
                    login_slot_for_closure,
                    trigger_slot_for_closure,
                    Arc::clone(&rx_total),
                    Some(Arc::clone(&detector)),
                    Some(on_detected),
                    move |data| {
                        let _ = app_handle.emit(
                            "terminal-data",
                            TerminalDataPayload {
                                session_id: sid.clone(),
                                data,
                            },
                        );
                    },
                ),
                make_on_close(app.clone(), session_id.clone()),
            );

            link_login_runner(login_link, &cmd_tx);
            maybe_install_trigger_runner(
                &trigger_slot,
                triggers_json.as_deref(),
                &session_id,
                cmd_tx.clone(),
                &app,
            );
            let stats_task =
                spawn_stats_emitter(app.clone(), session_id.clone(), Arc::clone(&rx_total));
            // Live MSS polling; aborted when the session closes.
            let mss_task =
                mss_watcher.map(|w| spawn_mss_emitter(app.clone(), session_id.clone(), w));

            // Clone the shared handles before they move into ActiveSession so
            // saved forwards can be auto-started against the same connection.
            let fwd_handle = ssh_handle.clone();
            let fwd_registry = remote_forwards.clone();

            state.sessions.lock().unwrap().insert(
                session_id.clone(),
                ActiveSession {
                    cmd_tx,
                    cols,
                    rows,
                    ssh_handle: Some(ssh_handle),
                    remote_forwards: Some(remote_forwards),
                    platform_detector: Some(detector),
                    tcp_mss,
                    sftp: Some(core_transport::empty_sftp_slot()),
                    rx_total,
                    stats_task: Some(stats_task),
                    mss_task,
                },
            );

            // Auto-start saved port-forwards (best-effort; failures are logged,
            // never fatal to the session open).
            let saved_forwards = state
                .db
                .list_session_forwards(saved_session_id)
                .unwrap_or_default();
            if !saved_forwards.is_empty() {
                crate::commands::forwards::autostart_saved_forwards(
                    &state,
                    &session_id,
                    fwd_handle,
                    fwd_registry,
                    &saved_forwards,
                )
                .await;
            }

            state.db.touch_session(saved_session_id).ok();
            tracing::debug!(session_id, saved_session_id, "saved session opened");
            Ok(session_id)
        }
        "telnet" => {
            let host = session
                .host
                .ok_or_else(|| AppError::Internal("missing host".into()))?;
            let port = session.port.unwrap_or(23);

            let session_id = Uuid::new_v4().to_string();
            let transport = TelnetTransport::connect(host.clone(), port)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let sid = session_id.clone();
            let app_handle = app.clone();
            let hl = Arc::clone(&state.highlighter);
            let lg = Arc::clone(&state.loggers);
            let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let login_slot: LoginRunnerSlot = empty_login_slot();
            let login_slot_for_closure = Arc::clone(&login_slot);
            // Before the I/O loop starts, so it sees the first bytes (see the fn).
            let login_link = maybe_install_login_runner(
                &login_slot,
                login_script_json.as_deref(),
                &session_id,
                &app,
            );
            let trigger_slot: TriggerRunnerSlot = empty_trigger_slot();
            let trigger_slot_for_closure = Arc::clone(&trigger_slot);
            let detector = new_detector();
            // Saved telnet/serial — persist the detected platform unless the row
            // already has a user-chosen one.
            let on_detected =
                make_detection_callback(app.clone(), session_id.clone(), Some(saved_session_id));
            let cmd_tx = transport.start_io_loop(
                cols,
                rows,
                make_on_data(
                    session_id.clone(),
                    lg,
                    hl,
                    login_slot_for_closure,
                    trigger_slot_for_closure,
                    Arc::clone(&rx_total),
                    Some(Arc::clone(&detector)),
                    Some(on_detected),
                    move |data| {
                        let _ = app_handle.emit(
                            "terminal-data",
                            TerminalDataPayload {
                                session_id: sid.clone(),
                                data,
                            },
                        );
                    },
                ),
                make_on_close(app.clone(), session_id.clone()),
            );

            link_login_runner(login_link, &cmd_tx);
            maybe_install_trigger_runner(
                &trigger_slot,
                triggers_json.as_deref(),
                &session_id,
                cmd_tx.clone(),
                &app,
            );
            let stats_task =
                spawn_stats_emitter(app.clone(), session_id.clone(), Arc::clone(&rx_total));

            state.sessions.lock().unwrap().insert(
                session_id.clone(),
                ActiveSession {
                    cmd_tx,
                    cols,
                    rows,
                    ssh_handle: None,
                    remote_forwards: None,
                    platform_detector: Some(detector),
                    tcp_mss: core_transport::TcpMss::default(),
                    sftp: None,
                    rx_total,
                    stats_task: Some(stats_task),
                    mss_task: None,
                },
            );

            state.db.touch_session(saved_session_id).ok();
            tracing::debug!(
                session_id,
                saved_session_id,
                host,
                port,
                "telnet session opened"
            );
            Ok(session_id)
        }
        "serial" => {
            let opts: serde_json::Value =
                serde_json::from_str(&session.options_json).unwrap_or(serde_json::json!({}));
            let device = opts["device"]
                .as_str()
                .ok_or_else(|| AppError::Internal("missing serial device".into()))?
                .to_string();
            let baud_rate = opts["baud_rate"].as_u64().unwrap_or(9600) as u32;

            let session_id = Uuid::new_v4().to_string();
            let transport = tokio::task::spawn_blocking({
                let device = device.clone();
                move || SerialTransport::open(&device, baud_rate)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(|e| AppError::Internal(e.to_string()))?;

            let sid = session_id.clone();
            let app_handle = app.clone();
            let hl = Arc::clone(&state.highlighter);
            let lg = Arc::clone(&state.loggers);
            let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let login_slot: LoginRunnerSlot = empty_login_slot();
            let login_slot_for_closure = Arc::clone(&login_slot);
            // Before the I/O loop starts, so it sees the first bytes (see the fn).
            let login_link = maybe_install_login_runner(
                &login_slot,
                login_script_json.as_deref(),
                &session_id,
                &app,
            );
            let trigger_slot: TriggerRunnerSlot = empty_trigger_slot();
            let trigger_slot_for_closure = Arc::clone(&trigger_slot);
            let detector = new_detector();
            // Saved telnet/serial — persist the detected platform unless the row
            // already has a user-chosen one.
            let on_detected =
                make_detection_callback(app.clone(), session_id.clone(), Some(saved_session_id));
            let cmd_tx = transport.start_io_loop(
                make_on_data(
                    session_id.clone(),
                    lg,
                    hl,
                    login_slot_for_closure,
                    trigger_slot_for_closure,
                    Arc::clone(&rx_total),
                    Some(Arc::clone(&detector)),
                    Some(on_detected),
                    move |data| {
                        let _ = app_handle.emit(
                            "terminal-data",
                            TerminalDataPayload {
                                session_id: sid.clone(),
                                data,
                            },
                        );
                    },
                ),
                make_on_close(app.clone(), session_id.clone()),
            );

            link_login_runner(login_link, &cmd_tx);
            maybe_install_trigger_runner(
                &trigger_slot,
                triggers_json.as_deref(),
                &session_id,
                cmd_tx.clone(),
                &app,
            );
            let stats_task =
                spawn_stats_emitter(app.clone(), session_id.clone(), Arc::clone(&rx_total));

            state.sessions.lock().unwrap().insert(
                session_id.clone(),
                ActiveSession {
                    cmd_tx,
                    cols,
                    rows,
                    ssh_handle: None,
                    remote_forwards: None,
                    platform_detector: Some(detector),
                    tcp_mss: core_transport::TcpMss::default(),
                    sftp: None,
                    rx_total,
                    stats_task: Some(stats_task),
                    mss_task: None,
                },
            );

            state.db.touch_session(saved_session_id).ok();
            tracing::debug!(
                session_id,
                saved_session_id,
                device,
                baud_rate,
                "serial session opened"
            );
            Ok(session_id)
        }
        "local" => {
            // A saved local session: a WSL distribution (`wsl_distro` in the
            // options) or, without one, the configured local shell.
            let opts: serde_json::Value =
                serde_json::from_str(&session.options_json).unwrap_or(serde_json::json!({}));
            let pty = match opts["wsl_distro"].as_str().filter(|d| !d.trim().is_empty()) {
                Some(distro) => core_transport::LocalPty::spawn_program(
                    cols,
                    rows,
                    "wsl.exe",
                    &wsl_args(distro, opts["wsl_user"].as_str()),
                ),
                None => {
                    let preferred = state.db.get_setting(LOCAL_SHELL_SETTING).ok().flatten();
                    core_transport::LocalPty::spawn_with(cols, rows, preferred.as_deref())
                }
            }
            .map_err(|e| AppError::Internal(e.to_string()))?;
            let session_id = Uuid::new_v4().to_string();
            start_local_pty(app, &state, session_id.clone(), pty)?;
            Ok(session_id)
        }
        p => Err(AppError::Internal(format!(
            "protocol '{p}' not yet supported"
        ))),
    }
}

/// `wsl.exe` arguments to open one distribution, optionally as one user
/// (`wsl --distribution <name> --user <user>`, from "Basic commands for WSL").
fn wsl_args(distro: &str, user: Option<&str>) -> Vec<String> {
    let mut args = vec!["--distribution".to_string(), distro.trim().to_string()];
    if let Some(user) = user.map(str::trim).filter(|u| !u.is_empty()) {
        args.push("--user".into());
        args.push(user.to_string());
    }
    args
}

/// Save a WSL session: a local session that opens one distribution. Its
/// platform is Linux, so it gets the Linux command lists and hints.
#[tauri::command]
pub async fn create_wsl_session(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<i64>,
    distro: String,
    user: Option<String>,
) -> Result<i64, AppError> {
    if distro.trim().is_empty() {
        return Err(AppError::Internal("Pick a WSL distribution".into()));
    }
    let mut opts = serde_json::json!({ "wsl_distro": distro.trim() });
    if let Some(user) = user.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        opts["wsl_user"] = serde_json::json!(user);
    }
    let id = state
        .db
        .create_session(
            folder_id,
            &name,
            "local",
            None,
            None,
            None,
            None,
            &opts.to_string(),
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
    state
        .db
        .set_session_platform(id, "linux")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(id)
}

/// Names of the installed WSL distributions (`wsl --list --quiet`). Empty
/// when WSL isn't there (and on macOS/Linux).
#[tauri::command]
pub async fn list_wsl_distros() -> Result<Vec<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = tokio::task::spawn_blocking(|| {
            std::process::Command::new("wsl.exe")
                .args(["--list", "--quiet"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(match out {
            Ok(o) if o.status.success() => parse_wsl_list(&o.stdout),
            _ => Vec::new(),
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

/// Parse `wsl --list --quiet` output: one name per line, written as UTF-16LE
/// by wsl.exe (plain UTF-8 accepted too).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    // A UTF-16LE byte-order mark, if any, would break the zero-byte check.
    let bytes = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let utf16 = bytes.len() >= 2
        && bytes.len().is_multiple_of(2)
        && bytes.iter().skip(1).step_by(2).all(|b| *b == 0);
    let text = if utf16 {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.lines()
        .map(|l| {
            l.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod wsl_tests {
    use super::{parse_wsl_list, wsl_args};

    #[test]
    fn wsl_list_utf16_and_utf8() {
        let utf16: Vec<u8> = "Ubuntu\r\nDebian\r\n"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert_eq!(parse_wsl_list(&utf16), vec!["Ubuntu", "Debian"]);
        assert_eq!(parse_wsl_list(b"Ubuntu-24.04\n\n"), vec!["Ubuntu-24.04"]);
        let mut with_bom = vec![0xFF, 0xFE];
        with_bom.extend_from_slice(&utf16);
        assert_eq!(parse_wsl_list(&with_bom), vec!["Ubuntu", "Debian"]);
        assert!(parse_wsl_list(b"").is_empty());
    }

    #[test]
    fn wsl_args_with_and_without_user() {
        assert_eq!(wsl_args(" Ubuntu ", None), vec!["--distribution", "Ubuntu"]);
        assert_eq!(
            wsl_args("Debian", Some("root")),
            vec!["--distribution", "Debian", "--user", "root"]
        );
        assert_eq!(
            wsl_args("Debian", Some("  ")),
            vec!["--distribution", "Debian"]
        );
    }
}

/// Open a Telnet session directly (without saving).
#[tauri::command]
pub async fn open_telnet_session(
    app: AppHandle,
    state: State<'_, AppState>,
    host: String,
    port: u16,
    cols: u16,
    rows: u16,
) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();
    let transport = TelnetTransport::connect(host.clone(), port)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let sid = session_id.clone();
    let app_handle = app.clone();
    let hl = Arc::clone(&state.highlighter);
    let lg = Arc::clone(&state.loggers);
    let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let detector = new_detector();
    // Ephemeral session (no saved id) — emit only, don't persist.
    let on_detected = make_detection_callback(app.clone(), session_id.clone(), None);
    let cmd_tx = transport.start_io_loop(
        cols,
        rows,
        make_on_data(
            session_id.clone(),
            lg,
            hl,
            empty_login_slot(),
            empty_trigger_slot(),
            Arc::clone(&rx_total),
            Some(Arc::clone(&detector)),
            Some(on_detected),
            move |data| {
                let _ = app_handle.emit(
                    "terminal-data",
                    TerminalDataPayload {
                        session_id: sid.clone(),
                        data,
                    },
                );
            },
        ),
        make_on_close(app.clone(), session_id.clone()),
    );
    let stats_task = Some(spawn_stats_emitter(
        app.clone(),
        session_id.clone(),
        Arc::clone(&rx_total),
    ));

    state.sessions.lock().unwrap().insert(
        session_id.clone(),
        ActiveSession {
            cmd_tx,
            cols,
            rows,
            ssh_handle: None,
            remote_forwards: None,
            platform_detector: Some(detector),
            tcp_mss: core_transport::TcpMss::default(),
            sftp: None,
            rx_total,
            stats_task,
            mss_task: None,
        },
    );

    tracing::debug!(session_id, host, port, "telnet session opened");
    Ok(session_id)
}

/// Open a Serial session directly (without saving).
#[tauri::command]
pub async fn open_serial_session(
    app: AppHandle,
    state: State<'_, AppState>,
    device: String,
    baud_rate: u32,
) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();
    let transport = tokio::task::spawn_blocking({
        let device = device.clone();
        move || SerialTransport::open(&device, baud_rate)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let sid = session_id.clone();
    let app_handle = app.clone();
    let hl = Arc::clone(&state.highlighter);
    let lg = Arc::clone(&state.loggers);
    let rx_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let detector = new_detector();
    // Ephemeral session (no saved id) — emit only, don't persist.
    let on_detected = make_detection_callback(app.clone(), session_id.clone(), None);
    let cmd_tx = transport.start_io_loop(
        make_on_data(
            session_id.clone(),
            lg,
            hl,
            empty_login_slot(),
            empty_trigger_slot(),
            Arc::clone(&rx_total),
            Some(Arc::clone(&detector)),
            Some(on_detected),
            move |data| {
                let _ = app_handle.emit(
                    "terminal-data",
                    TerminalDataPayload {
                        session_id: sid.clone(),
                        data,
                    },
                );
            },
        ),
        make_on_close(app.clone(), session_id.clone()),
    );
    let stats_task = Some(spawn_stats_emitter(
        app.clone(),
        session_id.clone(),
        Arc::clone(&rx_total),
    ));

    // Serial sessions get a default size; resize is ignored by the transport.
    state.sessions.lock().unwrap().insert(
        session_id.clone(),
        ActiveSession {
            cmd_tx,
            cols: 80,
            rows: 24,
            ssh_handle: None,
            remote_forwards: None,
            platform_detector: Some(detector),
            tcp_mss: core_transport::TcpMss::default(),
            sftp: None,
            rx_total,
            stats_task,
            mss_task: None,
        },
    );

    tracing::debug!(session_id, device, baud_rate, "serial session opened");
    Ok(session_id)
}

/// List available serial ports on this machine.
#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<String>, AppError> {
    Ok(core_transport::list_serial_ports())
}

/// Save a new Telnet session.
#[tauri::command]
pub async fn create_telnet_session(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<i64>,
    host: String,
    port: u16,
) -> Result<i64, AppError> {
    state
        .db
        .create_session(
            folder_id,
            &name,
            "telnet",
            Some(&host),
            Some(port),
            None,
            None,
            "{}",
        )
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Save a new Serial session (device and baud stored in options_json).
#[tauri::command]
pub async fn create_serial_session(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<i64>,
    device: String,
    baud_rate: u32,
) -> Result<i64, AppError> {
    let opts = serde_json::json!({ "device": device, "baud_rate": baud_rate }).to_string();
    state
        .db
        .create_session(folder_id, &name, "serial", None, None, None, None, &opts)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[derive(serde::Serialize, Clone)]
struct TerminalDataPayload {
    session_id: String,
    data: Vec<u8>,
}

#[derive(serde::Serialize, Clone)]
struct DisconnectPayload {
    session_id: String,
    /// The remote end finished the session cleanly (`exit`, logout, the
    /// device closed it): show it as ended, don't auto-reconnect. False for a
    /// lost link, which is worth reconnecting.
    ended: bool,
}

/// Build the `on_close` callback for a session: when it ends by itself (the
/// remote side closed it, or the link failed), emit `session-disconnected` so
/// the frontend can show it and, for a lost link, auto-reconnect. Never fired
/// when the user closes the tab.
fn make_on_close(
    app: AppHandle,
    session_id: String,
) -> impl FnOnce(core_transport::CloseReason) + Send + 'static {
    move |reason| emit_disconnected(&app, session_id, reason)
}

fn emit_disconnected(app: &AppHandle, session_id: String, reason: core_transport::CloseReason) {
    let ended = reason == core_transport::CloseReason::Ended;
    let _ = app.emit(
        "session-disconnected",
        DisconnectPayload { session_id, ended },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;

    #[test]
    fn typed_password_reads_the_encrypted_copy_and_warms_the_cache() {
        let dir = std::env::temp_dir().join(format!("zapterm-typed-pw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = core_persistence::Database::open(&dir.join("zapterm.db")).unwrap();
        let id = db
            .create_session_full(
                None,
                "fw",
                "ssh",
                Some("fw.example"),
                Some(22),
                Some("admin"),
                None,
                "{}",
                None,
                None,
            )
            .unwrap();
        let seed = "test-seed";
        db.set_session_secret(id, &core_vault::encrypt_with_seed(seed, "s3cret").unwrap())
            .unwrap();
        let cache: crate::state::PasswordCache = Arc::new(Mutex::new(HashMap::new()));

        // Run on a thread with a deadline: a lock held across the fallback
        // used to hang here forever.
        let (tx, rx) = mpsc::channel();
        let (db2, cache2) = (db, Arc::clone(&cache));
        std::thread::spawn(move || {
            let _ = tx.send(typed_password(&db2, &cache2, seed, id));
        });
        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("typed_password deadlocked");
        assert_eq!(got.as_deref(), Some("s3cret"));
        assert_eq!(
            cache.lock().unwrap().get(&id).map(String::as_str),
            Some("s3cret")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
