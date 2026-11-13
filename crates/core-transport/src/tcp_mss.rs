//! TCP Maximum Segment Size (MSS) querying — snapshot + live watcher.
//!
//! [`TcpMss`] is the per-direction byte count negotiated at TCP setup
//! (and revised by Path-MTU Discovery if it kicks in). [`MssWatcher`]
//! owns a duplicated socket handle so the app layer can poll periodically
//! AFTER russh has taken ownership of the original `TcpStream` — without
//! racing the kernel re-using the fd number.
//!
//! Platform support:
//!
//! * **Linux** — full bidirectional via `getsockopt(IPPROTO_TCP, TCP_INFO)`
//!   (`tcpi_snd_mss` ↑, `tcpi_rcv_mss` ↓).
//! * **macOS** — `getsockopt(IPPROTO_TCP, TCP_MAXSEG)` for the send side.
//!   Receive isn't exposed; we report `None`.
//! * **Windows** — `WSAIoctl(SIO_TCP_INFO)` returning `TCP_INFO_v0`. Only
//!   one MSS field is provided (the negotiated send side).

// libc::getsockopt and the Windows WSAIoctl are raw C calls — scoped allow
// keeps the rest of the crate's `deny(unsafe_code)` intact.
#![allow(unsafe_code)]

#[cfg(unix)]
use tokio::net::TcpStream;

/// Bidirectional MSS snapshot.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct TcpMss {
    /// Bytes per segment we will put on the wire when SENDING.
    pub send: Option<u32>,
    /// Bytes per segment the peer will put on the wire when sending TO us.
    pub recv: Option<u32>,
}

// ---------------------------------------------------------------------------
// One-shot query — convenience for the "take a snapshot at connect" path.
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn query(stream: &TcpStream) -> TcpMss {
    use std::os::fd::AsRawFd;
    query_by_fd_unix(stream.as_raw_fd())
}

#[cfg(windows)]
pub fn query(stream: &tokio::net::TcpStream) -> TcpMss {
    use std::os::windows::io::AsRawSocket;
    query_by_socket_win(stream.as_raw_socket() as windows_sys::Win32::Networking::WinSock::SOCKET)
}

#[cfg(not(any(unix, windows)))]
pub fn query(_stream: &tokio::net::TcpStream) -> TcpMss {
    TcpMss::default()
}

// ---------------------------------------------------------------------------
// Watcher — owns a duplicated socket handle so it can keep querying after
// russh has consumed the original `TcpStream`. Drop closes the duplicate.
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub struct MssWatcher {
    /// Our private dup of the original socket fd. Closed in `Drop`.
    fd: libc::c_int,
}

#[cfg(unix)]
impl MssWatcher {
    /// Duplicate the socket. Returns `None` if `dup()` fails — we don't
    /// poison the connect, just lose the ability to poll MSS over time.
    pub fn new(stream: &TcpStream) -> Option<Self> {
        use std::os::fd::AsRawFd;
        let orig = stream.as_raw_fd();
        // SAFETY: F_DUPFD_CLOEXEC on an open fd is always safe; the returned
        // fd is owned by us and freed by Drop. Close-on-exec (like every fd
        // Rust's std creates) keeps child processes — a script, xdg-open —
        // from inheriting the live SSH socket.
        let dup = unsafe { libc::fcntl(orig, libc::F_DUPFD_CLOEXEC, 0) };
        if dup < 0 {
            return None;
        }
        Some(MssWatcher { fd: dup })
    }

    pub fn query(&self) -> TcpMss {
        query_by_fd_unix(self.fd)
    }
}

#[cfg(unix)]
impl Drop for MssWatcher {
    fn drop(&mut self) {
        // SAFETY: fd is ours (returned by dup), valid until close.
        unsafe { libc::close(self.fd) };
    }
}

#[cfg(windows)]
pub struct MssWatcher {
    socket: windows_sys::Win32::Networking::WinSock::SOCKET,
}

#[cfg(windows)]
impl MssWatcher {
    /// Windows doesn't expose a cheap `dup()` for sockets — `WSADuplicateSocket`
    /// is a multi-step inter-process API designed for handoff to a child
    /// process, not for in-process sharing. For polling within the same
    /// process we keep the raw socket value and accept that if russh closes
    /// the connection and the OS reassigns the socket number, our next
    /// query may return `default()` (an EBADF-equivalent). That matches the
    /// "session ended" semantic the caller wants anyway.
    pub fn new(stream: &tokio::net::TcpStream) -> Option<Self> {
        use std::os::windows::io::AsRawSocket;
        Some(MssWatcher {
            socket: stream.as_raw_socket() as windows_sys::Win32::Networking::WinSock::SOCKET,
        })
    }

    pub fn query(&self) -> TcpMss {
        query_by_socket_win(self.socket)
    }
}

#[cfg(not(any(unix, windows)))]
pub struct MssWatcher;

#[cfg(not(any(unix, windows)))]
impl MssWatcher {
    pub fn new(_stream: &tokio::net::TcpStream) -> Option<Self> {
        None
    }
    pub fn query(&self) -> TcpMss {
        TcpMss::default()
    }
}

// ---------------------------------------------------------------------------
// Platform-specific getsockopt wrappers
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn query_by_fd_unix(fd: libc::c_int) -> TcpMss {
    let mut info: libc::tcp_info = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::tcp_info>() as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_INFO,
            &mut info as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if ret == 0 {
        TcpMss {
            send: Some(info.tcpi_snd_mss),
            recv: Some(info.tcpi_rcv_mss),
        }
    } else {
        TcpMss::default()
    }
}

#[cfg(target_os = "macos")]
fn query_by_fd_unix(fd: libc::c_int) -> TcpMss {
    let mut mss: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_MAXSEG,
            &mut mss as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if ret == 0 && mss > 0 {
        TcpMss {
            send: Some(mss as u32),
            recv: None,
        }
    } else {
        TcpMss::default()
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn query_by_fd_unix(_fd: libc::c_int) -> TcpMss {
    TcpMss::default()
}

#[cfg(windows)]
fn query_by_socket_win(socket: windows_sys::Win32::Networking::WinSock::SOCKET) -> TcpMss {
    use windows_sys::Win32::Networking::WinSock::{TCP_INFO_v0, WSAIoctl, SIO_TCP_INFO};

    let mut version: u32 = 0;
    let mut info: TCP_INFO_v0 = unsafe { std::mem::zeroed() };
    let mut bytes_returned: u32 = 0;
    let ret = unsafe {
        WSAIoctl(
            socket,
            SIO_TCP_INFO,
            &mut version as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
            &mut info as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TCP_INFO_v0>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
            None,
        )
    };
    if ret == 0 && info.Mss > 0 {
        // Windows only fills the send side; receive isn't surfaced.
        TcpMss {
            send: Some(info.Mss),
            recv: None,
        }
    } else {
        TcpMss::default()
    }
}
