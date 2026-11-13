//! SecureCRT → native [`ExportFile`].
//!
//! SecureCRT stores each session as a VanDyke-INI file under its
//! `Config/Sessions/` tree; subdirectories are the session folders. We walk
//! that directory: every `*.ini` is a session, its relative directory path
//! becomes the ZapTerm folder hierarchy.
//!
//! VanDyke INI lines are typed: `S:"Key"=str`, `D:"Key"=hexdword`,
//! `B:"Key"=00000001`. Keys we read:
//! - `S:"Protocol Name"` → SSH2/SSH1 → ssh, Telnet → telnet, Serial → serial
//! - `S:"Hostname"`, `S:"Username"`
//! - `D:"… Port"` (any key ending in "Port") → port
//! - `S:"Identity Filename V2"` / `S:"Identity Filename"` → SSH key path
//! - `S:"Serial Port"` + `D:"Baud Rate"` → serial device/baud
//!
//! `__FolderData__.ini` and similar non-session files are ignored.

use std::collections::HashMap;
use std::path::Path;

use core_persistence::Folder;

use crate::importers::common::{self, Auth, Parsed};

/// Walk a SecureCRT `Sessions` directory into the native envelope.
pub fn parse_dir(root: &Path) -> std::io::Result<Parsed> {
    let mut warnings = Vec::new();
    let mut folders: Vec<Folder> = Vec::new();
    let mut folder_ids: HashMap<String, i64> = HashMap::new();
    let mut sessions = Vec::new();
    let mut next_folder_id: i64 = 0;
    let mut next_session_id: i64 = 0;

    // Depth-first walk so parent folders are created before children.
    let mut stack: Vec<(std::path::PathBuf, Option<i64>)> = vec![(root.to_path_buf(), None)];
    while let Some((dir, parent_id)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("ini") {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem.is_empty() || stem.starts_with("__") {
                    continue; // folder metadata, not a session
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        if let Some(row) = decode_session(
                            stem,
                            &content,
                            parent_id,
                            &mut next_session_id,
                            &mut warnings,
                        ) {
                            sessions.push(row);
                        }
                    }
                    Err(e) => warnings.push(format!("{}: {e}", path.display())),
                }
            }
        }
        // Register subfolders and queue them.
        for sub in subdirs {
            let Some(name) = sub.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let key = format!(
                "{}/{name}",
                parent_id.map(|p| p.to_string()).unwrap_or_default()
            );
            let id = *folder_ids.entry(key).or_insert_with(|| {
                next_folder_id += 1;
                folders.push(common::folder(
                    next_folder_id,
                    parent_id,
                    name,
                    folders.len() as i32,
                ));
                next_folder_id
            });
            stack.push((sub, Some(id)));
        }
    }

    Ok(Parsed {
        file: common::envelope(folders, sessions),
        warnings,
    })
}

fn decode_session(
    name: &str,
    content: &str,
    folder_id: Option<i64>,
    next_id: &mut i64,
    warnings: &mut Vec<String>,
) -> Option<core_persistence::SavedSession> {
    let mut strings: HashMap<String, String> = HashMap::new();
    let mut dwords: HashMap<String, u32> = HashMap::new();

    for raw in content.lines() {
        let line = raw.trim();
        // <type>:"<key>"=<value>
        let Some((typ, rest)) = line.split_once(':') else {
            continue;
        };
        let Some((key_q, value)) = rest.split_once('=') else {
            continue;
        };
        let key = key_q.trim().trim_matches('"').to_owned();
        match typ.trim() {
            "S" => {
                strings.insert(key, value.trim().to_owned());
            }
            "D" => {
                if let Ok(n) = u32::from_str_radix(value.trim(), 16) {
                    dwords.insert(key, n);
                }
            }
            _ => {} // B (bool) and others not needed
        }
    }

    let proto_name = strings
        .get("Protocol Name")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let protocol = if proto_name.starts_with("ssh") {
        "ssh"
    } else if proto_name == "telnet" {
        "telnet"
    } else if proto_name == "serial" {
        "serial"
    } else if proto_name.is_empty() {
        // No protocol line — assume SSH if there's a hostname, else skip.
        if strings.contains_key("Hostname") {
            "ssh"
        } else {
            warnings.push(format!("«{name}»: sin protocolo ni host — omitida"));
            return None;
        }
    } else {
        warnings.push(format!(
            "«{name}»: protocolo «{proto_name}» no soportado — omitida"
        ));
        return None;
    };

    // Port: any D-key ending in "Port" (e.g. "[SSH2] Port").
    let port = dwords
        .iter()
        .find(|(k, _)| k.ends_with("Port"))
        .and_then(|(_, &v)| u16::try_from(v).ok());

    if protocol == "serial" {
        let device = strings
            .get("Serial Port")
            .or_else(|| strings.get("Port"))
            .cloned()
            .unwrap_or_default();
        if device.is_empty() {
            warnings.push(format!("«{name}»: serial sin puerto — omitida"));
            return None;
        }
        *next_id += 1;
        let mut row = common::session(
            *next_id,
            folder_id,
            name,
            "serial",
            None,
            None,
            None,
            Auth::Password,
            *next_id - 1,
        );
        let baud = dwords.get("Baud Rate").copied().unwrap_or(9600);
        row.options_json = format!(
            r#"{{"device":{},"baud_rate":{baud}}}"#,
            serde_json::to_string(&device).unwrap_or_else(|_| "\"\"".into())
        );
        return Some(row);
    }

    let host = strings.get("Hostname").cloned().filter(|h| !h.is_empty());
    let Some(host) = host else {
        warnings.push(format!("«{name}»: sin Hostname — omitida"));
        return None;
    };
    let username = strings.get("Username").cloned().filter(|u| !u.is_empty());

    let auth = if protocol == "ssh" {
        match strings
            .get("Identity Filename V2")
            .or_else(|| strings.get("Identity Filename"))
            .filter(|k| !k.is_empty())
        {
            Some(key) => Auth::Key(key.clone()),
            None => Auth::Agent,
        }
    } else {
        Auth::Password
    };

    *next_id += 1;
    Some(common::session(
        *next_id,
        folder_id,
        name,
        protocol,
        Some(host),
        Some(port.unwrap_or_else(|| common::default_port(protocol))),
        username,
        auth,
        *next_id - 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, content: &str) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn walks_session_tree() {
        let root = std::env::temp_dir().join(format!("zapterm-scrt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        write(
            &root.join("edge.ini"),
            "S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=10.0.0.1\nD:\"[SSH2] Port\"=00000016\nS:\"Username\"=admin\nS:\"Identity Filename V2\"=/home/u/.ssh/id_ed25519\n",
        );
        write(
            &root.join("Lab/sw1.ini"),
            "S:\"Protocol Name\"=Telnet\nS:\"Hostname\"=192.168.1.9\nD:\"[TELNET] Port\"=00000017\n",
        );
        write(&root.join("__FolderData__.ini"), "S:\"x\"=y\n");

        let parsed = parse_dir(&root).unwrap();
        let s = &parsed.file.sessions;
        assert_eq!(s.len(), 2);

        let edge = s.iter().find(|x| x.name == "edge").unwrap();
        assert_eq!(edge.protocol, "ssh");
        assert_eq!(edge.host.as_deref(), Some("10.0.0.1"));
        assert_eq!(edge.port, Some(22)); // 0x16
        assert_eq!(edge.username.as_deref(), Some("admin"));
        assert_eq!(edge.auth_method.as_deref(), Some("key"));
        assert!(edge.options_json.contains("id_ed25519"));
        assert_eq!(edge.folder_id, None);

        let sw = s.iter().find(|x| x.name == "sw1").unwrap();
        assert_eq!(sw.protocol, "telnet");
        assert_eq!(sw.port, Some(23)); // 0x17
        let lab = parsed
            .file
            .folders
            .iter()
            .find(|f| f.name == "Lab")
            .unwrap();
        assert_eq!(sw.folder_id, Some(lab.id));

        let _ = std::fs::remove_dir_all(&root);
    }
}
