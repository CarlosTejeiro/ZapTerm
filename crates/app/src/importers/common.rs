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

/// Decode an exported file whatever its encoding: UTF-8 (with or without a
/// BOM), UTF-16 with a BOM (what Windows tools such as regedit can write),
/// or else Windows-1252 — MobaXterm writes its sessions in CP1252, so a name
/// like "Producción" is not valid UTF-8 and must not fail the whole import.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

/// One Windows-1252 byte as a char (0x80–0x9F differ from Latin-1).
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Read and decode an exported file (see [`decode_text`]).
pub fn read_text(path: &std::path::Path) -> std::io::Result<String> {
    Ok(decode_text(&std::fs::read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_every_export_encoding() {
        assert_eq!(decode_text("Producción €".as_bytes()), "Producción €");
        assert_eq!(
            decode_text(b"\xEF\xBB\xBFS:\"Hostname\"=x"),
            "S:\"Hostname\"=x"
        );
        // CP1252: ó = F3, € = 80 (invalid as UTF-8).
        assert_eq!(decode_text(b"Producci\xF3n \x80"), "Producción €");
        let utf16le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("Año".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        assert_eq!(decode_text(&utf16le), "Año");
        let utf16be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("Año".encode_utf16().flat_map(|u| u.to_be_bytes()))
            .collect();
        assert_eq!(decode_text(&utf16be), "Año");
    }
}
