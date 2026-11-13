use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

/// Map of in-flight keyboard-interactive interactions, keyed by UUID. The
/// transport-layer responder inserts a sender here and awaits the matching
/// receiver; the `respond_keyboard_interactive` Tauri command fires it.
pub type KiPending = Arc<Mutex<HashMap<String, oneshot::Sender<Vec<String>>>>>;

use std::sync::atomic::AtomicU64;

use core_hints::detector::PlatformDetector;
use core_transport::{CancelToken, ForwardController, RemoteForwardRegistry, SessionCmd, TcpMss};

/// Shared per-session platform detector. Lives behind a `Mutex` so the
/// terminal-data on_data closure can feed bytes synchronously while Tauri
/// commands can also read the latched verdict.
pub type SharedPlatformDetector = Arc<Mutex<PlatformDetector>>;

/// Process-lifetime password cache keyed by saved-session id.
///
/// We need this because on macOS *unsigned* dev builds the OS Keychain often
/// denies the binary access to its own entry — so even though
/// `Vault::store` succeeded at create time, `Vault::retrieve` later returns
/// "no matching entry". The user-typed password is parked here on first
/// successful auth so subsequent reopens of the same session don't prompt.
/// Cleared on app exit.
pub type PasswordCache = Arc<Mutex<HashMap<i64, String>>>;

/// A live terminal session (local PTY or SSH).
pub struct ActiveSession {
    /// Send keyboard input or resize events to the session's I/O task.
    pub cmd_tx: UnboundedSender<SessionCmd>,
    pub cols: u16,
    pub rows: u16,
    /// Shared SSH session handle. `Some` for SSH sessions (used by port-forwards
    /// to open `direct-tcpip` channels); `None` for local PTY / Telnet / Serial.
    /// Mutex-wrapped so `-R` (which needs `&mut Handle`) can coexist with the
    /// `&self` calls used by `-L`, `-D` and SFTP.
    pub ssh_handle: Option<core_transport::SharedSshHandle>,
    /// Shared registry of `(bind_addr, bind_port) → target` for active `-R`
    /// remote forwards. Same `Arc` the russh client handler holds. `None`
    /// for non-SSH sessions.
    pub remote_forwards: Option<RemoteForwardRegistry>,
    /// Per-session platform auto-detector — fed bytes from `make_on_data`
    /// and latched the first time a recognised vendor prompt + signature
    /// appear together. `None` for non-SSH sessions (local PTY / serial /
    /// telnet) where auto-detection doesn't apply.
    pub platform_detector: Option<SharedPlatformDetector>,
    /// TCP MSS snapshot taken right after the SSH socket connected.
    /// `default()` for non-SSH sessions and for SSH-via-bastion (which has
    /// no direct TCP socket of its own).
    pub tcp_mss: TcpMss,
    /// Lazy-initialised SFTP subsystem, opened on first use by an `sftp_*`
    /// Tauri command. `None` outer = no SFTP yet attempted; `Some(None)` = slot
    /// exists for an SSH session; `Some(Some(_))` = SFTP active.
    pub sftp: Option<core_transport::SftpSlot>,
    /// Cumulative bytes received from the remote since session start. The
    /// stats emitter task reads this once per second to compute the rolling
    /// throughput rate without holding the sessions lock.
    pub rx_total: Arc<AtomicU64>,
    /// Handle for the periodic stats task — aborted on session close.
    pub stats_task: Option<tokio::task::JoinHandle<()>>,
    /// Handle for the live MSS polling task (direct SSH sessions) — aborted
    /// on session close. It holds a duplicate of the socket, so leaving it
    /// running would keep the closed connection's socket open.
    pub mss_task: Option<tokio::task::JoinHandle<()>>,
}

/// A running session log: the logger plus the DB row id for later finalisation.
pub struct ActiveLog {
    pub logger: core_logging::SessionLogger,
    pub log_db_id: i64,
}

/// Shared application state injected into Tauri commands via [`tauri::State`].
pub struct AppState {
    /// Resolved data directory + how it was chosen (flag/env/portable/
    /// pointer/default). Everything under it: `zapterm.db`, `session_logs/`,
    /// `catalogs/`.
    pub data_dir: crate::data_dir::DataDir,
    /// Live terminal sessions keyed by UUID.
    pub sessions: Mutex<HashMap<String, ActiveSession>>,
    /// SQLite database (folders, saved sessions, credential references).
    pub db: core_persistence::Database,
    /// Keyword highlighter — rebuilt whenever rules change.
    pub highlighter: Arc<RwLock<core_highlight::Highlighter>>,
    /// Active session loggers keyed by session UUID.
    /// Wrapped in Arc so callbacks can hold a reference without borrowing AppState.
    pub loggers: Arc<Mutex<HashMap<String, ActiveLog>>>,
    /// Directory where session log files are written.
    pub log_dir: PathBuf,
    /// In-flight keyboard-interactive prompts awaiting frontend responses,
    /// keyed by interaction id (UUID). The Tauri command `respond_keyboard_interactive`
    /// removes the sender for a given id and fires it with the user's answers.
    pub ki_pending: KiPending,
    /// Active port-forwards keyed by SSH session UUID. Dropping a
    /// [`ForwardController`] stops its listener task.
    pub forwards: Mutex<HashMap<String, Vec<ForwardController>>>,
    /// In-memory password cache (see [`PasswordCache`]). Survives Cmd+R
    /// because it lives in the Rust process, not the webview.
    pub password_cache: PasswordCache,
    /// Stable, per-installation seed used to derive the AES key for the
    /// SQLite `session_secrets` fallback. Currently the app data
    /// directory's path — same for every launch by the same OS user.
    pub vault_seed: String,
    /// Active SFTP transfers keyed by their frontend-issued `transfer_id`.
    /// The frontend calls `sftp_cancel_transfer` with the id; we flip the
    /// flag and the streaming loop returns `Error::Cancelled` on its next
    /// chunk boundary.
    pub sftp_cancellations: Arc<Mutex<HashMap<String, CancelToken>>>,
    /// Filesystem watcher for `<app_data>/catalogs/` — its Drop is what
    /// stops the worker thread, so we just need to keep it alive for the
    /// duration of the process. `None` if the watcher could not start.
    pub hint_watcher: Option<crate::hint_watcher::WatcherGuard>,
    /// Running Python scripts by run id (see `crate::scripting`).
    pub scripts: crate::scripting::Runs,
}
