//! Shared building blocks for third-party importers: the parse result type,
//! an empty native envelope, and a `SavedSession` builder so every importer
//! emits rows the same way (ssh protocol, agent/key/password auth, options
//! JSON for serial/key paths). Folders are emitted with file-local ids that
//! `apply_import` remaps.

use core_persistence::{Folder, SavedSession};

use crate::commands::transfer::ExportFile;

/// Result of an importer parse: the native envelope plus parser-level
/// warnings (the import pipeline appends its own on apply).
pub struct Parsed {
    pub file: ExportFile,
    pub warnings: Vec<String>,
}

/// An `ExportFile` with only folders and sessions populated — the shape
/// every third-party importer produces (no groups/snippets/rules).
pub fn envelope(folders: Vec<Folder>, sessions: Vec<SavedSession>) -> ExportFile {
    ExportFile {
        zapx_export: 1,
        app_version: String::new(),
        exported_at: String::new(),
        folders,
        sessions,
        broadcast_groups: vec![],
        snippets: vec![],
        highlight_rules: vec![],
        session_forwards: vec![],
        session_triggers: vec![],
    }
}

/// A folder row with a file-local id (remapped by `apply_import`).
pub fn folder(id: i64, parent_id: Option<i64>, name: &str, sort_order: i32) -> Folder {
    Folder {
        id,
        parent_id,
        name: name.to_owned(),
        sort_order,
    }
}

/// Auth flavour for [`session`]. `Key` stores the path in `options_json`.
pub enum Auth {
    Password,
    Agent,
    Key(String),
}

/// Build a `SavedSession` row with a file-local id. `protocol` is usually
/// "ssh"/"telnet"; `extra_options` lets serial sessions inject device/baud
/// (merged with the key path when both apply — callers pass one or the other
/// in practice).
#[allow(clippy::too_many_arguments)]
pub fn session(
    id: i64,
    folder_id: Option<i64>,
    name: &str,
    protocol: &str,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    auth: Auth,
    sort_order: i64,
) -> SavedSession {
    let (auth_method, options_json) = match auth {
        Auth::Password => (Some("password".to_owned()), "{}".to_owned()),
        Auth::Agent => (Some("agent".to_owned()), "{}".to_owned()),
        Auth::Key(path) => (
            Some("key".to_owned()),
            format!(
                r#"{{"key_path":{}}}"#,
                serde_json::to_string(&path).unwrap_or_else(|_| "\"\"".into())
            ),
        ),
    };
    // Telnet/serial don't use auth_method; keep it null for non-ssh.
    let auth_method = if protocol == "ssh" { auth_method } else { None };
    SavedSession {
        id,
        folder_id,
        name: name.to_owned(),
        protocol: protocol.to_owned(),
        host,
        port,
        username,
        credential_id: None,
        options_json,
        last_used_at: None,
        auth_method,
        via_session_id: None,
        login_script_json: None,
        sort_order,
    }
}

/// Default SSH/Telnet ports — used when the source omits the port.
pub fn default_port(protocol: &str) -> u16 {
    match protocol {
        "telnet" => 23,
        _ => 22,
    }
}
