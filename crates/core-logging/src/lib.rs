#![forbid(unsafe_code)]

//! `core-logging` — per-session loggers with size-based rotation.
//!
//! Each `SessionLogger` writes terminal output to a file in the app log
//! directory. When the file reaches 50 MB a new part file is opened
//! transparently (e.g., `session_20240101_120000.log` →
//! `session_20240101_120000.1.log`).
//!
//! Two modes:
//!  * [`LogFormat::Plain`] (default) — strips ANSI/VT escape sequences and
//!    OS-specific control codes so the file reads like a transcript.
//!  * [`LogFormat::Raw`]  — verbatim byte stream as the server sent it,
//!    including color codes, title sets, bracketed-paste markers, etc.
//!    Useful for replaying with `cat` into a real terminal.

pub mod error;
pub use error::Error;

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};

const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB per part
const BUF_CAP: usize = 64 * 1024; // 64 KB write buffer

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Strip ANSI/VT escape sequences and most C0 control chars; keep
    /// `\n`, `\r`, `\t`. This is the SecureCRT "log session" default.
    Plain,
    /// Pass bytes through verbatim. Useful for debugging or replaying.
    Raw,
}

impl LogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            LogFormat::Plain => "plain",
            LogFormat::Raw => "raw",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "raw" => LogFormat::Raw,
            _ => LogFormat::Plain,
        }
    }
}

pub struct SessionLogger {
    writer: BufWriter<File>,
    path: PathBuf,
    log_dir: PathBuf,
    /// `<name>_<timestamp>[-n]`: shared by every part of this log.
    stem: String,
    part: u32,
    /// Bytes in the current part (drives rotation).
    part_bytes: u64,
    /// Bytes in all parts together (what the log row records).
    bytes_written: u64,
    started_at: DateTime<Utc>,
    format: LogFormat,
    stripper: AnsiStripper,
}

impl SessionLogger {
    /// Open a new log file inside `log_dir`. Defaults to [`LogFormat::Plain`].
    pub fn open(log_dir: &Path, base_name: &str) -> Result<Self, Error> {
        Self::open_with_format(log_dir, base_name, LogFormat::Plain)
    }

    /// Open a new log file inside `log_dir` with an explicit format.
    ///
    /// `base_name` becomes the filename prefix (invalid filesystem chars are
    /// replaced with `_`). The actual filename is
    /// `<base_name>_YYYYMMDD_HHMMSS.log`.
    pub fn open_with_format(
        log_dir: &Path,
        base_name: &str,
        format: LogFormat,
    ) -> Result<Self, Error> {
        std::fs::create_dir_all(log_dir)?;
        let safe_name = sanitize(base_name);
        let (file, path, stem) = open_first(log_dir, &safe_name)?;
        Ok(Self {
            writer: BufWriter::with_capacity(BUF_CAP, file),
            path,
            log_dir: log_dir.to_owned(),
            stem,
            part: 0,
            part_bytes: 0,
            bytes_written: 0,
            started_at: Utc::now(),
            format,
            stripper: AnsiStripper::new(),
        })
    }

    /// Write terminal bytes. In [`LogFormat::Plain`] mode escape sequences
    /// are dropped before writing. Rotates to a new part file when ≥50 MB.
    pub fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.is_empty() {
            return Ok(());
        }
        let payload: &[u8] = match self.format {
            LogFormat::Raw => data,
            LogFormat::Plain => &self.stripper.strip(data),
        };
        if payload.is_empty() {
            return Ok(());
        }
        self.writer.write_all(payload)?;
        self.part_bytes += payload.len() as u64;
        self.bytes_written += payload.len() as u64;

        if self.part_bytes >= MAX_FILE_BYTES {
            self.writer.flush()?;
            let next = self.part + 1;
            let (new_file, new_path) = open_part(&self.log_dir, &self.stem, next)?;
            self.writer = BufWriter::with_capacity(BUF_CAP, new_file);
            self.path = new_path;
            self.part = next;
            self.part_bytes = 0;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes written so far, across every part.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Flush and consume the logger. Returns `(path of the last part, total
    /// bytes across all parts, started_at)`.
    pub fn close(mut self) -> Result<(PathBuf, u64, DateTime<Utc>), Error> {
        self.writer.flush()?;
        Ok((self.path, self.bytes_written, self.started_at))
    }
}

/// Create the first part, `<base>_<timestamp>.log`, as a NEW file: two logs
/// started with the same name in the same second (two tabs of one session)
/// used to append into one file, interleaving both. A taken name gets
/// `-2`, `-3`… Returns the file, its path and the stem the parts share.
fn open_first(dir: &Path, base: &str) -> Result<(File, PathBuf, String), Error> {
    let ts = Local::now().format("%Y%m%d_%H%M%S");
    for n in 1..1000u32 {
        let stem = if n == 1 {
            format!("{base}_{ts}")
        } else {
            format!("{base}_{ts}-{n}")
        };
        let path = dir.join(format!("{stem}.log"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path, stem)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free log file name").into())
}

/// Rotation part `part` (≥ 1) of a log: `<stem>.<part>.log`, next to the first.
fn open_part(dir: &Path, stem: &str, part: u32) -> Result<(File, PathBuf), Error> {
    let path = dir.join(format!("{stem}.{part}.log"));
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    Ok((file, path))
}

/// Byte-stream state machine that strips ANSI/VT escape sequences and most
/// C0 control characters, keeping the printable subset plus `\n`, `\r`, `\t`.
///
/// Handles sequences that span calls to [`Self::strip`] by parking the state
/// between invocations — important because a single OSC title set can
/// straddle several PTY chunks.
///
/// Covers, in order of frequency on a real shell:
///  * CSI:  ESC `[` … final-byte 0x40..=0x7e  (colors, cursor moves, modes)
///  * OSC:  ESC `]` … BEL (0x07) | ESC \\     (title, hyperlinks)
///  * SS2/SS3, DCS, PM, APC, single-byte ESC X — drop the sequence.
///  * Bracketed-paste markers (already CSI, so covered).
struct AnsiStripper {
    state: StripState,
}

#[derive(Clone, Copy)]
enum StripState {
    Normal,
    /// Just saw ESC (0x1b); waiting for the introducer.
    AfterEsc,
    /// Inside a CSI (ESC `[`); consume until a final byte 0x40..=0x7e.
    Csi,
    /// Inside an OSC (ESC `]`); consume until BEL or ESC \\.
    Osc,
    /// Inside an OSC and just saw ESC — next byte must be \\ to terminate.
    OscEsc,
}

impl AnsiStripper {
    fn new() -> Self {
        Self {
            state: StripState::Normal,
        }
    }

    fn strip(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &b in data {
            match self.state {
                StripState::Normal => {
                    if b == 0x1b {
                        self.state = StripState::AfterEsc;
                    } else if b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t' || b == 0x08 {
                        // Printable ASCII, UTF-8 continuation byte (≥0x80),
                        // or one of the whitespace controls we keep.
                        // Also keep backspace (0x08) so prompt edits survive.
                        out.push(b);
                    } else {
                        // BEL, NUL, etc — drop.
                    }
                }
                StripState::AfterEsc => match b {
                    b'[' => self.state = StripState::Csi,
                    b']' => self.state = StripState::Osc,
                    // ESC \\ terminator (string terminator) outside OSC — just drop.
                    b'\\' | b'P' | b'X' | b'^' | b'_' => {
                        // DCS / SOS / PM / APC start: same OSC-style termination.
                        self.state = StripState::Osc;
                    }
                    _ => {
                        // Single-byte ESC sequence (e.g. ESC = / >). Drop.
                        self.state = StripState::Normal;
                    }
                },
                StripState::Csi => {
                    if (0x40..=0x7e).contains(&b) {
                        self.state = StripState::Normal;
                    }
                    // else: parameter / intermediate byte, keep consuming.
                }
                StripState::Osc => match b {
                    0x07 => self.state = StripState::Normal, // BEL terminator
                    0x1b => self.state = StripState::OscEsc,
                    _ => { /* keep consuming */ }
                },
                StripState::OscEsc => {
                    // After ESC inside OSC, any byte ends the sequence.
                    self.state = StripState::Normal;
                }
            }
        }
        out
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_and_close() {
        let dir = std::env::temp_dir().join("zapterm_log_test");
        let _ = fs::remove_dir_all(&dir);
        let mut logger = SessionLogger::open(&dir, "test-session").unwrap();
        logger.write(b"hello world\n").unwrap();
        assert_eq!(logger.bytes_written(), 12);
        let (path, bytes, _) = logger.close().unwrap();
        assert!(path.exists());
        assert_eq!(bytes, 12);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two logs with the same name started in the same second get their own
    /// files (they used to append into one).
    #[test]
    fn same_name_same_second_gets_separate_files() {
        let dir = std::env::temp_dir().join(format!("zapterm-log-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = SessionLogger::open(&dir, "fw").unwrap();
        let mut b = SessionLogger::open(&dir, "fw").unwrap();
        assert_ne!(a.path(), b.path());
        a.write(b"from a\n").unwrap();
        b.write(b"from b\n").unwrap();
        let (pa, na, _) = a.close().unwrap();
        let (pb, _, _) = b.close().unwrap();
        assert_eq!(std::fs::read_to_string(&pa).unwrap(), "from a\n");
        assert_eq!(std::fs::read_to_string(&pb).unwrap(), "from b\n");
        assert_eq!(na, 7);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_name() {
        assert_eq!(sanitize("my session/test:1"), "my_session_test_1");
    }

    fn strip(input: &[u8]) -> String {
        let mut s = AnsiStripper::new();
        String::from_utf8(s.strip(input)).unwrap()
    }

    #[test]
    fn strips_csi_colors() {
        let raw = b"\x1b[01;32mcarlos@kobe\x1b[00m:\x1b[01;34m~\x1b[00m$ ";
        assert_eq!(strip(raw), "carlos@kobe:~$ ");
    }

    #[test]
    fn strips_osc_title() {
        let raw = b"\x1b]0;carlos@kobe: ~\x07$ ls";
        assert_eq!(strip(raw), "$ ls");
    }

    #[test]
    fn strips_bracketed_paste_modes() {
        // These are the ?2004h / ?2004l shown in the user's report.
        let raw = b"\x1b[?2004h$ hi\x1b[?2004l\n";
        assert_eq!(strip(raw), "$ hi\n");
    }

    #[test]
    fn handles_sequence_split_across_chunks() {
        let mut s = AnsiStripper::new();
        let mut out = s.strip(b"hello \x1b[01");
        out.extend(s.strip(b";32mworld\x1b[00m"));
        assert_eq!(String::from_utf8(out).unwrap(), "hello world");
    }

    #[test]
    fn keeps_newlines_and_utf8() {
        let raw = "hola múnd\n".as_bytes();
        assert_eq!(strip(raw), "hola múnd\n");
    }

    #[test]
    fn plain_mode_logger_writes_clean_text() {
        let dir = std::env::temp_dir().join("zapterm_log_plain_test");
        let _ = fs::remove_dir_all(&dir);
        let mut logger =
            SessionLogger::open_with_format(&dir, "plain-test", LogFormat::Plain).unwrap();
        logger
            .write(b"\x1b[?2004h\x1b[01;32mok\x1b[0m\r\n")
            .unwrap();
        let (path, _, _) = logger.close().unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "ok\r\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
