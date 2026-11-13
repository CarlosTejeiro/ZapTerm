// `deny` (not `forbid`) so the `tcp_mss` module can locally `allow(unsafe_code)`
// for its libc FFI. Every other module in this crate still rejects unsafe.
#![deny(unsafe_code)]

//! `core-transport` — the per-protocol session transports (SSH, Telnet,
//! serial, local PTY), port forwarding and SFTP.
//!
//! This crate owns the boundary between session management and the underlying
//! network/serial protocols. Each transport exposes the same shape: open it,
//! then `start_io_loop` with an output callback and a close callback, and
//! drive it with [`SessionCmd`]s.

pub mod error;
pub mod forwards;
pub mod local_pty;
pub mod serial;
pub mod sftp;
pub mod ssh;
pub mod tcp_mss;
pub mod telnet;

pub use tcp_mss::{MssWatcher, TcpMss};

pub use forwards::{
    open_dynamic_forward, open_local_forward, open_remote_forward, ForwardController, ForwardInfo,
};
pub use local_pty::LocalPty;
pub use serial::{list_ports as list_serial_ports, SerialTransport};
pub use sftp::{
    empty_slot as empty_sftp_slot, new_cancel_token as new_sftp_cancel_token, CancelToken,
    ProgressFn, SftpClient, SftpEntry, SftpSlot,
};
pub use ssh::{
    connect_authenticated, connect_authenticated_via, known_hosts_path, merge_known_hosts_text,
    overwrite_host_key, preflight_host_key, set_keepalive_secs, trust_host_key, AgentPriority,
    HostKeyStatus, KiPrompt, KiRequest, KiResponder, RemoteForwardRegistry, RemoteTarget, SshAuth,
    SshTransport,
};

/// Type alias for the authenticated SSH session handle that the app layer
/// passes around as a bastion link in ProxyJump chains.
pub type SshHandle = russh::client::Handle<ssh::SshClientHandler>;

/// Shared version of [`SshHandle`] used for the *primary* SSH session of a
/// tab (shell, SFTP, port forwards). Every russh `Handle` method used here
/// takes `&self`, so callers take a *read* lock and run concurrently — a slow
/// `direct-tcpip` open (the server connecting to a dead target) no longer
/// holds up the other tunnels or SFTP on the same session.
pub type SharedSshHandle = std::sync::Arc<tokio::sync::RwLock<SshHandle>>;
pub use telnet::TelnetTransport;

/// Why a session's I/O loop ended by itself (it never fires when the app
/// closed the session).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// The remote end finished the session cleanly: the shell exited, the
    /// device logged the user out, the server closed the connection.
    Ended,
    /// The link failed: a read/write error, unanswered keepalives, an
    /// unplugged serial adapter. Worth reconnecting.
    Lost,
}

/// Commands sent to a live session's I/O task.
#[derive(Debug)]
pub enum SessionCmd {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}
