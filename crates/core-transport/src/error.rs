/// Errors produced by [`core_transport`](crate).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to spawn shell process: {0}")]
    SpawnFailed(String),

    #[error("failed to resize terminal: {0}")]
    ResizeFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SSH error: {0}")]
    Ssh(#[from] russh::Error),

    #[error("authentication failed")]
    AuthFailed,

    #[error("failed to load private key: {0}")]
    KeyLoad(String),

    #[error("wrong or missing passphrase for private key")]
    KeyPassphrase,

    #[error("HOST KEY CHANGED — possible MITM (fingerprint {fingerprint})")]
    HostKeyChanged { fingerprint: String },

    #[error("known_hosts: {0}")]
    KnownHosts(String),

    #[error("invalid connection setup: {0}")]
    InvalidSetup(String),

    #[error("ssh agent error: {0}")]
    Agent(String),

    #[error("SFTP error: {0}")]
    Sftp(String),

    #[error("serial port error: {0}")]
    Serial(#[from] tokio_serial::Error),

    #[error("connection refused or timed out")]
    ConnectionFailed,

    #[error("transfer cancelled by user")]
    Cancelled,
}
