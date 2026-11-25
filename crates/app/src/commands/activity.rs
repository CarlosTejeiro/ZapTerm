//! Automation activity: a plain-text record, on this computer, of everything
//! a script, a notebook or an AI assistant (MCP) sent — or tried to send — to
//! a device through ZapTerm.
//!
//! One file per month in `<logs>/automation/`, `activity-YYYY-MM.log`, one
//! line per event with tab-separated fields (see [`HEADER`]), so it reads in
//! any editor and opens as a table in a spreadsheet. Only the command line is
//! kept, never what the device printed (session logging records that) and
//! never a password.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

/// First lines of every activity file.
const HEADER: &str =
    "# ZapTerm automation activity. One line per event, fields separated by tabs:\n\
# time\tsource\tclient\tdevice\taddress\taction\tresult\tseconds\tdetail\n";

/// One writer at a time, so two events never interleave in the file.
static WRITE: Mutex<()> = Mutex::new(());

/// What the frontend reports; the time is stamped here.
#[derive(Debug, Clone, Deserialize)]
pub struct NewActivity {
    /// `script`, `notebook` or `mcp`.
    pub source: String,
    /// Script file name, `Jupyter`, or the MCP client's name.
    #[serde(default)]
    pub client: String,
    /// Tab name; empty for connection events.
    #[serde(default)]
    pub device: String,
    /// `host:port` of the device, when known.
    #[serde(default)]
    pub address: String,
    /// `run`, `send`, `secret`, `open`, `close`, `connect`, `disconnect`,
    /// `start`, `finish`.
    pub action: String,
    /// `sent`, `approved` (asked, allowed, sent), `refused` (read-only),
    /// `declined` (asked, not allowed), `timeout`, `error`, `ok`.
    pub result: String,
    /// How long it took, for commands.
    #[serde(default)]
    pub seconds: Option<f64>,
    /// The command line, or what happened.
    #[serde(default)]
    pub detail: String,
}

/// One recorded event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActivityEntry {
    /// Local time with its UTC offset (RFC 3339, to the second).
    pub time: String,
    pub source: String,
    pub client: String,
    pub device: String,
    pub address: String,
    pub action: String,
    pub result: String,
    pub seconds: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct ActivityLog {
    /// Oldest first.
    pub entries: Vec<ActivityEntry>,
    /// Folder holding the files.
    pub dir: String,
    /// This month's file (it may not exist yet).
    pub current_file: String,
    /// Monthly files that exist but were not read (older than asked for).
    pub older_files: usize,
}

/// A field as written in the file: tabs and line breaks can't appear raw.
fn escape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    for c in field.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn format_line(e: &ActivityEntry) -> String {
    let seconds = e.seconds.map(|s| format!("{s:.1}")).unwrap_or_default();
    [
        e.time.as_str(),
        &escape(&e.source),
        &escape(&e.client),
        &escape(&e.device),
        &escape(&e.address),
        &escape(&e.action),
        &escape(&e.result),
        &seconds,
        &escape(&e.detail),
    ]
    .join("\t")
}

/// A line of the file back into an entry; `None` for comments, blank lines
/// and anything that isn't one of ours.
fn parse_line(line: &str) -> Option<ActivityEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let f: Vec<&str> = line.splitn(9, '\t').collect();
    if f.len() != 9 || DateTime::parse_from_rfc3339(f[0]).is_err() {
        return None;
    }
    Some(ActivityEntry {
        time: f[0].to_owned(),
        source: unescape(f[1]),
        client: unescape(f[2]),
        device: unescape(f[3]),
        address: unescape(f[4]),
        action: unescape(f[5]),
        result: unescape(f[6]),
        seconds: f[7].parse().ok(),
        detail: unescape(f[8]),
    })
}

fn activity_dir(log_dir: &Path) -> PathBuf {
    log_dir.join("automation")
}

fn month_file(dir: &Path, at: &DateTime<Local>) -> PathBuf {
    dir.join(format!("activity-{:04}-{:02}.log", at.year(), at.month()))
}

/// The monthly files in `dir`, oldest first (their names sort by date).
fn month_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("activity-") && n.ends_with(".log"))
        })
        .collect();
    files.sort();
    files
}

/// Append `new` to the month file of `at` in `dir`, creating both as needed.
fn append(dir: &Path, new: NewActivity, at: DateTime<Local>) -> std::io::Result<ActivityEntry> {
    let entry = ActivityEntry {
        time: at.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        source: new.source,
        client: new.client,
        device: new.device,
        address: new.address,
        action: new.action,
        result: new.result,
        seconds: new.seconds.filter(|s| s.is_finite() && *s >= 0.0),
        detail: new.detail,
    };
    let _guard = WRITE.lock().unwrap();
    std::fs::create_dir_all(dir)?;
    let path = month_file(dir, &at);
    let is_new = !path.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    if is_new {
        file.write_all(HEADER.as_bytes())?;
    }
    file.write_all(format!("{}\n", format_line(&entry)).as_bytes())?;
    Ok(entry)
}

/// The entries of the newest `months` monthly files, oldest first, and how
/// many older files were left out.
fn read(dir: &Path, months: usize) -> (Vec<ActivityEntry>, usize) {
    let files = month_files(dir);
    let skip = files.len().saturating_sub(months.max(1));
    let mut entries = Vec::new();
    for path in &files[skip..] {
        if let Ok(text) = std::fs::read_to_string(path) {
            entries.extend(text.lines().filter_map(parse_line));
        }
    }
    (entries, skip)
}

/// Record one event. Returns it as stored, with its time.
#[tauri::command]
pub async fn activity_record(
    state: State<'_, AppState>,
    entry: NewActivity,
) -> Result<ActivityEntry, AppError> {
    append(&activity_dir(&state.log_dir), entry, Local::now())
        .map_err(|e| AppError::Internal(format!("could not write the activity file: {e}")))
}

/// The recorded activity of the newest `months` months (default 2).
#[tauri::command]
pub async fn activity_read(
    state: State<'_, AppState>,
    months: Option<usize>,
) -> Result<ActivityLog, AppError> {
    let dir = activity_dir(&state.log_dir);
    let (entries, older_files) = read(&dir, months.unwrap_or(2));
    Ok(ActivityLog {
        entries,
        current_file: month_file(&dir, &Local::now())
            .to_string_lossy()
            .into_owned(),
        dir: dir.to_string_lossy().into_owned(),
        older_files,
    })
}

/// Open the activity folder, or this month's file, with the system's own
/// application. Best-effort, like `open_logs_dir`.
#[tauri::command]
pub async fn activity_open(state: State<'_, AppState>, file: bool) -> Result<String, AppError> {
    let dir = activity_dir(&state.log_dir);
    let _ = std::fs::create_dir_all(&dir);
    let current = month_file(&dir, &Local::now());
    let target = if file && current.is_file() {
        current
    } else {
        dir
    };
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(&target).spawn() {
        tracing::warn!("activity_open spawn failed: {e}");
    }
    Ok(target.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn new(action: &str, result: &str, detail: &str) -> NewActivity {
        NewActivity {
            source: "mcp".into(),
            client: "Claude Desktop".into(),
            device: "FWF-40F".into(),
            address: "192.168.20.254:22".into(),
            action: action.into(),
            result: result.into(),
            seconds: Some(0.42),
            detail: detail.into(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zapterm-activity-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_line_round_trips_with_tabs_and_line_breaks_in_the_command() {
        let entry = ActivityEntry {
            time: "2026-11-24T13:05:12+01:00".into(),
            source: "notebook".into(),
            client: "Jupyter".into(),
            device: "core\tsw".into(),
            address: "".into(),
            action: "run".into(),
            result: "sent".into(),
            seconds: Some(1.5),
            detail: "show run | include a\\b\tc\nd".into(),
        };
        let line = format_line(&entry);
        assert_eq!(line.matches('\t').count(), 8, "{line}");
        assert!(!line.contains('\n'));
        assert_eq!(parse_line(&line), Some(entry));
    }

    #[test]
    fn comments_blank_and_foreign_lines_are_skipped() {
        assert_eq!(parse_line("# time\tsource"), None);
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("not\tan\tactivity\tline"), None);
        assert_eq!(parse_line("yesterday\ta\tb\tc\td\te\tf\t\tg"), None);
    }

    #[test]
    fn events_go_to_their_month_file_with_a_header_and_read_back_in_order() {
        let dir = temp_dir("months");
        let oct = Local.with_ymd_and_hms(2026, 10, 31, 23, 59, 0).unwrap();
        let nov = Local.with_ymd_and_hms(2026, 11, 1, 0, 1, 0).unwrap();
        append(&dir, new("run", "sent", "get system status"), oct).unwrap();
        append(&dir, new("run", "refused", "config system global"), nov).unwrap();
        append(&dir, new("run", "sent", "show | grep x"), nov).unwrap();

        let files = month_files(&dir);
        assert_eq!(files.len(), 2);
        let text = std::fs::read_to_string(&files[1]).unwrap();
        assert!(text.starts_with("# ZapTerm automation activity"));
        assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 2);

        let (all, older) = read(&dir, 6);
        assert_eq!(older, 0);
        let details: Vec<&str> = all.iter().map(|e| e.detail.as_str()).collect();
        assert_eq!(
            details,
            ["get system status", "config system global", "show | grep x"]
        );
        assert_eq!(all[0].seconds, Some(0.4));

        let (latest, older) = read(&dir, 1);
        assert_eq!((latest.len(), older), (2, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_reads_as_empty() {
        let (entries, older) = read(&temp_dir("missing"), 2);
        assert!(entries.is_empty());
        assert_eq!(older, 0);
    }
}
