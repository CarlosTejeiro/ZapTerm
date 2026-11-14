//! SecureCRT → native [`ExportFile`].
//!
//! SecureCRT stores each session as a VanDyke-INI file under its
//! `Config/Sessions/` tree; subdirectories are the session folders. We walk
//! that directory: every `*.ini` is a session, its relative directory path
//! becomes the ZapTerm folder hierarchy.
//!
//! VanDyke INI lines are typed: `S:"Key"=str`, `D:"Key"=hexdword`,
//! `B:"Key"=…` (binary, continued on indented lines). The key names below
//! are the ones VanDyke's own scripting examples read and write
//! (vandyke.com/support/scripting/scripting-examples):
//! - `S:"Protocol Name"` → SSH2/SSH1 → ssh, Telnet → telnet, Serial → serial
//! - `S:"Hostname"`, `S:"Username"`
//! - the port: `D:"[SSH2] Port"` for SSH2, `D:"[SSH1] Port"` for SSH1,
//!   `D:"Port"` for Telnet
//! - `S:"Com Port"` + `D:"Baud Rate"` → serial device/speed
//! - `S:"Identity Filename V2"` / `S:"Identity Filename"` → SSH key path
//! - `S:"Firewall Name"` = `Session:<name>` → jump host (another session)
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
    // Session id → its path under the root ("Folder/Name") and the session
    // it names as its jump host, resolved once every file is read.
    let mut paths: HashMap<i64, String> = HashMap::new();
    let mut jumps: Vec<(i64, String)> = Vec::new();
    let mut next_folder_id: i64 = 0;
    let mut next_session_id: i64 = 0;

    // Depth-first walk so parent folders are created before children.
    let mut stack: Vec<(std::path::PathBuf, Option<i64>, String)> =
        vec![(root.to_path_buf(), None, String::new())];
    while let Some((dir, parent_id, rel)) = stack.pop() {
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
                match common::read_text(&path) {
                    Ok(content) => {
                        if let Some((row, jump)) = decode_session(
                            stem,
                            &content,
                            parent_id,
                            &mut next_session_id,
                            &mut warnings,
                        ) {
                            paths.insert(row.id, format!("{rel}{stem}"));
                            if let Some(j) = jump {
                                jumps.push((row.id, j));
                            }
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
            let sub_rel = format!("{rel}{name}/");
            stack.push((sub, Some(id), sub_rel));
        }
    }

    // `Firewall Name` = "Session:<path>": the session to connect through.
    for (id, target) in jumps {
        let target = target.replace('\\', "/");
        let found = paths
            .iter()
            .find(|(_, p)| **p == target)
            .or_else(|| {
                // A bare name: accept it when exactly one session has it.
                let mut by_name = paths
                    .iter()
                    .filter(|(_, p)| p.rsplit('/').next() == Some(target.as_str()));
                match (by_name.next(), by_name.next()) {
                    (Some(hit), None) => Some(hit),
                    _ => None,
                }
            })
            .map(|(&jid, _)| jid);
        let name = paths.get(&id).cloned().unwrap_or_default();
        match found {
            Some(jid) if jid != id => {
                if let Some(s) = sessions.iter_mut().find(|s| s.id == id) {
                    s.via_session_id = Some(jid);
                }
            }
            _ => warnings.push(format!(
                "“{name}”: jump host session “{target}” isn't in the folder — connects directly"
            )),
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
) -> Option<(core_persistence::SavedSession, Option<String>)> {
    let mut strings: HashMap<String, String> = HashMap::new();
    let mut dwords: HashMap<String, u32> = HashMap::new();

    for raw in content.lines() {
        // Indented lines continue a multi-line (binary or list) value.
        if raw.starts_with([' ', '\t']) {
            continue;
        }
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
            warnings.push(format!("“{name}”: no protocol and no host — skipped"));
            return None;
        }
    } else {
        warnings.push(format!(
            "“{name}”: protocol “{proto_name}” isn't supported — skipped"
        ));
        return None;
    };

    // The port key depends on the protocol.
    let port_key = match proto_name.as_str() {
        "ssh2" => "[SSH2] Port",
        "ssh1" => "[SSH1] Port",
        _ => "Port",
    };
    let port = dwords
        .get(port_key)
        .and_then(|&v| u16::try_from(v).ok())
        .filter(|&p| p > 0);

    if protocol == "serial" {
        let device = strings.get("Com Port").cloned().unwrap_or_default();
        if device.is_empty() {
            warnings.push(format!(
                "“{name}”: serial session without a COM port — skipped"
            ));
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
        return Some((row, None));
    }

    let host = strings.get("Hostname").cloned().filter(|h| !h.is_empty());
    let Some(host) = host else {
        warnings.push(format!("“{name}”: no Hostname — skipped"));
        return None;
    };
    let username = strings.get("Username").cloned().filter(|u| !u.is_empty());

    // SecureCRT exports no passwords: password auth (ZapTerm prompts on
    // connect) unless the session names a key file that exists here. A path
    // through a SecureCRT variable (`${VDS_USER_DATA_PATH}\Identity`) can't
    // be resolved outside SecureCRT.
    let mut auth = Auth::Password;
    if protocol == "ssh" {
        if let Some(key) = strings
            .get("Identity Filename V2")
            .or_else(|| strings.get("Identity Filename"))
            .filter(|k| !k.is_empty())
        {
            if !key.contains("${") && Path::new(key).is_file() {
                auth = Auth::Key(key.clone());
            } else {
                warnings.push(format!(
                    "“{name}”: key {key} not found on this computer — set to password; pick the key in the session to use it"
                ));
            }
        }
    }
    let jump = strings
        .get("Firewall Name")
        .and_then(|f| f.strip_prefix("Session:"))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    *next_id += 1;
    let row = common::session(
        *next_id,
        folder_id,
        name,
        protocol,
        Some(host),
        Some(port.unwrap_or_else(|| common::default_port(protocol))),
        username,
        auth,
        *next_id - 1,
    );
    Some((row, jump))
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
        let key = root.join("keys/id_ed25519");
        write(&key, "x");

        // A real SSH2 session also carries other "…Port" keys; only the
        // protocol's own key is the port. Indented lines continue a binary
        // value and must not be read as keys.
        write(
            &root.join("edge.ini"),
            &format!(
                "\u{feff}S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=10.0.0.1\nD:\"Port\"=00000017\n\
                 B:\"Normal Font v2\"=00000060\n f3 ff ff ff 00 00\n S:\"Hostname\"=evil\n\
                 D:\"[SSH2] Port\"=00000016\nD:\"[SSH1] Port\"=00000063\nS:\"Username\"=admin\n\
                 S:\"Identity Filename V2\"={}\n",
                key.display()
            ),
        );
        write(
            &root.join("Lab/sw1.ini"),
            "S:\"Protocol Name\"=Telnet\nS:\"Hostname\"=192.168.1.9\nD:\"Port\"=00000017\nD:\"[SSH2] Port\"=00000016\n",
        );
        write(
            &root.join("Lab/console.ini"),
            "S:\"Protocol Name\"=Serial\nS:\"Com Port\"=COM3\nD:\"Baud Rate\"=00002580\n",
        );
        write(
            &root.join("Lab/inner.ini"),
            "S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=10.9.9.9\nS:\"Firewall Name\"=Session:Jumps/bastion\n\
             S:\"Identity Filename V2\"=${VDS_USER_DATA_PATH}\\Identity\n",
        );
        write(
            &root.join("Jumps/bastion.ini"),
            "S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=bastion.example\nD:\"[SSH2] Port\"=000008ae\n",
        );
        write(
            &root.join("Lab/lost.ini"),
            "S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=10.9.9.8\nS:\"Firewall Name\"=Session:missing\n",
        );
        write(&root.join("__FolderData__.ini"), "S:\"x\"=y\n");

        let parsed = parse_dir(&root).unwrap();
        let s = &parsed.file.sessions;
        assert_eq!(s.len(), 6);
        let by = |n: &str| s.iter().find(|x| x.name == n).unwrap();

        let edge = by("edge");
        assert_eq!(edge.protocol, "ssh");
        assert_eq!(edge.host.as_deref(), Some("10.0.0.1"));
        assert_eq!(edge.port, Some(22)); // [SSH2] Port = 0x16
        assert_eq!(edge.username.as_deref(), Some("admin"));
        assert_eq!(edge.auth_method.as_deref(), Some("key"));
        assert!(edge.options_json.contains("id_ed25519"));
        assert_eq!(edge.folder_id, None);

        let sw = by("sw1");
        assert_eq!(sw.protocol, "telnet");
        assert_eq!(sw.port, Some(23)); // Port = 0x17
        let lab = parsed
            .file
            .folders
            .iter()
            .find(|f| f.name == "Lab")
            .unwrap();
        assert_eq!(sw.folder_id, Some(lab.id));

        let console = by("console");
        assert_eq!(console.protocol, "serial");
        assert!(console.options_json.contains("COM3"));
        assert!(console.options_json.contains("9600")); // 0x2580

        let bastion = by("bastion");
        assert_eq!(bastion.port, Some(2222)); // 0x8ae
        let inner = by("inner");
        assert_eq!(inner.via_session_id, Some(bastion.id));
        assert_eq!(inner.auth_method.as_deref(), Some("password"));
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("VDS_USER_DATA_PATH")));

        assert_eq!(by("lost").via_session_id, None);
        assert!(parsed.warnings.iter().any(|w| w.contains("missing")));

        let _ = std::fs::remove_dir_all(&root);
    }
}
