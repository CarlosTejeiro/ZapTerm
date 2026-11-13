//! SFTP wrapper over an established SSH session.
//!
//! Opens an SFTP subsystem channel through an [`Arc<SshHandle>`] and exposes a
//! small, app-friendly API (entries, stat, mkdir, rm, rename, upload/download)
//! that returns serializable types. The russh-sftp `SftpSession` is hidden
//! behind [`SftpClient`] so the app crate doesn't depend on russh-sftp.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, OpenFlags};

use crate::error::Error;

/// Chunk size for streaming transfers. 64 KiB is a sweet spot between
/// per-request overhead (one SFTP packet per chunk) and the rate at which
/// progress events reach the UI. Anything bigger and progress bars look
/// jumpy on slow links; anything smaller and CPU spend on packet framing
/// dominates over the wire bytes.
const STREAM_CHUNK: usize = 64 * 1024;

/// User-supplied progress callback. Fired after every chunk with
/// `(bytes_done, total_or_none)` — total is `None` when the size couldn't
/// be determined upfront (e.g. streaming uploads from a pipe — not the case
/// today but kept open).
pub type ProgressFn = Arc<dyn Fn(u64, Option<u64>) + Send + Sync + 'static>;

/// Cooperative cancellation flag. The streaming transfer checks the flag
/// after each chunk and returns [`Error::Cancelled`] if set. The app crate
/// hands one per `transfer_id` so the frontend's cancel button works.
pub type CancelToken = Arc<AtomicBool>;

pub fn new_cancel_token() -> CancelToken {
    Arc::new(AtomicBool::new(false))
}

/// One entry in a remote directory listing (mirrored 1:1 to TS).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SftpEntry {
    pub name: String,
    /// `"dir" | "file" | "symlink" | "other"`.
    pub kind: String,
    /// File size in bytes (None for some kinds).
    pub size: Option<u64>,
    /// POSIX permission bits (mode & 0o7777), if reported.
    pub permissions: Option<u32>,
    /// Last-modified time as Unix seconds (None if not reported).
    pub mtime: Option<u64>,
}

fn kind_str(t: FileType) -> &'static str {
    if t.is_dir() {
        "dir"
    } else if t.is_file() {
        "file"
    } else if t.is_symlink() {
        "symlink"
    } else {
        "other"
    }
}

fn map_err<E: std::fmt::Display>(e: E) -> Error {
    Error::Sftp(e.to_string())
}

/// App-facing SFTP client. Cheap to clone via `Arc<SftpClient>`.
///
/// All methods take `&self` so multiple Tauri commands can issue concurrent
/// requests through the same SFTP subsystem.
pub struct SftpClient {
    session: SftpSession,
}

impl SftpClient {
    /// Open an SFTP subsystem channel through the given SSH session handle and
    /// wrap it as an SFTP client.
    pub async fn open(
        handle: &russh::client::Handle<crate::ssh::SshClientHandler>,
    ) -> Result<Self, Error> {
        let mut channel = handle.channel_open_session().await.map_err(Error::Ssh)?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(Error::Ssh)?;
        // Wait for the server's answer. A host without SFTP (many network
        // devices, or `Subsystem sftp` missing in sshd_config) refuses here;
        // without waiting, that only surfaced as the SFTP handshake timing
        // out 10 s later, with an error that didn't say why.
        let answer = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                match channel.wait().await {
                    Some(russh::ChannelMsg::Success) => return Ok(()),
                    Some(russh::ChannelMsg::Failure) => return Err(
                        "the server refused the SFTP subsystem (SFTP is not enabled on this host)",
                    ),
                    Some(russh::ChannelMsg::Eof | russh::ChannelMsg::Close) | None => {
                        return Err("the server closed the SFTP channel")
                    }
                    Some(_) => {}
                }
            }
        })
        .await;
        match answer {
            Ok(Ok(())) => {}
            Ok(Err(why)) => return Err(Error::Sftp(why.into())),
            Err(_) => {
                return Err(Error::Sftp(
                    "the server did not answer the SFTP request".into(),
                ))
            }
        }
        let stream = channel.into_stream();
        let session = SftpSession::new(stream).await.map_err(map_err)?;
        Ok(Self { session })
    }

    pub async fn list_dir(&self, path: &str) -> Result<Vec<SftpEntry>, Error> {
        let read_dir = self.session.read_dir(path).await.map_err(map_err)?;
        let mut entries = Vec::new();
        for entry in read_dir {
            let md = entry.metadata();
            entries.push(SftpEntry {
                name: entry.file_name(),
                kind: kind_str(entry.file_type()).to_string(),
                size: md.size,
                permissions: md.permissions.map(|p| p & 0o7777),
                mtime: md.mtime.map(u64::from),
            });
        }
        // Stable ordering: directories first, then files; case-insensitive name
        // (each name lowercased once, not on every comparison).
        entries.sort_by_cached_key(|e| (e.kind != "dir", e.name.to_lowercase()));
        Ok(entries)
    }

    pub async fn stat(&self, path: &str) -> Result<SftpEntry, Error> {
        let md = self.session.metadata(path).await.map_err(map_err)?;
        let name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        Ok(SftpEntry {
            name,
            kind: kind_str(md.file_type()).to_string(),
            size: md.size,
            permissions: md.permissions.map(|p| p & 0o7777),
            mtime: md.mtime.map(u64::from),
        })
    }

    /// Resolve `path` (which may be `"."` or relative) to an absolute path on the
    /// remote side. Useful to discover the user's `$HOME` at panel open time.
    pub async fn canonicalize(&self, path: &str) -> Result<String, Error> {
        self.session.canonicalize(path).await.map_err(map_err)
    }

    pub async fn create_dir(&self, path: &str) -> Result<(), Error> {
        self.session.create_dir(path).await.map_err(map_err)
    }

    pub async fn remove_dir(&self, path: &str) -> Result<(), Error> {
        self.session.remove_dir(path).await.map_err(map_err)
    }

    pub async fn remove_file(&self, path: &str) -> Result<(), Error> {
        self.session.remove_file(path).await.map_err(map_err)
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<(), Error> {
        self.session.rename(from, to).await.map_err(map_err)
    }

    /// Download a remote file to `local_path`. Returns the byte count.
    /// Small/medium files only — buffers in memory. Streaming + progress is a
    /// follow-up.
    pub async fn download(&self, remote_path: &str, local_path: &Path) -> Result<u64, Error> {
        let bytes = self.session.read(remote_path).await.map_err(map_err)?;
        let mut f = tokio::fs::File::create(local_path)
            .await
            .map_err(Error::Io)?;
        f.write_all(&bytes).await.map_err(Error::Io)?;
        f.flush().await.map_err(Error::Io)?;
        Ok(bytes.len() as u64)
    }

    /// Upload `local_path` to the remote `remote_path`, creating or truncating
    /// it. Returns the byte count.
    ///
    /// Opens with `CREATE | TRUNCATE` explicitly: russh-sftp's convenience
    /// `write()` opens with `WRITE` only, which the server answers with
    /// `NO_SUCH_FILE` for a file that doesn't exist yet — so this method could
    /// never upload a *new* file. The UI uses [`Self::upload_streaming`] and
    /// was unaffected; the integration test caught it.
    pub async fn upload(&self, local_path: &Path, remote_path: &str) -> Result<u64, Error> {
        let bytes = tokio::fs::read(local_path).await.map_err(Error::Io)?;
        let len = bytes.len() as u64;
        let mut remote = self
            .session
            .create(remote_path.to_string())
            .await
            .map_err(map_err)?;
        remote.write_all(&bytes).await.map_err(Error::Io)?;
        remote.flush().await.map_err(Error::Io)?;
        // Await the SFTP CLOSE so the write is acknowledged before we report
        // success (see `upload_streaming`).
        remote.shutdown().await.map_err(Error::Io)?;
        Ok(len)
    }

    /// Streaming download with progress + cooperative cancellation.
    ///
    /// Opens the remote file as an `AsyncRead`, then loops reading
    /// [`STREAM_CHUNK`]-sized buffers and writing them to `local_path`.
    /// `progress` is called after every chunk with `(bytes_done, total)`.
    /// `cancel` is checked after every chunk; setting it makes the function
    /// return [`Error::Cancelled`].
    ///
    /// The bytes go to a temporary file next to `local_path`, renamed over it
    /// only once complete: a cancelled or failed download leaves an existing
    /// file at `local_path` untouched (it used to be truncated at the start
    /// and then deleted), and never leaves a partial file under that name.
    pub async fn download_streaming(
        &self,
        remote_path: &str,
        local_path: &Path,
        cancel: CancelToken,
        progress: ProgressFn,
    ) -> Result<u64, Error> {
        let total = self
            .session
            .metadata(remote_path.to_string())
            .await
            .ok()
            .and_then(|m| m.size);

        let mut remote = self
            .session
            .open(remote_path.to_string())
            .await
            .map_err(map_err)?;
        let part = partial_path(local_path);
        let mut local = tokio::fs::File::create(&part).await.map_err(Error::Io)?;

        let mut done: u64 = 0;
        let mut buf = vec![0u8; STREAM_CHUNK];
        progress(0, total);
        let copied: Result<(), Error> = async {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                let n = remote.read(&mut buf).await.map_err(Error::Io)?;
                if n == 0 {
                    break;
                }
                local.write_all(&buf[..n]).await.map_err(Error::Io)?;
                done += n as u64;
                progress(done, total);
            }
            local.flush().await.map_err(Error::Io)?;
            local.sync_all().await.map_err(Error::Io)?;
            drop(local);
            // Replaces an existing file (on Windows too).
            tokio::fs::rename(&part, local_path)
                .await
                .map_err(Error::Io)
        }
        .await;
        if let Err(e) = copied {
            // Cancelled, or the read/write/link/rename failed part-way: drop
            // the partial copy; the target was never touched.
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }
        Ok(done)
    }

    /// Streaming upload with progress + cooperative cancellation.
    ///
    /// Reads `local_path` in `STREAM_CHUNK` blocks and writes each block to
    /// the remote file via `russh-sftp`'s AsyncWrite impl. On cancel, the
    /// remote file is closed mid-stream and left to the server's GC (we do
    /// not attempt to unlink because the user may want to resume).
    pub async fn upload_streaming(
        &self,
        local_path: &Path,
        remote_path: &str,
        cancel: CancelToken,
        progress: ProgressFn,
    ) -> Result<u64, Error> {
        let total = tokio::fs::metadata(local_path).await.ok().map(|m| m.len());

        let mut local = tokio::fs::File::open(local_path).await.map_err(Error::Io)?;
        // Truncate-create on the remote side so resuming starts from zero.
        let flags = OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE;
        let mut remote = self
            .session
            .open_with_flags(remote_path.to_string(), flags)
            .await
            .map_err(map_err)?;

        let mut done: u64 = 0;
        let mut buf = vec![0u8; STREAM_CHUNK];
        progress(0, total);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let n = local.read(&mut buf).await.map_err(Error::Io)?;
            if n == 0 {
                break;
            }
            remote.write_all(&buf[..n]).await.map_err(Error::Io)?;
            done += n as u64;
            progress(done, total);
        }
        remote.flush().await.map_err(Error::Io)?;
        // Explicitly close the remote handle so the final writes are acked by
        // the server before we report success — relying on drop would close it
        // without awaiting the SFTP CLOSE, which can race a subsequent stat.
        remote.shutdown().await.map_err(Error::Io)?;
        Ok(done)
    }
}

/// Hidden temporary name next to `target` for a download in progress
/// (`.name.zapterm-part-<pid>-<n>`), so the rename stays on one filesystem.
fn partial_path(target: &Path) -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    target.with_file_name(format!(".{name}.zapterm-part-{}-{n}", std::process::id()))
}

/// Convenient handle stored on `ActiveSession` for lazy SFTP init.
pub type SftpSlot = Arc<tokio::sync::Mutex<Option<Arc<SftpClient>>>>;

/// Build an empty lazy slot — call this when constructing an SSH `ActiveSession`.
pub fn empty_slot() -> SftpSlot {
    Arc::new(tokio::sync::Mutex::new(None))
}
