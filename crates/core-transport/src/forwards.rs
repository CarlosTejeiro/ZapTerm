//! Port forwarding over an established SSH session.
//!
//! Implements:
//! - **Local forward (`-L`)**: bind a local TCP port; for each incoming TCP
//!   connection open a `direct-tcpip` channel to a fixed `(target_host, target_port)`
//!   on the SSH side and bridge bytes both ways.
//! - **Dynamic forward (`-D`)**: bind a local TCP port as a SOCKS5 server;
//!   each accepted client's CONNECT request determines the target host/port
//!   for the `direct-tcpip` channel.
//! - **Remote forward (`-R`)**: ask the server to listen on a remote bind
//!   port; each incoming connection arrives as a `forwarded-tcpip` channel
//!   handled by [`crate::ssh::SshClientHandler::server_channel_open_forwarded_tcpip`],
//!   which looks the target up in the shared [`RemoteForwardRegistry`] and
//!   bridges to a local TCP socket.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

use crate::error::Error;
use crate::ssh::{RemoteForwardRegistry, RemoteTarget, SshClientHandler};

/// Live metadata for a forward, shipped to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForwardInfo {
    pub id: String,
    /// `"local"`, `"dynamic"`, or `"remote"`.
    pub kind: String,
    /// For local/dynamic: the local TCP listener address. For remote: the
    /// remote bind address requested from the server.
    pub bind_addr: String,
    /// For local/dynamic: the OS-resolved local port. For remote: the port
    /// the server actually bound (0 in the request means "server picks").
    pub bind_port: u16,
    /// `Some((host, port))` for `local` and `remote`; `None` for `dynamic`
    /// (SOCKS picks per connection).
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    /// The user's name for this tunnel (from the saved forward, or typed when
    /// adding it by hand); `None` = unnamed.
    pub name: Option<String>,
}

/// Cleanup the controller fires on drop. Local/dynamic forwards just abort
/// the listener task; remote forwards have to send `cancel-tcpip-forward`
/// to the server AND drop the registry entry so further channels are
/// refused — that's async, so we keep a boxed closure that returns a future.
type DropCleanup =
    Box<dyn FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>;

/// Owns the resources behind one forward — listener task (local/dynamic) or
/// the server-side registration (remote). Dropping fires the cleanup so the
/// app layer can just store and drop these via `Vec<ForwardController>`.
pub struct ForwardController {
    pub info: ForwardInfo,
    task: Option<tokio::task::AbortHandle>,
    bridges: Bridges,
    cleanup: Option<DropCleanup>,
}

impl Drop for ForwardController {
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        for t in self.bridges.lock().unwrap().drain(..) {
            t.abort();
        }
        if let Some(c) = self.cleanup.take() {
            // Best-effort: detach the async cleanup. Without a runtime (a
            // drop from a plain thread, or after shutdown) it can't run —
            // `tokio::spawn` would panic, and a panic in Drop can abort the
            // process — so skip it: the connection is closing anyway.
            if let Ok(rt) = tokio::runtime::Handle::try_current() {
                rt.spawn(c());
            }
        }
    }
}

/// Per-connection bridge tasks of a forward, aborted with it: closing a
/// forward (or its tab) ends the tunnels already open through it too —
/// otherwise each one would keep the SSH connection alive.
type Bridges = Arc<std::sync::Mutex<Vec<tokio::task::AbortHandle>>>;

/// Track a new bridge task, dropping finished ones so the list stays small.
fn track_bridge(bridges: &Bridges, task: tokio::task::AbortHandle) {
    let mut list = bridges.lock().unwrap();
    list.retain(|t| !t.is_finished());
    list.push(task);
}

/// Accept errors that only mean "try again" (one connection aborted before
/// we got it, or a momentary fd shortage): keep the listener alive.
fn accept_error_is_transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(
        e.kind(),
        ConnectionAborted | ConnectionReset | Interrupted | WouldBlock
    ) || matches!(e.raw_os_error(), Some(23 | 24)) // ENFILE / EMFILE
}

/// Open a local TCP listener on `bind_addr:bind_port`; tunnel every connection
/// to `(target_host, target_port)` via `direct-tcpip` over `handle`.
pub async fn open_local_forward(
    handle: Arc<tokio::sync::RwLock<russh::client::Handle<SshClientHandler>>>,
    bind_addr: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
) -> Result<ForwardController, Error> {
    let listener = TcpListener::bind((bind_addr.as_str(), bind_port))
        .await
        .map_err(Error::Io)?;
    // Re-read the bound port in case the caller asked for 0 (OS-assigned).
    let local_addr = listener.local_addr().map_err(Error::Io)?;
    let resolved_bind_port = local_addr.port();

    let target_host_loop = target_host.clone();
    let bridges: Bridges = Arc::default();
    let bridges_loop = Arc::clone(&bridges);
    let join = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let handle = Arc::clone(&handle);
                    let target_host = target_host_loop.clone();
                    let task = tokio::spawn(async move {
                        if let Err(e) =
                            bridge_local(stream, peer, handle, target_host, target_port).await
                        {
                            tracing::debug!("local forward bridge ended: {e}");
                        }
                    });
                    track_bridge(&bridges_loop, task.abort_handle());
                }
                Err(e) if accept_error_is_transient(&e) => {
                    tracing::debug!("local forward accept error (retrying): {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::warn!("local forward accept error: {e}");
                    break;
                }
            }
        }
    });

    Ok(ForwardController {
        info: ForwardInfo {
            id: Uuid::new_v4().to_string(),
            kind: "local".to_string(),
            bind_addr,
            bind_port: resolved_bind_port,
            target_host: Some(target_host),
            target_port: Some(target_port),
            name: None,
        },
        task: Some(join.abort_handle()),
        bridges,
        cleanup: None,
    })
}

/// Open a local TCP listener serving SOCKS5; each accepted client's CONNECT
/// request determines the target for the `direct-tcpip` channel.
pub async fn open_dynamic_forward(
    handle: Arc<tokio::sync::RwLock<russh::client::Handle<SshClientHandler>>>,
    bind_addr: String,
    bind_port: u16,
) -> Result<ForwardController, Error> {
    let listener = TcpListener::bind((bind_addr.as_str(), bind_port))
        .await
        .map_err(Error::Io)?;
    let local_addr = listener.local_addr().map_err(Error::Io)?;
    let resolved_bind_port = local_addr.port();

    let bridges: Bridges = Arc::default();
    let bridges_loop = Arc::clone(&bridges);
    let join = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let handle = Arc::clone(&handle);
                    let task = tokio::spawn(async move {
                        if let Err(e) = bridge_socks5(stream, peer, handle).await {
                            tracing::debug!("dynamic forward bridge ended: {e}");
                        }
                    });
                    track_bridge(&bridges_loop, task.abort_handle());
                }
                Err(e) if accept_error_is_transient(&e) => {
                    tracing::debug!("dynamic forward accept error (retrying): {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::warn!("dynamic forward accept error: {e}");
                    break;
                }
            }
        }
    });

    Ok(ForwardController {
        info: ForwardInfo {
            id: Uuid::new_v4().to_string(),
            kind: "dynamic".to_string(),
            bind_addr,
            bind_port: resolved_bind_port,
            target_host: None,
            target_port: None,
            name: None,
        },
        task: Some(join.abort_handle()),
        bridges,
        cleanup: None,
    })
}

/// Ask the SSH server to listen on `bind_addr:bind_port` and tunnel every
/// inbound connection back to us via `forwarded-tcpip`. Each channel is
/// handled by [`crate::ssh::SshClientHandler::server_channel_open_forwarded_tcpip`],
/// which bridges to `(target_host, target_port)` on this machine.
///
/// If `bind_port` is `0`, the server picks the port — the returned
/// [`ForwardInfo::bind_port`] reflects the actual port the server bound.
///
/// Drop the returned controller to release the forward: the registry entry
/// is removed and a `cancel-tcpip-forward` request is sent on a detached
/// task so the server stops accepting new connections.
pub async fn open_remote_forward(
    handle: Arc<tokio::sync::RwLock<russh::client::Handle<SshClientHandler>>>,
    registry: RemoteForwardRegistry,
    bind_addr: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
) -> Result<ForwardController, Error> {
    // Register the target BEFORE telling the server, so the moment it starts
    // forwarding channels we know where to send them.
    let key = (bind_addr.clone(), bind_port);
    {
        let mut map = registry.lock().unwrap();
        // A forward on this bind is already live: refuse rather than
        // overwrite its target (and have the rollback below delete it).
        if bind_port != 0 && map.contains_key(&key) {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!("a remote forward on {bind_addr}:{bind_port} is already active"),
            )));
        }
        map.insert(
            key.clone(),
            RemoteTarget {
                host: target_host.clone(),
                port: target_port,
            },
        );
    }

    // Some servers reply with an unsolicited port when bind_port == 0; russh
    // exposes it as the future's return value. Non-zero requests get back the
    // same port (or 0 to indicate "OK" with a literal bind on the requested
    // port — we treat 0 as "use what we asked for").
    let granted = match handle
        .read()
        .await
        .tcpip_forward(bind_addr.clone(), u32::from(bind_port))
        .await
    {
        Ok(p) => p,
        Err(e) => {
            // Rollback the registry entry if the server refuses.
            registry.lock().unwrap().remove(&key);
            return Err(Error::Ssh(e));
        }
    };
    let resolved_port: u16 = if granted == 0 {
        bind_port
    } else {
        u16::try_from(granted).unwrap_or(bind_port)
    };

    // If the server assigned a different port than we requested, re-key the
    // registry entry so the callback can look it up.
    if resolved_port != bind_port {
        let mut map = registry.lock().unwrap();
        if let Some(t) = map.remove(&key) {
            map.insert((bind_addr.clone(), resolved_port), t);
        }
    }

    let cleanup_handle = Arc::clone(&handle);
    let cleanup_registry = Arc::clone(&registry);
    let cleanup_addr = bind_addr.clone();
    let cleanup: DropCleanup = Box::new(move || {
        Box::pin(async move {
            cleanup_registry
                .lock()
                .unwrap()
                .remove(&(cleanup_addr.clone(), resolved_port));
            // `cancel_tcpip_forward` is `&self` on russh's Handle, so the
            // shared lock is enough here.
            if let Err(e) = cleanup_handle
                .read()
                .await
                .cancel_tcpip_forward(cleanup_addr.clone(), u32::from(resolved_port))
                .await
            {
                tracing::debug!("cancel_tcpip_forward({cleanup_addr}:{resolved_port}) failed: {e}");
            }
        })
    });

    Ok(ForwardController {
        info: ForwardInfo {
            id: Uuid::new_v4().to_string(),
            kind: "remote".to_string(),
            bind_addr,
            bind_port: resolved_port,
            target_host: Some(target_host),
            target_port: Some(target_port),
            name: None,
        },
        task: None,
        bridges: Arc::default(),
        cleanup: Some(cleanup),
    })
}

// ---------------------------------------------------------------------------
// Bridges
// ---------------------------------------------------------------------------

async fn bridge_local(
    mut local: TcpStream,
    peer: SocketAddr,
    handle: Arc<tokio::sync::RwLock<russh::client::Handle<SshClientHandler>>>,
    target_host: String,
    target_port: u16,
) -> Result<(), Error> {
    let channel = handle
        .read()
        .await
        .channel_open_direct_tcpip(
            target_host,
            u32::from(target_port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await
        .map_err(Error::Ssh)?;
    let mut remote = channel.into_stream();
    tokio::io::copy_bidirectional(&mut local, &mut remote)
        .await
        .map_err(Error::Io)?;
    Ok(())
}

async fn bridge_socks5(
    mut local: TcpStream,
    peer: SocketAddr,
    handle: Arc<tokio::sync::RwLock<russh::client::Handle<SshClientHandler>>>,
) -> Result<(), Error> {
    let (target_host, target_port) = socks5_handshake(&mut local).await?;
    let channel_result = handle
        .read()
        .await
        .channel_open_direct_tcpip(
            target_host,
            u32::from(target_port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await;

    match channel_result {
        Ok(channel) => {
            // SOCKS5 success reply (REP=0x00, BND.ADDR=0.0.0.0:0).
            local
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .map_err(Error::Io)?;
            let mut remote = channel.into_stream();
            tokio::io::copy_bidirectional(&mut local, &mut remote)
                .await
                .map_err(Error::Io)?;
            Ok(())
        }
        Err(e) => {
            // SOCKS5 general failure.
            let _ = local
                .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            Err(Error::Ssh(e))
        }
    }
}

// ---------------------------------------------------------------------------
// SOCKS5 (RFC 1928) — CONNECT only, no auth.
// ---------------------------------------------------------------------------

async fn socks5_handshake(s: &mut TcpStream) -> Result<(String, u16), Error> {
    // Greeting: [VER=5, NMETHODS, METHODS...]
    let mut head = [0u8; 2];
    s.read_exact(&mut head).await.map_err(Error::Io)?;
    if head[0] != 0x05 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not SOCKS5",
        )));
    }
    let mut methods = vec![0u8; head[1] as usize];
    s.read_exact(&mut methods).await.map_err(Error::Io)?;
    // Reply: pick "no auth" (0x00). If the client didn't offer it, reject.
    if !methods.contains(&0x00) {
        s.write_all(&[0x05, 0xFF]).await.map_err(Error::Io)?;
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "SOCKS5: no acceptable auth method",
        )));
    }
    s.write_all(&[0x05, 0x00]).await.map_err(Error::Io)?;

    // Request: [VER=5, CMD, RSV=0, ATYP, DST.ADDR, DST.PORT]
    let mut req = [0u8; 4];
    s.read_exact(&mut req).await.map_err(Error::Io)?;
    if req[0] != 0x05 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "SOCKS5 request version",
        )));
    }
    if req[1] != 0x01 {
        // Only CONNECT supported; reply with command-not-supported.
        s.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .map_err(Error::Io)?;
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "SOCKS5: only CONNECT supported",
        )));
    }
    let host = match req[3] {
        0x01 => {
            let mut a = [0u8; 4];
            s.read_exact(&mut a).await.map_err(Error::Io)?;
            format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3])
        }
        0x03 => {
            let mut len = [0u8; 1];
            s.read_exact(&mut len).await.map_err(Error::Io)?;
            let mut name = vec![0u8; len[0] as usize];
            s.read_exact(&mut name).await.map_err(Error::Io)?;
            String::from_utf8(name).map_err(|_| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "SOCKS5: non-UTF8 hostname",
                ))
            })?
        }
        0x04 => {
            let mut a = [0u8; 16];
            s.read_exact(&mut a).await.map_err(Error::Io)?;
            std::net::Ipv6Addr::from(a).to_string()
        }
        _ => {
            s.write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .map_err(Error::Io)?;
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "SOCKS5: unsupported ATYP",
            )));
        }
    };
    let mut port_bytes = [0u8; 2];
    s.read_exact(&mut port_bytes).await.map_err(Error::Io)?;
    let port = u16::from_be_bytes(port_bytes);
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> ForwardInfo {
        ForwardInfo {
            id: "t".into(),
            kind: "remote".into(),
            bind_addr: "127.0.0.1".into(),
            bind_port: 1,
            target_host: None,
            target_port: None,
            name: None,
        }
    }

    /// Dropping a controller outside a Tokio runtime must not panic (a panic
    /// in Drop during unwinding aborts the process).
    #[test]
    fn drop_without_a_runtime_skips_the_cleanup() {
        let cleanup: DropCleanup = Box::new(|| Box::pin(async {}));
        let c = ForwardController {
            info: info(),
            task: None,
            bridges: Arc::default(),
            cleanup: Some(cleanup),
        };
        drop(c);
    }

    /// Closing a forward also ends the tunnels already open through it.
    #[tokio::test]
    async fn drop_aborts_open_bridges() {
        let bridges: Bridges = Arc::default();
        let bridge = tokio::spawn(std::future::pending::<()>());
        track_bridge(&bridges, bridge.abort_handle());
        drop(ForwardController {
            info: info(),
            task: None,
            bridges,
            cleanup: None,
        });
        assert!(bridge.await.unwrap_err().is_cancelled());
    }

    #[test]
    fn only_transient_accept_errors_keep_the_listener() {
        use std::io::{Error, ErrorKind};
        assert!(accept_error_is_transient(&Error::from(
            ErrorKind::ConnectionAborted
        )));
        assert!(accept_error_is_transient(&Error::from_raw_os_error(24)));
        assert!(!accept_error_is_transient(&Error::from(
            ErrorKind::PermissionDenied
        )));
    }
}
