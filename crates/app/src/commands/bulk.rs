//! Add many devices to the inventory at once (File → Add devices in bulk…).
//!
//! The frontend parses a CSV file or a pasted list into [`BulkRow`]s, with
//! the dialog's defaults already applied. This module checks every row
//! against the database and, unless it is a preview, creates the sessions:
//!
//! * a row whose (name, protocol, host, port) already exists is skipped,
//!   exactly like the session import does, so adding the same list twice
//!   adds nothing;
//! * folders are given as a path (`"DC/Madrid"`) and created when missing;
//! * the credential is a reusable vault entry and the jump host a saved SSH
//!   session, both by name — the password itself never travels;
//! * nothing is created when any row has an error, so a typo doesn't leave
//!   half a list behind.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use core_persistence::{Database, Folder};

use crate::error::AppError;
use crate::state::AppState;

/// One device, as the dialog sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkRow {
    pub name: String,
    /// `"ssh"` or `"telnet"`.
    pub protocol: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// Folder path, `/`-separated; empty or absent = top level.
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Name of a reusable vault entry (SSH only).
    #[serde(default)]
    pub credential: Option<String>,
    /// Name of a saved SSH session to go through (SSH only).
    #[serde(default)]
    pub jump_host: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RowStatus {
    /// Will be (or was) created.
    New,
    /// Already in the inventory — skipped.
    Exists,
    /// Can't be created as it is; nothing is created while any row has one.
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowResult {
    pub status: RowStatus,
    /// Why it's an error or skipped, or a note on a new row.
    pub message: Option<String>,
    /// Folder path segments this row needs that don't exist yet.
    pub new_folders: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult {
    pub rows: Vec<RowResult>,
    pub added: usize,
    pub skipped: usize,
    pub errors: usize,
    pub folders_added: usize,
    /// True when sessions were created (not a preview, and no row had an error).
    pub applied: bool,
}

/// Check (and, when `apply`, create) the rows.
#[tauri::command]
pub async fn bulk_add_sessions(
    state: State<'_, AppState>,
    rows: Vec<BulkRow>,
    apply: bool,
) -> Result<BulkResult, AppError> {
    bulk_add(&state.db, &rows, apply)
}

/// Read a device list file as text (UTF-8, UTF-16 with a BOM, or Windows-1252,
/// as spreadsheet programs save CSV files).
#[tauri::command]
pub async fn read_device_list_file(path: String) -> Result<String, AppError> {
    crate::importers::common::read_text(std::path::Path::new(&path))
        .map_err(|e| AppError::Internal(format!("{path}: {e}")))
}

fn internal(e: core_persistence::Error) -> AppError {
    AppError::Internal(e.to_string())
}

/// Folder path → its segments, trimmed, without empty ones.
fn path_segments(path: Option<&str>) -> Vec<String> {
    path.unwrap_or("")
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The id of the folder at `segments` if it exists, else how many leading
/// segments do exist (the rest would be created).
fn find_folder(folders: &[Folder], segments: &[String]) -> Result<Option<i64>, usize> {
    let mut parent: Option<i64> = None;
    for (i, seg) in segments.iter().enumerate() {
        match folders
            .iter()
            .find(|f| f.parent_id == parent && f.name == *seg)
        {
            Some(f) => parent = Some(f.id),
            None => return Err(i),
        }
    }
    Ok(parent)
}

fn options_json(row: &BulkRow) -> String {
    let mut opts = serde_json::Map::new();
    let tags: Vec<&str> = row
        .tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if !tags.is_empty() {
        opts.insert("tags".into(), serde_json::json!(tags));
    }
    if let Some(n) = row
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        opts.insert("notes".into(), serde_json::json!(n));
    }
    serde_json::Value::Object(opts).to_string()
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

pub fn bulk_add(db: &Database, rows: &[BulkRow], apply: bool) -> Result<BulkResult, AppError> {
    let sessions = db.list_sessions().map_err(internal)?;
    let folders = db.list_folders().map_err(internal)?;
    let vault = db.list_vault_entries().map_err(internal)?;

    let mut result = BulkResult::default();
    // Resolved per row, for the create pass: (credential id, via id).
    let mut resolved: Vec<(Option<i64>, Option<i64>)> = Vec::with_capacity(rows.len());
    let mut seen: HashMap<(String, String, String, u16), usize> = HashMap::new();

    for (i, row) in rows.iter().enumerate() {
        let name = row.name.trim();
        let host = row.host.trim();
        let protocol = row.protocol.trim().to_ascii_lowercase();
        let mut notes: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        let mut credential_id = None;
        let mut via_id = None;

        if name.is_empty() {
            error = Some("no name".into());
        } else if host.is_empty() {
            error = Some("no host".into());
        } else if host.chars().any(char::is_whitespace) {
            error = Some(format!("“{host}” isn't a host name or address"));
        } else if protocol != "ssh" && protocol != "telnet" {
            error = Some(format!(
                "protocol “{}” — use ssh or telnet",
                row.protocol.trim()
            ));
        } else if row.port == 0 {
            error = Some("port must be 1–65535".into());
        }

        if error.is_none() {
            let cred = non_empty(&row.credential);
            let jump = non_empty(&row.jump_host);
            if protocol == "telnet" {
                if cred.is_some() || jump.is_some() {
                    notes.push("Telnet: credential and jump host don't apply".into());
                }
            } else {
                if let Some(c) = cred {
                    match vault.iter().find(|v| v.name == c) {
                        Some(v) => credential_id = Some(v.id),
                        None => error = Some(format!("no vault credential called “{c}”")),
                    }
                }
                if let (None, Some(j)) = (&error, jump) {
                    let matches: Vec<_> = sessions.iter().filter(|s| s.name == j).collect();
                    match matches.as_slice() {
                        [] => {
                            error = Some(format!("no saved session called “{j}” for the jump host"))
                        }
                        [s] if s.protocol != "ssh" => {
                            error = Some(format!("jump host “{j}” isn't an SSH session"))
                        }
                        [s] => via_id = Some(s.id),
                        _ => error = Some(format!("several saved sessions are called “{j}”")),
                    }
                }
            }
        }

        let key = (name.to_owned(), protocol.clone(), host.to_owned(), row.port);
        let status = if let Some(e) = error {
            notes.insert(0, e);
            RowStatus::Error
        } else if let Some(first) = seen.get(&key) {
            notes.insert(0, format!("same device as row {}", first + 1));
            RowStatus::Error
        } else if sessions.iter().any(|s| {
            s.name == name
                && s.protocol == protocol
                && s.host.as_deref() == Some(host)
                && s.port == Some(row.port)
        }) {
            notes.insert(
                0,
                "already in the inventory — skipped, left as it is".into(),
            );
            RowStatus::Exists
        } else {
            RowStatus::New
        };
        seen.entry(key).or_insert(i);

        let segments = path_segments(row.folder.as_deref());
        let new_folders = match find_folder(&folders, &segments) {
            Ok(_) => Vec::new(),
            Err(from) => segments[from..].to_vec(),
        };
        match status {
            RowStatus::New => result.added += 1,
            RowStatus::Exists => result.skipped += 1,
            RowStatus::Error => result.errors += 1,
        }
        result.rows.push(RowResult {
            status,
            message: (!notes.is_empty()).then(|| notes.join(" · ")),
            new_folders,
        });
        resolved.push((credential_id, via_id));
    }

    if !apply || result.errors > 0 {
        return Ok(result);
    }

    // Create folders as needed (cached by path), then the sessions.
    let mut folders = folders;
    let mut folder_ids: HashMap<Vec<String>, Option<i64>> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if result.rows[i].status != RowStatus::New {
            continue;
        }
        let segments = path_segments(row.folder.as_deref());
        let folder_id = match folder_ids.get(&segments) {
            Some(id) => *id,
            None => {
                let mut parent: Option<i64> = None;
                for seg in &segments {
                    let existing = folders
                        .iter()
                        .find(|f| f.parent_id == parent && f.name == *seg)
                        .map(|f| f.id);
                    parent = Some(match existing {
                        Some(id) => id,
                        None => {
                            let id = db.create_folder(seg, parent).map_err(internal)?;
                            folders.push(Folder {
                                id,
                                parent_id: parent,
                                name: seg.clone(),
                                sort_order: 0,
                            });
                            result.folders_added += 1;
                            id
                        }
                    });
                }
                folder_ids.insert(segments.clone(), parent);
                parent
            }
        };
        let protocol = row.protocol.trim().to_ascii_lowercase();
        let ssh = protocol == "ssh";
        let (credential_id, via_id) = resolved[i];
        db.create_session_full(
            folder_id,
            row.name.trim(),
            &protocol,
            Some(row.host.trim()),
            Some(row.port),
            if ssh { non_empty(&row.username) } else { None },
            credential_id,
            &options_json(row),
            // Password auth: the vault entry's secret, or a prompt on connect.
            if ssh { Some("password") } else { None },
            via_id,
        )
        .map_err(internal)?;
    }
    result.applied = true;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("zapterm-bulk-test-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (Database::open(&path).expect("open temp db"), path)
    }

    fn row(name: &str, host: &str) -> BulkRow {
        BulkRow {
            name: name.into(),
            protocol: "ssh".into(),
            host: host.into(),
            port: 22,
            username: None,
            folder: None,
            tags: vec![],
            notes: None,
            credential: None,
            jump_host: None,
        }
    }

    #[test]
    fn preview_creates_nothing_and_apply_creates_folders_and_sessions() {
        let (db, path) = temp_db("apply");
        let mut a = row("sw-01", "10.0.0.1");
        a.folder = Some(" DC / Madrid ".into());
        a.tags = vec!["core".into(), " ".into(), "mad".into()];
        a.notes = Some("rack 4".into());
        a.username = Some("netops".into());
        let mut b = row("sw-02", "10.0.0.2");
        b.folder = Some("DC/Madrid".into());
        b.protocol = "Telnet".into();
        b.port = 23;
        b.username = Some("ignored".into());

        let preview = bulk_add(&db, &[a.clone(), b.clone()], false).unwrap();
        assert!(!preview.applied);
        assert_eq!(preview.added, 2);
        assert_eq!(preview.rows[0].new_folders, vec!["DC", "Madrid"]);
        assert!(db.list_sessions().unwrap().is_empty());
        assert!(db.list_folders().unwrap().is_empty());

        let done = bulk_add(&db, &[a.clone(), b.clone()], true).unwrap();
        assert!(done.applied);
        assert_eq!((done.added, done.folders_added), (2, 2));
        let folders = db.list_folders().unwrap();
        let madrid = folders.iter().find(|f| f.name == "Madrid").unwrap();
        assert!(madrid.parent_id.is_some());
        let sessions = db.list_sessions().unwrap();
        let s1 = sessions.iter().find(|s| s.name == "sw-01").unwrap();
        assert_eq!(s1.folder_id, Some(madrid.id));
        assert_eq!(s1.username.as_deref(), Some("netops"));
        assert_eq!(s1.auth_method.as_deref(), Some("password"));
        let opts: serde_json::Value = serde_json::from_str(&s1.options_json).unwrap();
        assert_eq!(opts["tags"], serde_json::json!(["core", "mad"]));
        assert_eq!(opts["notes"], "rack 4");
        let s2 = sessions.iter().find(|s| s.name == "sw-02").unwrap();
        assert_eq!(s2.protocol, "telnet");
        assert_eq!(s2.username, None);

        // The same list again adds nothing.
        let again = bulk_add(&db, &[a, b], true).unwrap();
        assert_eq!((again.added, again.skipped), (0, 2));
        assert_eq!(again.rows[0].status, RowStatus::Exists);
        assert_eq!(db.list_sessions().unwrap().len(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolves_vault_credential_and_jump_host_by_name() {
        let (db, path) = temp_db("refs");
        let cred = db
            .create_vault_entry("Lab admin", Some("admin"), "k1")
            .unwrap();
        let bastion = db
            .create_session(
                None,
                "bastion",
                "ssh",
                Some("192.0.2.1"),
                Some(22),
                None,
                None,
                "{}",
            )
            .unwrap();
        db.create_session(
            None,
            "console",
            "telnet",
            Some("192.0.2.2"),
            Some(23),
            None,
            None,
            "{}",
        )
        .unwrap();
        let mut a = row("r1", "10.1.0.1");
        a.credential = Some("Lab admin".into());
        a.jump_host = Some("bastion".into());
        let done = bulk_add(&db, &[a], true).unwrap();
        assert!(done.applied);
        let s = db
            .list_sessions()
            .unwrap()
            .into_iter()
            .find(|s| s.name == "r1")
            .unwrap();
        assert_eq!(s.credential_id, Some(cred));
        assert_eq!(s.via_session_id, Some(bastion));

        let mut bad_cred = row("r2", "10.1.0.2");
        bad_cred.credential = Some("Nope".into());
        let mut telnet_jump = row("r3", "10.1.0.3");
        telnet_jump.jump_host = Some("console".into());
        let mut missing_jump = row("r4", "10.1.0.4");
        missing_jump.jump_host = Some("ghost".into());
        let r = bulk_add(&db, &[bad_cred, telnet_jump, missing_jump], false).unwrap();
        assert_eq!(r.errors, 3);
        assert!(r.rows[0]
            .message
            .as_deref()
            .unwrap()
            .contains("no vault credential"));
        assert!(r.rows[1]
            .message
            .as_deref()
            .unwrap()
            .contains("isn't an SSH session"));
        assert!(r.rows[2]
            .message
            .as_deref()
            .unwrap()
            .contains("no saved session"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn any_error_blocks_the_whole_list() {
        let (db, path) = temp_db("errors");
        let good = row("ok", "10.2.0.1");
        let mut no_host = row("bad", "");
        no_host.host = "  ".into();
        let mut spaces = row("bad2", "10.2.0 .2");
        spaces.port = 22;
        let mut proto = row("bad3", "10.2.0.3");
        proto.protocol = "rdp".into();
        let dup = row("ok", "10.2.0.1");
        let r = bulk_add(&db, &[good, no_host, spaces, proto, dup], true).unwrap();
        assert!(!r.applied);
        assert_eq!((r.added, r.errors), (1, 4));
        assert_eq!(r.rows[1].message.as_deref(), Some("no host"));
        assert!(r.rows[2]
            .message
            .as_deref()
            .unwrap()
            .contains("isn't a host"));
        assert!(r.rows[3]
            .message
            .as_deref()
            .unwrap()
            .contains("use ssh or telnet"));
        assert_eq!(r.rows[4].message.as_deref(), Some("same device as row 1"));
        assert!(db.list_sessions().unwrap().is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn telnet_ignores_credential_and_jump_with_a_note() {
        let (db, path) = temp_db("telnet");
        let mut t = row("t1", "10.3.0.1");
        t.protocol = "telnet".into();
        t.port = 23;
        t.credential = Some("whatever".into());
        let r = bulk_add(&db, &[t], true).unwrap();
        assert!(r.applied);
        assert_eq!(r.rows[0].status, RowStatus::New);
        assert!(r.rows[0].message.as_deref().unwrap().contains("Telnet"));
        let s = &db.list_sessions().unwrap()[0];
        assert_eq!((s.credential_id, s.auth_method.as_deref()), (None, None));
        let _ = std::fs::remove_file(path);
    }
}
