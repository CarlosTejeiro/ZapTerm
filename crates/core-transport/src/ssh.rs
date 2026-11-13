use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use russh::client::{self, AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::ChannelMsg;

use crate::error::Error;
use crate::SessionCmd;

/// Global SSH keepalive interval in seconds (0 = disabled). It's a single
/// user-wide preference, so a global avoids threading the value through every
/// connect signature (including the whole bastion chain). Read when each russh
/// client config is built, so it takes effect on connections opened afterwards.
static SSH_KEEPALIVE_SECS: AtomicU64 = AtomicU64::new(0);

/// Set the SSH server-alive interval (seconds; 0 disables). Affects sessions
/// opened after this call.
pub fn set_keepalive_secs(secs: u64) {
    SSH_KEEPALIVE_SECS.store(secs, Ordering::Relaxed);
}

/// russh client config carrying the configured keepalive. `keepalive_max` is
/// how many unanswered keepalives russh tolerates before dropping the link —
/// which then surfaces as a normal disconnect (and drives auto-reconnect).
fn build_client_config() -> Arc<client::Config> {
    let mut config = client::Config::default();
    let secs = SSH_KEEPALIVE_SECS.load(Ordering::Relaxed);
    if secs > 0 {
        config.keepalive_interval = Some(std::time::Duration::from_secs(secs));
        config.keepalive_max = 3;
    }
    Arc::new(config)
}

// ---------------------------------------------------------------------------
// Remote forward (-R) registry
// ---------------------------------------------------------------------------

/// Target of an active remote forward (where to bridge incoming
/// `forwarded-tcpip` channels). Owned by the registry described below.
#[derive(Debug, Clone)]
pub struct RemoteTarget {
    pub host: String,
    pub port: u16,
}

/// Shared map of `(bind_address, bind_port)` on the SSH server → local
/// target to bridge to. Both the russh client handler (which receives
/// `forwarded-tcpip` channels) and the [`crate::forwards::open_remote_forward`]
/// code (which inserts new entries) hold an `Arc` to the same map.
///
/// We don't use russh's per-message routing because it can't bridge to an
/// arbitrary local address — the bridge logic and the target lookup must
/// live in our handler.
pub type RemoteForwardRegistry = Arc<Mutex<HashMap<(String, u16), RemoteTarget>>>;

pub fn new_remote_forward_registry() -> RemoteForwardRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Authentication strategy
// ---------------------------------------------------------------------------

/// One server-driven keyboard-interactive prompt.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KiPrompt {
    pub prompt: String,
    pub echo: bool,
}

/// A keyboard-interactive `InfoRequest` from the server. The caller must
/// provide a `Vec<String>` of responses with one entry per `prompts` element.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KiRequest {
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<KiPrompt>,
}

/// Async callback that receives a [`KiRequest`] and returns the user's answers.
///
/// Kept as a trait object so `core-transport` stays free of Tauri/UI concerns:
/// the app layer supplies a responder that bridges to the frontend via a
/// Tauri event + oneshot channel.
pub type KiResponder =
    Arc<dyn Fn(KiRequest) -> Pin<Box<dyn Future<Output = Vec<String>> + Send>> + Send + Sync>;

/// On Windows, which agent backend(s) `SshAuth::Agent` tries — and in which
/// order. Unix builds ignore this (only one agent, via `SSH_AUTH_SOCK`).
///
/// `Auto` is the default and matches the historical behaviour: OpenSSH-for-
/// Windows first, fall back to Pageant. `PageantFirst` is for PuTTY-centric
/// users whose keys live exclusively in Pageant. The `*Only` variants skip
/// the fallback so a misconfiguration produces a clear error instead of a
/// silent attempt against the wrong agent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AgentPriority {
    #[default]
    Auto,
    PageantFirst,
    OpenSshOnly,
    PageantOnly,
}

impl AgentPriority {
    /// Parse the user-facing string form persisted in `settings`. Unknown
    /// strings fall back to `Auto` so a stale or hand-edited DB never blocks
    /// the user from connecting.
    pub fn from_setting(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Self::Auto,
            "pageant-first" | "pageant_first" => Self::PageantFirst,
            "openssh-only" | "openssh_only" => Self::OpenSshOnly,
            "pageant-only" | "pageant_only" => Self::PageantOnly,
            _ => Self::Auto,
        }
    }

    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::PageantFirst => "pageant-first",
            Self::OpenSshOnly => "openssh-only",
            Self::PageantOnly => "pageant-only",
        }
    }
}

/// How to authenticate an SSH session.
///
/// The key *path* (not the key bytes) is carried here: the file is not a secret
/// to copy around, and `russh::keys::load_secret_key` reads from a path. Only the
/// passphrase is a secret and lives in the OS vault.
#[derive(Clone)]
pub enum SshAuth {
    Password(String),
    PublicKey {
        key_path: String,
        passphrase: Option<String>,
    },
    /// 2FA / OTP. The `responder` is invoked once per server `InfoRequest`
    /// (there can be several) and must return one answer per prompt.
    KeyboardInteractive {
        responder: KiResponder,
    },
    /// SSH agent auth. `priority` is only consulted on Windows where two
    /// agent implementations (OpenSSH named pipe, Pageant) might both be
    /// reachable; on Unix it's ignored.
    Agent {
        priority: AgentPriority,
    },
}

// ---------------------------------------------------------------------------
// Host-key verification (known_hosts)
// ---------------------------------------------------------------------------

/// Result of checking a server's host key against `~/.ssh/known_hosts`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum HostKeyStatus {
    /// Recorded and matches — safe to connect silently.
    Known,
    /// Not in known_hosts — first connection (TOFU decision needed).
    Unknown { fingerprint: String },
    /// Recorded but DIFFERENT — possible MITM; must warn the user.
    Changed { fingerprint: String },
}

/// `SHA256:<base64, unpadded>` — the OpenSSH display form. ssh-key's
/// `Fingerprint` renders exactly that, so the strings the UI shows and compares
/// are byte-identical to what earlier builds produced.
fn fingerprint(pubkey: &PublicKey) -> String {
    pubkey.fingerprint(HashAlg::Sha256).to_string()
}

/// The key to check against `known_hosts`. For a host *certificate* that is the
/// key embedded in it — what OpenSSH falls back to when no `@cert-authority`
/// line matches (russh's known_hosts support has no notion of a CA).
fn host_key_of(presented: &PublicKeyOrCertificate) -> PublicKey {
    match presented {
        PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
        PublicKeyOrCertificate::Certificate(cert) => PublicKey::from(cert.public_key().clone()),
    }
}

/// Expand a leading `~`, `~/` or `~\` to the user's home directory
/// (`load_secret_key` opens the path verbatim and does not expand `~`). Uses
/// the same home lookup as `known_hosts`.
fn expand_tilde(path: &str) -> std::path::PathBuf {
    let rest = if path == "~" {
        Some("")
    } else {
        path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"))
    };
    if let (Some(rest), Some(home)) = (rest, home::home_dir()) {
        return home.join(rest);
    }
    std::path::PathBuf::from(path)
}

/// Host-key handler for real connections.
///
/// * **Changed** keys are always rejected (possible MITM — fail closed).
/// * **Unknown** keys depend on [`Self::allow_tofu`]:
///   - `false` (direct connections): rejected. Trust must be established
///     out-of-band first — the UI runs [`preflight_host_key`], shows the
///     fingerprint, and on approval calls [`trust_host_key`] to record it,
///     so by connect time the key is already `Known`. An unknown key here
///     means preflight was bypassed or the key changed in the gap, so we
///     fail closed rather than silently trusting.
///   - `true` (bastion hops and jump-host targets): Trust On First Use —
///     record and accept, logging the learned fingerprint for an audit
///     trail. This is unavoidable for hosts reachable *only* through a jump
///     host, since the UI preflight can't open a direct socket to prompt.
pub struct SshClientHandler {
    host: String,
    port: u16,
    /// Whether to Trust On First Use an unknown key (see the type docs).
    /// Only `true` for sessions that ride inside an SSH tunnel (bastion hops
    /// and jump-host targets), where a direct preflight is impossible.
    allow_tofu: bool,
    /// Shared with [`crate::forwards::open_remote_forward`]; the
    /// `server_channel_open_forwarded_tcpip` callback below uses it to find
    /// where to bridge each incoming forwarded channel.
    forwards: RemoteForwardRegistry,
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        presented: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = host_key_of(presented);
        let server_public_key = &key;
        // known_hosts is read (and possibly written) with blocking file I/O:
        // keep it off the async worker, which also drives other sessions.
        let (host, port, k) = (self.host.clone(), self.port, key.clone());
        let checked = tokio::task::spawn_blocking(move || keys::check_known_hosts(&host, port, &k))
            .await
            .unwrap_or_else(|e| Err(keys::Error::IO(std::io::Error::other(e.to_string()))));
        match checked {
            // Recorded and matches → accept.
            Ok(true) => Ok(true),
            // Recorded but DIFFERENT → possible MITM; reject (fail closed).
            Err(keys::Error::KeyChanged { .. }) => Ok(false),
            // Unknown (not recorded) or unreadable known_hosts.
            _ => {
                if !self.allow_tofu {
                    // Direct connection: the UI preflight should already have
                    // recorded this key. Reaching here means it didn't —
                    // fail closed instead of trusting blindly.
                    tracing::warn!(
                        host = %self.host,
                        port = self.port,
                        fingerprint = %fingerprint(server_public_key),
                        "rejecting unknown host key on a direct connection (preflight bypassed?)"
                    );
                    return Ok(false);
                }
                // Tunneled host (bastion / jump target): TOFU is the only
                // option, but leave an audit trail of what we trusted.
                tracing::warn!(
                    host = %self.host,
                    port = self.port,
                    fingerprint = %fingerprint(server_public_key),
                    "trusting unknown host key on first use (reachable only via jump host)"
                );
                let (host, port, k) = (self.host.clone(), self.port, key.clone());
                let _ = tokio::task::spawn_blocking(move || {
                    keys::known_hosts::learn_known_hosts(&host, port, &k)
                })
                .await;
                Ok(true)
            }
        }
    }

    /// Handle an incoming `forwarded-tcpip` channel — fired when something
    /// on the remote side connects to one of our active `-R` bind ports.
    /// Look up the local target in [`Self::forwards`] and bridge bytes.
    /// Unknown binds are refused (the server shouldn't be forwarding ports we
    /// didn't register). The open must be answered through `reply` either
    /// way — russh no longer confirms the channel on our behalf.
    #[allow(clippy::too_many_arguments)]
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let key = (connected_address.to_string(), connected_port as u16);
        let target = match self.forwards.lock() {
            Ok(map) => map.get(&key).cloned(),
            Err(_) => None,
        };
        let Some(target) = target else {
            tracing::debug!(
                "forwarded-tcpip for unknown bind {}:{}; refusing",
                connected_address,
                connected_port
            );
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        // Connect to the local target first and answer the open by the
        // result, like OpenSSH: a down target is refused cleanly instead of
        // looking like a connection that opened and closed at once. Bounded,
        // since this runs on the session task.
        let connect = tokio::net::TcpStream::connect((target.host.as_str(), target.port));
        let mut local = match tokio::time::timeout(std::time::Duration::from_secs(5), connect).await
        {
            Ok(Ok(local)) => local,
            outcome => {
                let why = match outcome {
                    Ok(Err(e)) => e.to_string(),
                    _ => "timed out".into(),
                };
                tracing::warn!(
                    "remote forward → {}:{} connect failed: {why}",
                    target.host,
                    target.port
                );
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
                return Ok(());
            }
        };
        reply.accept().await;
        tokio::spawn(async move {
            let mut remote = channel.into_stream();
            if let Err(e) = tokio::io::copy_bidirectional(&mut remote, &mut local).await {
                tracing::debug!("remote forward bridge ended: {e}");
            }
        });
        Ok(())
    }
}

/// Handler used only for preflight/trust: captures the server's host key (so we
/// can inspect it) and accepts the connection so the KEX completes. It never
/// authenticates.
struct KeyCaptureHandler {
    captured: Arc<Mutex<Option<PublicKey>>>,
}

impl client::Handler for KeyCaptureHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        presented: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        *self.captured.lock().unwrap() = Some(host_key_of(presented));
        Ok(true)
    }
}

/// Longest wait for the SSH banner + key exchange (before authentication,
/// which can be interactive and is never bounded). A host that accepts TCP
/// but never speaks SSH — wrong port, tarpit, half-dead middlebox — would
/// otherwise hang the tab forever.
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

async fn within_handshake<T>(
    fut: impl Future<Output = Result<T, russh::Error>>,
) -> Result<T, Error> {
    match tokio::time::timeout(HANDSHAKE_TIMEOUT, fut).await {
        Ok(res) => res.map_err(Error::Ssh),
        Err(_) => Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!(
                "no SSH handshake after {}s (is it an SSH port?)",
                HANDSHAKE_TIMEOUT.as_secs()
            ),
        ))),
    }
}

/// Open a throwaway connection, capture the host key, and return it.
async fn capture_host_key(host: &str, port: u16) -> Result<PublicKey, Error> {
    let captured = Arc::new(Mutex::new(None));
    let handler = KeyCaptureHandler {
        captured: Arc::clone(&captured),
    };
    let config = build_client_config();
    // The session is dropped at the end of this fn, closing the connection.
    let _session =
        within_handshake(client::connect(config, (host.to_owned(), port), handler)).await?;
    let key = captured
        .lock()
        .unwrap()
        .take()
        .ok_or(Error::ConnectionFailed)?;
    Ok(key)
}

/// Fetch the server host key and classify it against `known_hosts`.
///
/// Run this *before* connecting so the UI can prompt for TOFU approval or warn
/// about a changed key.
pub async fn preflight_host_key(host: String, port: u16) -> Result<HostKeyStatus, Error> {
    let key = capture_host_key(&host, port).await?;
    let fp = fingerprint(&key);
    let status = match keys::check_known_hosts(&host, port, &key) {
        Ok(true) => HostKeyStatus::Known,
        Err(keys::Error::KeyChanged { .. }) => HostKeyStatus::Changed { fingerprint: fp },
        // Ok(false) = not recorded; any other error = unreadable known_hosts,
        // both treated as "unknown" so the user gets a TOFU prompt.
        _ => HostKeyStatus::Unknown { fingerprint: fp },
    };
    Ok(status)
}

/// Persist trust for a host key (writes to `~/.ssh/known_hosts`).
///
/// `expected_fp` is the fingerprint the user approved during
/// [`preflight_host_key`]. We re-fetch the key here (a fresh connection) and
/// refuse to learn it unless it still matches `expected_fp` — otherwise a MITM
/// could present a benign key at preflight and swap in a different one at trust
/// time, getting their key written to `known_hosts`. Comparing the full
/// SHA-256 fingerprint closes that TOCTOU window.
pub async fn trust_host_key(host: String, port: u16, expected_fp: String) -> Result<(), Error> {
    let key = capture_host_key(&host, port).await?;
    let actual_fp = fingerprint(&key);
    if actual_fp != expected_fp {
        return Err(Error::HostKeyChanged {
            fingerprint: actual_fp,
        });
    }
    keys::known_hosts::learn_known_hosts(&host, port, &key)
        .map_err(|e| Error::KnownHosts(e.to_string()))?;
    Ok(())
}

/// The `known_hosts` file russh reads/writes: the OpenSSH-standard
/// `~/.ssh/known_hosts` on every platform (mirrors russh's private
/// `known_hosts_path`). Older russh used `%USERPROFILE%\ssh\known_hosts` (no
/// dot) on Windows — see [`migrate_legacy_known_hosts`] for the one-time move.
fn known_hosts_file() -> Result<std::path::PathBuf, Error> {
    let home = home::home_dir().ok_or_else(|| Error::KnownHosts("no home directory".into()))?;
    Ok(home.join(".ssh").join("known_hosts"))
}

/// Windows users of earlier ZapTerm builds have their trusted hosts in
/// `%USERPROFILE%\ssh\known_hosts` (the path the old SSH library used); the
/// library now reads the standard `.ssh\known_hosts`, so without this every
/// host would come back as "unknown" and need re-approval. Fold the legacy
/// entries into the standard file once, then rename the legacy file so the
/// merge never repeats. Never fatal: on any error the user just re-approves a
/// key. No-op on other platforms and when there is nothing to migrate.
pub fn migrate_legacy_known_hosts() {
    #[cfg(windows)]
    {
        let Some(home) = home::home_dir() else { return };
        let legacy = home.join("ssh").join("known_hosts");
        if !legacy.is_file() {
            return;
        }
        let target = home.join(".ssh").join("known_hosts");
        match merge_known_hosts(&legacy, &target) {
            Ok(added) => {
                tracing::info!(added, "migrated legacy Windows known_hosts entries");
                let parked = home.join("ssh").join("known_hosts.migrated-to-.ssh");
                if let Err(e) = std::fs::rename(&legacy, &parked) {
                    tracing::warn!("could not park legacy known_hosts: {e}");
                }
            }
            Err(e) => tracing::warn!("legacy known_hosts migration failed: {e}"),
        }
    }
}

/// Where the user's `known_hosts` lives (`~/.ssh/known_hosts`), or `None`
/// when the home directory can't be resolved. Exposed so the encrypted
/// backup can carry the trusted host keys along with the sessions.
pub fn known_hosts_path() -> Option<std::path::PathBuf> {
    known_hosts_file().ok()
}

/// Append every non-blank, non-comment line of `legacy` that `target` does not
/// already contain verbatim. Creates `target` (and its directory) if missing.
/// Returns how many lines were added. Platform-agnostic so it can be unit
/// tested anywhere; the Windows-only caller decides the paths.
#[cfg_attr(not(windows), allow(dead_code))]
fn merge_known_hosts(legacy: &std::path::Path, target: &std::path::Path) -> std::io::Result<usize> {
    let old = std::fs::read_to_string(legacy)?;
    merge_known_hosts_text(&old, target)
}

/// Line-level merge behind [`merge_known_hosts`]: fold `known_hosts`-formatted
/// `old` text into `target`, adding only the lines it lacks. Creates `target`
/// (and its directory) if missing. Returns how many lines were added.
pub fn merge_known_hosts_text(old: &str, target: &std::path::Path) -> std::io::Result<usize> {
    use std::io::Write;
    // Read the current file as bytes: a missing file is empty, any other read
    // error aborts (never rewrite a file we couldn't read), and the existing
    // content is never rewritten — new lines are only appended — so a byte
    // that isn't valid UTF-8 (a Latin-1 comment) survives untouched.
    let existing = match std::fs::read(target) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let existing_text = String::from_utf8_lossy(&existing);
    let have: std::collections::HashSet<&str> = existing_text.lines().map(str::trim).collect();
    // Host + key type already recorded: an incoming line with another key for
    // the same pair is an older key (a backup made before the server was
    // reinstalled) and would make every connect report "HOST KEY CHANGED".
    let have_hosts: std::collections::HashSet<(&str, &str)> = existing_text
        .lines()
        .filter_map(host_and_key_type)
        .collect();
    let mut add = String::new();
    let mut added = 0usize;
    for line in old.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || have.contains(t) {
            continue;
        }
        if host_and_key_type(t).is_some_and(|k| have_hosts.contains(&k)) {
            continue;
        }
        add.push_str(t);
        add.push('\n');
        added += 1;
    }
    if added > 0 {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)?;
        if !existing.is_empty() && !existing.ends_with(b"\n") {
            f.write_all(b"\n")?;
        }
        f.write_all(add.as_bytes())?;
    }
    Ok(added)
}

/// `(hosts, key type)` of a `known_hosts` line, skipping a leading
/// `@cert-authority` / `@revoked` marker; None for blanks and comments.
fn host_and_key_type(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split_whitespace();
    let mut first = fields.next()?;
    if first.starts_with('#') {
        return None;
    }
    if first.starts_with('@') {
        first = fields.next()?;
    }
    Some((first, fields.next()?))
}

/// Drop every plaintext `known_hosts` entry for `host` (keyed `[host]:port` when
/// the port isn't 22) from `path`, mirroring `ssh-keygen -R`. Returns how many
/// lines were removed. A missing file is not an error (0 removed).
///
/// Hashed (`|1|…`) entries are left untouched — matching them needs the per-line
/// HMAC salt, which we deliberately don't pull in here; [`overwrite_host_key`]
/// re-verifies afterward and surfaces a clear error if a stale hashed entry
/// remains, rather than silently "succeeding" into another mismatch.
fn remove_known_hosts_lines(
    path: &std::path::Path,
    host: &str,
    port: u16,
) -> std::io::Result<usize> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let needle = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };
    let mut removed = 0usize;
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        // The host field is the first whitespace-delimited token; a single entry
        // can list several comma-separated hosts (e.g. `host,1.2.3.4`).
        let is_match = {
            let trimmed = line.trim_start();
            !trimmed.starts_with('#')
                && trimmed
                    .split_whitespace()
                    .next()
                    .is_some_and(|hosts| hosts.split(',').any(|h| h == needle))
        };
        if is_match {
            removed += 1;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if removed > 0 {
        std::fs::write(path, out)?;
    }
    Ok(removed)
}

/// Overwrite a **changed** host key: drop the stale `known_hosts` entry and
/// record the key the server currently presents. This is the one-click
/// equivalent of `ssh-keygen -R host` followed by a fresh trust-on-first-use,
/// for the legitimate "server was reinstalled / factory-reset" case.
///
/// `expected_fp` is the fingerprint the user just saw in the warning and
/// approved. We re-capture the key and refuse unless it still matches — the same
/// TOCTOU guard as [`trust_host_key`], so a MITM can't present a benign key in
/// the warning and swap in a different one before it gets written.
pub async fn overwrite_host_key(host: String, port: u16, expected_fp: String) -> Result<(), Error> {
    let key = capture_host_key(&host, port).await?;
    let actual_fp = fingerprint(&key);
    if actual_fp != expected_fp {
        return Err(Error::HostKeyChanged {
            fingerprint: actual_fp,
        });
    }
    let path = known_hosts_file()?;
    remove_known_hosts_lines(&path, &host, port).map_err(|e| Error::KnownHosts(e.to_string()))?;
    keys::known_hosts::learn_known_hosts(&host, port, &key)
        .map_err(|e| Error::KnownHosts(e.to_string()))?;
    // Confirm the new key now validates. If a stale entry we couldn't match
    // (e.g. a hashed one) still lingers, russh reports KeyChanged again — fail
    // loudly with a manual next step instead of leaving a silently-broken host.
    match keys::check_known_hosts(&host, port, &key) {
        Ok(true) => Ok(()),
        _ => Err(Error::KnownHosts(format!(
            "the old host key entry could not be removed automatically \
             (it may be stored hashed); remove it manually with `ssh-keygen -R {host}`"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub struct SshTransport {
    /// Shared so the app layer can open port-forwards on this session without
    /// stealing the handle from the I/O task. Every `Handle` method used takes
    /// `&self`, so users take a read lock and don't block each other.
    /// Dropping the last `Arc` closes the SSH connection.
    session: Arc<tokio::sync::RwLock<client::Handle<SshClientHandler>>>,
    channel: russh::Channel<client::Msg>,
    /// Chain of bastion handles whose `direct-tcpip` channels carry this
    /// session. Kept alive for the duration of the transport — dropping any
    /// of them would tear down the tunnel beneath us.
    _bastions: Vec<client::Handle<SshClientHandler>>,
    /// Same `Arc` the russh handler holds. Surfaced via [`start_io_loop`] so
    /// the app layer can register `-R` targets without poking russh internals.
    forwards: RemoteForwardRegistry,
    /// TCP MSS snapshot taken at connect time. `default()` for bastion / SSH-
    /// over-SSH sessions where no TCP socket is owned directly.
    tcp_mss: crate::TcpMss,
    /// Owns a duplicated socket handle so MSS can be re-queried after russh
    /// has consumed the original `TcpStream`. `None` for bastion sessions
    /// and for platforms where the dup itself failed.
    mss_watcher: Option<crate::tcp_mss::MssWatcher>,
}

impl SshTransport {
    /// TCP MSS snapshot captured at connect time. Cheap (just a struct copy).
    pub fn tcp_mss(&self) -> crate::TcpMss {
        self.tcp_mss
    }

    /// Take ownership of the live MSS watcher so the app layer can poll it
    /// from a background task. Returns `None` for bastion sessions or when
    /// the watcher couldn't be created. Once taken, subsequent calls return
    /// `None`.
    pub fn take_mss_watcher(&mut self) -> Option<crate::tcp_mss::MssWatcher> {
        self.mss_watcher.take()
    }
}

impl SshTransport {
    /// Connect, authenticate with `auth`, and open a PTY shell channel.
    ///
    /// The host key is verified strictly against `known_hosts`; callers must run
    /// [`preflight_host_key`] (and [`trust_host_key`] on approval) first for
    /// hosts that aren't already trusted.
    pub fn open_shell(
        host: String,
        port: u16,
        user: String,
        auth: SshAuth,
        cols: u16,
        rows: u16,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SshTransport, Error>> + Send + 'static>,
    > {
        Box::pin(open_shell_async(host, port, user, auth, cols, rows))
    }

    /// Like [`open_shell`], but reaches the target through a chain of bastion
    /// SSH sessions (ProxyJump). The last `Handle` in `bastions` is used to
    /// open a `direct-tcpip` channel to `target_host:target_port`; that
    /// channel's stream becomes the transport for a brand-new SSH session.
    ///
    /// `bastions` is kept alive on the returned [`SshTransport`] for the
    /// duration of the connection.
    #[allow(clippy::too_many_arguments)]
    pub fn open_shell_via(
        bastions: Vec<client::Handle<SshClientHandler>>,
        target_host: String,
        target_port: u16,
        target_user: String,
        target_auth: SshAuth,
        cols: u16,
        rows: u16,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SshTransport, Error>> + Send + 'static>,
    > {
        Box::pin(open_shell_via_async(
            bastions,
            target_host,
            target_port,
            target_user,
            target_auth,
            cols,
            rows,
        ))
    }

    /// Spawn the async I/O loop and return the command sender PLUS a shared
    /// reference to the SSH session handle (for opening port-forwards from
    /// the app layer) AND the per-session remote-forward registry. The shell
    /// stays alive until either the caller drops `cmd_tx` or the remote
    /// closes the channel.
    pub fn start_io_loop(
        self,
        on_data: impl Fn(Vec<u8>) + Send + Sync + 'static,
        on_close: impl FnOnce(crate::CloseReason) + Send + 'static,
    ) -> (
        tokio::sync::mpsc::UnboundedSender<SessionCmd>,
        Arc<tokio::sync::RwLock<client::Handle<SshClientHandler>>>,
        RemoteForwardRegistry,
    ) {
        use crate::CloseReason;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SessionCmd>();
        let (mut read_half, write_half) = self.channel.split();
        let session = self.session;
        let session_for_task = Arc::clone(&session);
        let bastions = self._bastions;
        let forwards = self.forwards;

        // Writer on its own task: `data()` waits for the server's window, and
        // the reader below must keep draining output (and with it the
        // WINDOW_ADJUSTs) meanwhile — a long paste to a device that echoes
        // every line would otherwise stall both directions for good.
        // Returns true when a write failed (link gone), false when the app
        // dropped `cmd_tx` (tab closed).
        let mut writer = tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    SessionCmd::Data(data) => {
                        if write_half.data(data.as_ref()).await.is_err() {
                            return true;
                        }
                    }
                    SessionCmd::Resize { cols, rows } => {
                        write_half
                            .window_change(cols.into(), rows.into(), 0, 0)
                            .await
                            .ok();
                    }
                }
            }
            false
        });

        tokio::spawn(async move {
            // Keep session AND any bastion handles alive for the duration of
            // the I/O loop — dropping bastions would tear the tunnel down.
            let _session = session_for_task;
            let _bastions = bastions;
            // Some servers send exit-status before the last output and EOF:
            // keep reading briefly after it so trailing bytes aren't lost.
            let mut exit_grace: Option<tokio::time::Instant> = None;
            let reason: Option<CloseReason> = loop {
                let grace = async {
                    match exit_grace {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    res = &mut writer => {
                        // Write failed → link gone; channel dropped → tab closed
                        // by the app, which needs no event.
                        break if matches!(res, Ok(true)) { Some(CloseReason::Lost) } else { None };
                    }
                    msg = read_half.wait() => match msg {
                        Some(ChannelMsg::Data { ref data }) => on_data(data.to_vec()),
                        // stderr stream — treat as regular output
                        Some(ChannelMsg::ExtendedData { ref data, .. }) => on_data(data.to_vec()),
                        Some(ChannelMsg::ExitStatus { .. }) | Some(ChannelMsg::ExitSignal { .. }) => {
                            exit_grace.get_or_insert_with(|| {
                                tokio::time::Instant::now() + std::time::Duration::from_millis(500)
                            });
                        }
                        // The shell exited / the server closed the channel.
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => {
                            break Some(CloseReason::Ended);
                        }
                        // The session went away without closing the channel
                        // (dropped link, keepalives unanswered).
                        None => {
                            break Some(if exit_grace.is_some() {
                                CloseReason::Ended
                            } else {
                                CloseReason::Lost
                            });
                        }
                        _ => {}
                    },
                    _ = grace => break Some(CloseReason::Ended),
                }
            };
            writer.abort();
            if let Some(reason) = reason {
                on_close(reason);
            }
        });

        (tx, session, forwards)
    }
}

// ---------------------------------------------------------------------------
// Private async implementation — separated from Box::pin to avoid the
// rustc 1.95.0 ICE (mir_borrowck panic in the diagnostic renderer when
// complex async logic lives directly inside Box::pin(async move {...})).
// ---------------------------------------------------------------------------

async fn open_shell_async(
    host: String,
    port: u16,
    user: String,
    auth: SshAuth,
    cols: u16,
    rows: u16,
) -> Result<SshTransport, Error> {
    // Direct connection: the UI preflight handles trust, so fail closed on
    // an unknown key here rather than silently trusting it.
    let (mut session, forwards, tcp_mss, mss_watcher) =
        connect_only(host.clone(), port, false).await?;
    let authenticated = perform_auth(&mut session, user, auth).await?;
    if !authenticated {
        return Err(Error::AuthFailed);
    }
    let channel = open_pty_shell(&mut session, cols, rows).await?;
    Ok(SshTransport {
        session: Arc::new(tokio::sync::RwLock::new(session)),
        channel,
        _bastions: Vec::new(),
        forwards,
        tcp_mss,
        mss_watcher,
    })
}

/// Open a fresh SSH session to `host:port`, drive KEX (which enforces
/// known_hosts via [`SshClientHandler`]), and return the unauthenticated
/// handle together with the per-session remote-forward registry the handler
/// holds, plus the TCP-level MSS snapshot taken right after the socket
/// connected (zeroed if the OS doesn't expose it).
///
/// We connect the TCP socket manually (instead of letting russh dial it for
/// us) so we own a `&TcpStream` we can `getsockopt` against — there's no
/// other way to recover the FD after russh swallows it.
async fn connect_only(
    host: String,
    port: u16,
    allow_tofu: bool,
) -> Result<
    (
        client::Handle<SshClientHandler>,
        RemoteForwardRegistry,
        crate::TcpMss,
        Option<crate::tcp_mss::MssWatcher>,
    ),
    Error,
> {
    // Bound the TCP connect so an unreachable host (e.g. a jump host behind a
    // firewall) fails in a few seconds with a clear error instead of hanging on
    // the OS default (~21s on Windows, surfaced as a bare "os error 10060").
    // This covers only the pre-auth socket connect, so it never truncates
    // interactive auth (password / 2FA keyboard-interactive).
    let connect = tokio::net::TcpStream::connect((host.as_str(), port));
    let stream = match tokio::time::timeout(std::time::Duration::from_secs(15), connect).await {
        Ok(res) => res.map_err(Error::Io)?,
        Err(_) => {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("connection to {host}:{port} timed out after 15s"),
            )))
        }
    };
    let mss = crate::tcp_mss::query(&stream);
    let watcher = crate::tcp_mss::MssWatcher::new(&stream);

    let config = build_client_config();
    let forwards = new_remote_forward_registry();
    let handler = SshClientHandler {
        host: host.clone(),
        port,
        allow_tofu,
        forwards: Arc::clone(&forwards),
    };
    let handle = within_handshake(client::connect_stream(config, stream, handler)).await?;
    Ok((handle, forwards, mss, watcher))
}

/// Which signature hashes to offer for an RSA key, in order. When the server
/// advertised `server-sig-algs` (EXT_INFO) we use exactly what it prefers
/// (`None` = it only speaks legacy `ssh-rsa`/SHA-1). When it didn't — typical
/// of older network gear — try `rsa-sha2-256` first, then fall back to SHA-1,
/// so both modern OpenSSH (which refuses SHA-1) and legacy devices (which
/// only accept it) authenticate. Bounded so a server that never sends
/// EXT_INFO can't stall the login.
async fn rsa_hash_candidates(session: &client::Handle<SshClientHandler>) -> Vec<Option<HashAlg>> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        session.best_supported_rsa_hash(),
    )
    .await
    {
        Ok(Ok(Some(best))) => vec![best],
        _ => vec![Some(HashAlg::Sha256), None],
    }
}

/// Authenticate `user` against an already-connected session using `auth`.
/// Returns true on success, false if the server rejected every attempt.
async fn perform_auth(
    session: &mut client::Handle<SshClientHandler>,
    user: String,
    auth: SshAuth,
) -> Result<bool, Error> {
    match auth {
        SshAuth::Password(password) => {
            let result = session
                .authenticate_password(user.clone(), password.clone())
                .await
                .map_err(Error::Ssh)?;
            match result {
                AuthResult::Success => Ok(true),
                // Many PAM servers turn plain password auth off and ask for
                // the password through keyboard-interactive instead; answer
                // it with the same password, as OpenSSH does.
                AuthResult::Failure {
                    remaining_methods, ..
                } if remaining_methods.contains(&russh::MethodKind::KeyboardInteractive) => {
                    password_via_keyboard_interactive(session, user, &password).await
                }
                AuthResult::Failure { .. } => Ok(false),
            }
        }
        SshAuth::PublicKey {
            key_path,
            passphrase,
        } => {
            let key = keys::load_secret_key(expand_tilde(&key_path), passphrase.as_deref())
                .map_err(|e| match e {
                    // Encrypted key with no/empty passphrase.
                    keys::Error::KeyIsEncrypted => Error::KeyPassphrase,
                    // A missing/unreadable file is that, whatever passphrase
                    // was given — not "wrong passphrase".
                    keys::Error::IO(io) => Error::KeyLoad(format!("{}: {io}", key_path)),
                    // Decrypting with the given passphrase failed: most
                    // likely a wrong one.
                    _ if passphrase.is_some() => Error::KeyPassphrase,
                    other => Error::KeyLoad(other.to_string()),
                })?;
            let key = Arc::new(key);
            let candidates = if key.algorithm().is_rsa() {
                rsa_hash_candidates(session).await
            } else {
                vec![None]
            };
            for hash_alg in candidates {
                let result = session
                    .authenticate_publickey(
                        user.clone(),
                        PrivateKeyWithHashAlg::new(Arc::clone(&key), hash_alg),
                    )
                    .await
                    .map_err(Error::Ssh)?;
                if matches!(result, AuthResult::Success) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        SshAuth::KeyboardInteractive { responder } => {
            let mut response = session
                .authenticate_keyboard_interactive_start(user, None)
                .await
                .map_err(Error::Ssh)?;
            loop {
                match response {
                    KeyboardInteractiveAuthResponse::Success => return Ok(true),
                    KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
                    // No questions (some PAM stacks send one after the last
                    // step, or one carrying only instructions): answer it
                    // empty instead of showing the user a blank dialog.
                    KeyboardInteractiveAuthResponse::InfoRequest { ref prompts, .. }
                        if prompts.is_empty() =>
                    {
                        response = session
                            .authenticate_keyboard_interactive_respond(Vec::new())
                            .await
                            .map_err(Error::Ssh)?;
                    }
                    KeyboardInteractiveAuthResponse::InfoRequest {
                        name,
                        instructions,
                        prompts,
                    } => {
                        let req = KiRequest {
                            name,
                            instructions,
                            prompts: prompts
                                .into_iter()
                                .map(|p| KiPrompt {
                                    prompt: p.prompt,
                                    echo: p.echo,
                                })
                                .collect(),
                        };
                        let answers = (responder)(req).await;
                        response = session
                            .authenticate_keyboard_interactive_respond(answers)
                            .await
                            .map_err(Error::Ssh)?;
                    }
                }
            }
        }
        SshAuth::Agent { priority } => agent_auth(session, user, priority).await,
    }
}

/// Keyboard-interactive login that answers every hidden prompt with
/// `password`. Gives up (false) on a visible prompt (an OTP code, a username)
/// or after a few rounds — those need the interactive dialog.
async fn password_via_keyboard_interactive(
    session: &mut client::Handle<SshClientHandler>,
    user: String,
    password: &str,
) -> Result<bool, Error> {
    let mut response = session
        .authenticate_keyboard_interactive_start(user, None)
        .await
        .map_err(Error::Ssh)?;
    for _ in 0..4 {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                if prompts.iter().any(|p| p.echo) {
                    return Ok(false);
                }
                let answers = prompts.iter().map(|_| password.to_string()).collect();
                response = session
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(Error::Ssh)?;
            }
        }
    }
    Ok(false)
}

async fn open_pty_shell(
    session: &mut client::Handle<SshClientHandler>,
    cols: u16,
    rows: u16,
) -> Result<russh::Channel<client::Msg>, Error> {
    let channel = session.channel_open_session().await.map_err(Error::Ssh)?;
    channel
        .request_pty(false, "xterm-256color", cols.into(), rows.into(), 0, 0, &[])
        .await
        .map_err(Error::Ssh)?;
    channel.request_shell(false).await.map_err(Error::Ssh)?;
    Ok(channel)
}

/// Connect to an SSH host and authenticate, but do NOT open a shell.
///
/// Used by the app layer to build bastion handles for ProxyJump chains —
/// the resulting [`client::Handle`] is fed to [`SshTransport::open_shell_via`]
/// (possibly after opening another `direct-tcpip` channel through it for the
/// next bastion in the chain).
pub async fn connect_authenticated(
    host: String,
    port: u16,
    user: String,
    auth: SshAuth,
) -> Result<client::Handle<SshClientHandler>, Error> {
    // Bastion-only path: -R against a bastion is not supported, so we drop
    // the registry returned by `connect_only`. MSS is dropped too — we only
    // care about the FINAL session's MSS, not each bastion hop. This first
    // hop is a direct TCP connection, so the UI preflights its host key like
    // any direct host (see `ssh_direct_endpoint` in the app): no TOFU here — an
    // unknown key is rejected rather than trusted, and its credentials sent,
    // blindly.
    let (mut session, _bastion_forwards, _bastion_mss, _bastion_watcher) =
        connect_only(host, port, false).await?;
    let authenticated = perform_auth(&mut session, user, auth).await?;
    if !authenticated {
        return Err(Error::AuthFailed);
    }
    Ok(session)
}

/// Connect to an SSH host THROUGH an already-authenticated bastion, then
/// authenticate the new connection. Used when chaining several jump hosts.
pub async fn connect_authenticated_via(
    bastion: &client::Handle<SshClientHandler>,
    host: String,
    port: u16,
    user: String,
    auth: SshAuth,
) -> Result<client::Handle<SshClientHandler>, Error> {
    let channel = bastion
        .channel_open_direct_tcpip(host.clone(), u32::from(port), "127.0.0.1", 0)
        .await
        .map_err(Error::Ssh)?;
    let stream = channel.into_stream();
    let config = build_client_config();
    // Bastion: discard the registry — the resulting handle is only used as
    // a hop for direct-tcpip channels, never the final SSH session. Reached
    // through a tunnel, so it can't be preflighted: TOFU on first use.
    let handler = SshClientHandler {
        host: host.clone(),
        port,
        allow_tofu: true,
        forwards: new_remote_forward_registry(),
    };
    let mut session = within_handshake(client::connect_stream(config, stream, handler)).await?;
    let authenticated = perform_auth(&mut session, user, auth).await?;
    if !authenticated {
        return Err(Error::AuthFailed);
    }
    Ok(session)
}

/// Open an SSH session to `target_host:target_port` over a `direct-tcpip`
/// channel through the LAST handle in `bastions`. The whole chain is kept
/// alive on the returned [`SshTransport`].
async fn open_shell_via_async(
    bastions: Vec<client::Handle<SshClientHandler>>,
    target_host: String,
    target_port: u16,
    target_user: String,
    target_auth: SshAuth,
    cols: u16,
    rows: u16,
) -> Result<SshTransport, Error> {
    let last = bastions.last().ok_or_else(|| {
        Error::InvalidSetup("open_shell_via called with empty bastion chain".into())
    })?;

    // Tunnel to (target_host, target_port) via the last bastion.
    let channel = last
        .channel_open_direct_tcpip(target_host.clone(), u32::from(target_port), "127.0.0.1", 0)
        .await
        .map_err(Error::Ssh)?;
    let stream = channel.into_stream();

    // Run a fresh SSH client over that tunneled stream. The known_hosts
    // handler is keyed to the TARGET, not the bastion. The target is only
    // reachable through the tunnel, so the UI can't preflight it: TOFU on
    // first use (the learned fingerprint is logged).
    let config = build_client_config();
    let forwards = new_remote_forward_registry();
    let handler = SshClientHandler {
        host: target_host.clone(),
        port: target_port,
        allow_tofu: true,
        forwards: Arc::clone(&forwards),
    };
    let mut session = within_handshake(client::connect_stream(config, stream, handler)).await?;

    let authenticated = perform_auth(&mut session, target_user, target_auth).await?;
    if !authenticated {
        return Err(Error::AuthFailed);
    }

    let channel = open_pty_shell(&mut session, cols, rows).await?;

    Ok(SshTransport {
        session: Arc::new(tokio::sync::RwLock::new(session)),
        channel,
        _bastions: bastions,
        forwards,
        // Bastion sessions live inside an SSH tunnel — there's no direct TCP
        // socket whose MSS would be meaningful. Surface empty.
        tcp_mss: crate::TcpMss::default(),
        mss_watcher: None,
    })
}

// ---------------------------------------------------------------------------
// SSH agent authentication (Phase 3)
//
// Connect to the OS ssh-agent, enumerate identities, and try each one against
// the server via `authenticate_future` (which threads the `Signer` back out so
// we can reuse it across attempts).
//
// * Unix: `AgentClient::connect_env()` (uses `$SSH_AUTH_SOCK`).
// * Windows: try OpenSSH-for-Windows first (named pipe at the path given by
//   `$SSH_AUTH_SOCK` or `\\.\pipe\openssh-ssh-agent` by default), then fall
//   back to Pageant (PuTTY) via shared memory + WM_COPYDATA. Both shapes
//   share the same identity-enumeration + signing loop below.
// ---------------------------------------------------------------------------

#[cfg(unix)]
async fn agent_auth(
    session: &mut client::Handle<SshClientHandler>,
    user: String,
    // Only used on Windows; accepted here so the call site can stay
    // platform-agnostic. `_` to keep clippy quiet on Unix builds.
    _priority: AgentPriority,
) -> Result<bool, Error> {
    let agent = AgentClient::connect_env()
        .await
        .map_err(|e| Error::Agent(format!("no SSH agent available: {e}")))?;
    run_agent_auth(session, user, agent).await
}

#[cfg(windows)]
async fn agent_auth(
    session: &mut client::Handle<SshClientHandler>,
    user: String,
    priority: AgentPriority,
) -> Result<bool, Error> {
    // Build the ordered list of agent backends to try based on the user's
    // priority. `*Only` variants skip the fallback entirely so a config error
    // ("I told you to use Pageant only") fails loudly rather than silently
    // succeeding against the wrong agent.
    let backends: &[AgentBackend] = match priority {
        AgentPriority::Auto => &[AgentBackend::OpenSsh, AgentBackend::Pageant],
        AgentPriority::PageantFirst => &[AgentBackend::Pageant, AgentBackend::OpenSsh],
        AgentPriority::OpenSshOnly => &[AgentBackend::OpenSsh],
        AgentPriority::PageantOnly => &[AgentBackend::Pageant],
    };

    let mut errors: Vec<String> = Vec::new();
    for backend in backends {
        match try_windows_backend(*backend, session, user.clone()).await {
            Ok(true) => return Ok(true),
            // Identities found but server rejected all of them — that's a
            // final verdict for this backend; try the next one in priority
            // because the OTHER agent may hold a key the server accepts.
            Ok(false) => {
                errors.push(format!(
                    "{}: no identity accepted by server",
                    backend.label()
                ));
            }
            Err(e) => errors.push(format!("{}: {e}", backend.label())),
        }
    }
    Err(Error::Agent(format!(
        "no usable SSH agent on Windows ({})",
        errors.join("; ")
    )))
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
enum AgentBackend {
    OpenSsh,
    Pageant,
}

#[cfg(windows)]
impl AgentBackend {
    fn label(self) -> &'static str {
        match self {
            Self::OpenSsh => "OpenSSH",
            Self::Pageant => "Pageant",
        }
    }
}

#[cfg(windows)]
async fn try_windows_backend(
    backend: AgentBackend,
    session: &mut client::Handle<SshClientHandler>,
    user: String,
) -> Result<bool, Error> {
    match backend {
        AgentBackend::OpenSsh => {
            // `$SSH_AUTH_SOCK` is the way Microsoft's port advertises its
            // path; we fall back to the well-known default for older setups.
            let pipe_path = std::env::var_os("SSH_AUTH_SOCK")
                .unwrap_or_else(|| std::ffi::OsString::from(r"\\.\pipe\openssh-ssh-agent"));
            let agent = AgentClient::connect_named_pipe(&pipe_path)
                .await
                .map_err(|e| {
                    Error::Agent(format!("named pipe '{}': {e}", pipe_path.to_string_lossy()))
                })?;
            run_agent_auth(session, user, agent).await
        }
        AgentBackend::Pageant => {
            // A missing Pageant process is reported here (or, for some
            // failure modes, on the first `request_identities()` call inside
            // the shared helper) — both surface as an `Agent` error naming
            // the backend, so the caller's per-backend summary stays clear.
            let agent = AgentClient::connect_pageant()
                .await
                .map_err(|e| Error::Agent(format!("Pageant: {e}")))?;
            run_agent_auth(session, user, agent).await
        }
    }
}

/// Identity-enumeration + signing loop shared by Unix and Windows agent
/// front-ends. Generic over the agent's stream type so we monomorphise once
/// per platform without duplicating the body. The `AgentClient` itself is the
/// `Signer`: each attempt asks the agent to sign the auth challenge for one
/// of its keys, so no private key ever leaves the agent.
async fn run_agent_auth<R>(
    session: &mut client::Handle<SshClientHandler>,
    user: String,
    mut agent: AgentClient<R>,
) -> Result<bool, Error>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| Error::Agent(format!("failed to list agent identities: {e}")))?;

    if identities.is_empty() {
        return Err(Error::Agent("ssh-agent has no identities loaded".into()));
    }

    // Resolved lazily: only needed if the agent holds an RSA key, and it
    // costs a round-trip wait for the server's EXT_INFO.
    let mut rsa_candidates: Option<Vec<Option<HashAlg>>> = None;
    for identity in identities {
        let key = match identity {
            AgentIdentity::PublicKey { key, .. } => key,
            // Certificate identities need `authenticate_certificate_with`;
            // ZapTerm has never supported user certificates, so skip them rather
            // than fail the whole agent pass.
            AgentIdentity::Certificate { .. } => continue,
        };
        let candidates = if key.algorithm().is_rsa() {
            if rsa_candidates.is_none() {
                rsa_candidates = Some(rsa_hash_candidates(session).await);
            }
            rsa_candidates.clone().unwrap_or_default()
        } else {
            vec![None]
        };
        for hash_alg in candidates {
            match session
                .authenticate_publickey_with(user.clone(), key.clone(), hash_alg, &mut agent)
                .await
            {
                Ok(AuthResult::Success) => return Ok(true),
                // Rejected: try the next hash / identity.
                Ok(AuthResult::Failure { .. }) => {}
                // The agent refused to sign (locked, key removed, IPC error):
                // that identity is unusable, but another may still work.
                Err(e) => tracing::debug!("agent signing failed for one identity: {e}"),
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::{expand_tilde, merge_known_hosts, remove_known_hosts_lines, AgentPriority};

    #[test]
    fn expand_tilde_handles_unix_and_windows_forms() {
        let home = home::home_dir().unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(
            expand_tilde("~/.ssh/id_ed25519"),
            home.join(".ssh/id_ed25519")
        );
        assert_eq!(expand_tilde("~\\.ssh\\id"), home.join(".ssh\\id"));
        assert_eq!(
            expand_tilde("/abs/key"),
            std::path::PathBuf::from("/abs/key")
        );
        assert_eq!(
            expand_tilde("~other/key"),
            std::path::PathBuf::from("~other/key")
        );
    }

    /// The legacy-Windows migration merges only lines the standard file lacks,
    /// skips blanks/comments, creates the target (and its directory) when
    /// missing, and is idempotent.
    #[test]
    fn merge_known_hosts_appends_only_missing_entries() {
        let dir = std::env::temp_dir().join(format!("zapterm-kh-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("legacy_known_hosts");
        let target = dir.join("dotssh").join("known_hosts");
        std::fs::write(
            &legacy,
            "# old file\n\
             10.0.0.1 ssh-ed25519 AAAAONE\n\
             \n\
             10.0.0.2 ssh-ed25519 AAAATWO\n",
        )
        .unwrap();

        // Target doesn't exist yet: both entries land, directory is created.
        assert_eq!(merge_known_hosts(&legacy, &target).unwrap(), 2);
        let after = std::fs::read_to_string(&target).unwrap();
        assert_eq!(
            after,
            "10.0.0.1 ssh-ed25519 AAAAONE\n10.0.0.2 ssh-ed25519 AAAATWO\n"
        );

        // Second run adds nothing (idempotent), file untouched.
        assert_eq!(merge_known_hosts(&legacy, &target).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), after);

        // A target that already has one of the lines only gains the other.
        std::fs::write(&target, "10.0.0.2 ssh-ed25519 AAAATWO").unwrap(); // no trailing newline
        assert_eq!(merge_known_hosts(&legacy, &target).unwrap(), 1);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "10.0.0.2 ssh-ed25519 AAAATWO\n10.0.0.1 ssh-ed25519 AAAAONE\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A merge never drops what the target holds: an unreadable target is an
    /// error (not "empty"), bytes that aren't UTF-8 survive, and an incoming
    /// older key for a host + key type already present is not appended.
    #[test]
    fn merge_known_hosts_keeps_existing_entries() {
        let dir = std::env::temp_dir().join(format!("zapterm-kh-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("known_hosts");

        // Latin-1 comment byte + an entry; the merge appends, never rewrites.
        let mut original = b"# caf\xe9\n10.0.0.1 ssh-ed25519 AAAANEW\n".to_vec();
        std::fs::write(&target, &original).unwrap();
        let incoming = "10.0.0.1 ssh-ed25519 AAAAOLD\n\
                        10.0.0.1 ssh-rsa AAAARSA\n\
                        10.0.0.3 ssh-ed25519 AAAATHREE\n";
        assert_eq!(super::merge_known_hosts_text(incoming, &target).unwrap(), 2);
        original.extend_from_slice(b"10.0.0.1 ssh-rsa AAAARSA\n10.0.0.3 ssh-ed25519 AAAATHREE\n");
        assert_eq!(std::fs::read(&target).unwrap(), original);

        // A target that can't be read (here: a directory) is an error, and
        // nothing is written.
        let not_a_file = dir.join("dir_target");
        std::fs::create_dir_all(&not_a_file).unwrap();
        assert!(super::merge_known_hosts_text(incoming, &not_a_file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `remove_known_hosts_lines` drops exactly the entries for the target host
    /// (plain and `host,ip` forms), keeps everything else, and preserves
    /// comments and unrelated hosts.
    #[test]
    fn remove_known_hosts_drops_matching_host() {
        let dir = std::env::temp_dir().join(format!("zapterm-kh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("known_hosts");
        std::fs::write(
            &path,
            "# a comment\n\
             172.19.14.187 ssh-ed25519 AAAAOLD\n\
             other.example.com,10.0.0.9 ssh-ed25519 AAAAKEEP\n\
             172.19.14.187,172.19.14.187 ssh-rsa AAAAOLD2\n\
             172.19.14.1 ssh-ed25519 AAAAKEEP2\n",
        )
        .unwrap();

        let removed = remove_known_hosts_lines(&path, "172.19.14.187", 22).unwrap();
        assert_eq!(removed, 2);

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            !after.contains("AAAAOLD"),
            "old key lines must be gone: {after}"
        );
        assert!(after.contains("# a comment"));
        assert!(after.contains("other.example.com"));
        assert!(after.contains("172.19.14.1 ")); // the .1 host is a different host
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Non-standard ports are keyed `[host]:port`; a bare host must NOT match a
    /// `[host]:port` entry and vice-versa. Missing file = 0 removed, no error.
    #[test]
    fn remove_known_hosts_port_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("zapterm-kh-port-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("known_hosts");
        std::fs::write(
            &path,
            "[10.0.0.5]:2222 ssh-ed25519 AAAAPORT\n10.0.0.5 ssh-ed25519 AAAABARE\n",
        )
        .unwrap();

        // Removing host on port 2222 only touches the bracketed entry.
        assert_eq!(
            remove_known_hosts_lines(&path, "10.0.0.5", 2222).unwrap(),
            1
        );
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("10.0.0.5 ssh-ed25519 AAAABARE"));
        assert!(!after.contains("AAAAPORT"));

        // A missing file is a no-op, not an error.
        let missing = dir.join("nope").join("known_hosts");
        assert_eq!(remove_known_hosts_lines(&missing, "x", 22).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn priority_round_trip() {
        for v in [
            AgentPriority::Auto,
            AgentPriority::PageantFirst,
            AgentPriority::OpenSshOnly,
            AgentPriority::PageantOnly,
        ] {
            assert_eq!(AgentPriority::from_setting(v.as_setting()), v);
        }
    }

    #[test]
    fn priority_accepts_common_variants() {
        // Underscore form (some users hand-edit the DB), trailing whitespace,
        // mixed case — all should resolve cleanly.
        assert_eq!(AgentPriority::from_setting(""), AgentPriority::Auto);
        assert_eq!(AgentPriority::from_setting("AUTO"), AgentPriority::Auto);
        assert_eq!(
            AgentPriority::from_setting("pageant_first"),
            AgentPriority::PageantFirst
        );
        assert_eq!(
            AgentPriority::from_setting("  Pageant-First  "),
            AgentPriority::PageantFirst
        );
        assert_eq!(
            AgentPriority::from_setting("openssh_only"),
            AgentPriority::OpenSshOnly
        );
    }

    #[test]
    fn priority_unknown_falls_back_to_auto() {
        // A stale or corrupted setting must never block the user.
        assert_eq!(AgentPriority::from_setting("bogus"), AgentPriority::Auto);
        assert_eq!(
            AgentPriority::from_setting("openssh,pageant"),
            AgentPriority::Auto
        );
    }
}
