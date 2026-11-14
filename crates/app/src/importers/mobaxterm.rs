//! MobaXterm `.ini` / `.mxtsessions` → native [`ExportFile`].
//!
//! Bookmarks live in `[Bookmarks]` (root) and `[Bookmarks_N]` sections. Each
//! section has a `SubRep` (folder path, `\`-separated, empty at root) and
//! `ImgNum` (icon, ignored); every other `key=value` is a session, the key
//! being the display name.
//!
//! The value encodes `#<icon>#<typeId>%<f1>%<f2>%…`. For SSH (typeId 0 /
//! icon 109) the field layout is well documented: f1=host, f2=port,
//! f3=username — that's the high-confidence path. Telnet (icon 98) and
//! serial (icon 131) are best-effort and warned; RDP/VNC/other are skipped
//! with a warning (ZapTerm has no such protocols).
//!
//! See https://gist.github.com/Ruzgfpegk/ab597838e4abbe8de30d7224afd062ea

use std::collections::HashMap;

use core_persistence::Folder;

use core_persistence::{SavedForward, SavedSession};

use crate::commands::transfer::SessionForwards;
use crate::importers::common::{self, Auth, Parsed};

pub fn parse(content: &str) -> Parsed {
    let mut warnings = Vec::new();

    // Section → (SubRep folder path, [(name, value)]).
    let mut section_subrep = String::new();
    let mut in_bookmarks = false;
    let mut in_portfwd = false;
    // Raw `[PortForwarding]` lines, resolved to forwards after the bookmarks.
    let mut forward_lines: Vec<String> = Vec::new();
    // Folder path string → local folder id; sessions reference these.
    let mut folder_ids: HashMap<String, i64> = HashMap::new();
    let mut folders: Vec<Folder> = Vec::new();
    let mut sessions = Vec::new();
    // Sessions reached through SSH gateways: (session id, hops).
    let mut gateway_hops: Vec<(i64, Vec<Hop>)> = Vec::new();
    let mut next_folder_id: i64 = 0;
    let mut next_session_id: i64 = 0;

    // Resolve a `\`-separated SubRep path to a leaf folder id, creating each
    // component (and its parents) once. Empty path → root (None).
    let mut ensure_folder = |path: &str,
                             folders: &mut Vec<Folder>,
                             folder_ids: &mut HashMap<String, i64>|
     -> Option<i64> {
        let path = path.trim().trim_matches('\\');
        if path.is_empty() {
            return None;
        }
        let mut parent: Option<i64> = None;
        let mut acc = String::new();
        for part in path.split('\\').filter(|p| !p.is_empty()) {
            if !acc.is_empty() {
                acc.push('\\');
            }
            acc.push_str(part);
            if let Some(&id) = folder_ids.get(&acc) {
                parent = Some(id);
                continue;
            }
            next_folder_id += 1;
            let id = next_folder_id;
            folders.push(common::folder(id, parent, part, (folders.len()) as i32));
            folder_ids.insert(acc.clone(), id);
            parent = Some(id);
        }
        parent
    };

    for raw in content.lines() {
        let line = raw.trim_end_matches(['\r', '\n']).trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_bookmarks = name == "Bookmarks" || name.starts_with("Bookmarks_");
            in_portfwd = name == "PortForwarding";
            section_subrep.clear();
            continue;
        }
        if in_portfwd {
            forward_lines.push(line.to_owned());
            continue;
        }
        if !in_bookmarks {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.eq_ignore_ascii_case("SubRep") {
            section_subrep = value.trim().to_owned();
            continue;
        }
        if key.eq_ignore_ascii_case("ImgNum") {
            continue;
        }
        // A session line. Decode its protocol + fields.
        let folder_id = ensure_folder(&section_subrep, &mut folders, &mut folder_ids);
        if let Some(row) =
            decode_session(key, value, folder_id, &mut next_session_id, &mut warnings)
        {
            let hops = gateways(value);
            if !hops.is_empty() {
                gateway_hops.push((row.id, hops));
            }
            sessions.push(row); // None = skipped, already warned
        }
    }

    // Jump hosts after every bookmark is known, so a gateway that is also a
    // bookmark is reused rather than duplicated.
    link_gateways(
        &mut sessions,
        &gateway_hops,
        &mut next_session_id,
        &mut warnings,
    );

    // Resolve [PortForwarding] tunnels against the imported gateway sessions
    // (creating a session for the gateway when none matches).
    let session_forwards = attach_forwards(
        &mut sessions,
        &forward_lines,
        &mut next_session_id,
        &mut warnings,
    );

    let mut file = common::envelope(folders, sessions);
    file.session_forwards = session_forwards;
    Parsed { file, warnings }
}

/// Parse the `[PortForwarding]` lines and attach each tunnel to the SSH
/// session that is its gateway. Line format (semicolon-separated):
/// `<key>.=<Type>;<user@host:port>;<dest_host:port>;<bind_port>;…;<bind_addr>;…`
/// where Type is Local / Remote / Dynamic.
fn attach_forwards(
    sessions: &mut Vec<SavedSession>,
    lines: &[String],
    next_id: &mut i64,
    warnings: &mut Vec<String>,
) -> Vec<SessionForwards> {
    let mut by_session: std::collections::HashMap<i64, Vec<SavedForward>> =
        std::collections::HashMap::new();

    for line in lines {
        let Some((_key, rhs)) = line.split_once('=') else {
            continue;
        };
        let f: Vec<&str> = rhs.split(';').collect();
        if f.len() < 4 {
            warnings.push(format!("tunnel “{line}”: unexpected format — skipped"));
            continue;
        }
        let kind = match f[0].trim().to_ascii_lowercase().as_str() {
            "local" => "local",
            "remote" => "remote",
            "dynamic" => "dynamic",
            other => {
                warnings.push(format!(
                    "tunnel “{}”: type “{other}” isn't supported — skipped",
                    f[0].trim()
                ));
                continue;
            }
        };

        // Gateway = [user@]host:port (the SSH session the tunnel runs through).
        let gw = f[1].trim();
        let (gw_user, gw_hostport) = match gw.split_once('@') {
            Some((u, h)) => (Some(u.trim()), h.trim()),
            None => (None, gw),
        };
        let (gw_host, gw_port) = gw_hostport
            .rsplit_once(':')
            .and_then(|(h, p)| p.trim().parse::<u16>().ok().map(|pp| (h.trim(), pp)))
            .unwrap_or((gw_hostport, 22));
        if gw_host.is_empty() {
            warnings.push(format!("tunnel “{line}”: no gateway — skipped"));
            continue;
        }

        let bind_port: u16 = match f.get(3).and_then(|s| s.trim().parse().ok()) {
            Some(p) if p > 0 => p,
            _ => {
                warnings.push(format!("tunnel “{line}”: invalid local port — skipped"));
                continue;
            }
        };
        let bind_addr = f
            .get(6)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("127.0.0.1")
            .to_owned();

        let (target_host, target_port) = if kind == "dynamic" {
            (None, None)
        } else {
            match f.get(2).and_then(|d| d.trim().rsplit_once(':')) {
                Some((h, p)) => match p.trim().parse::<u16>() {
                    Ok(pp) if !h.is_empty() => (Some(h.to_owned()), Some(pp)),
                    _ => {
                        warnings.push(format!("tunnel “{line}”: invalid destination — skipped"));
                        continue;
                    }
                },
                None => {
                    warnings.push(format!("tunnel “{line}”: no destination — skipped"));
                    continue;
                }
            }
        };

        let fwd = SavedForward {
            kind: kind.into(),
            bind_addr,
            bind_port,
            target_host,
            target_port,
            name: None,
        };

        // Attach to the matching gateway session, or create one for it.
        let sid = match sessions.iter().find(|s| {
            s.protocol == "ssh" && s.host.as_deref() == Some(gw_host) && s.port == Some(gw_port)
        }) {
            Some(s) => s.id,
            None => {
                *next_id += 1;
                let name = match gw_user {
                    Some(u) => format!("{u}@{gw_host}"),
                    None => gw_host.to_owned(),
                };
                sessions.push(common::session(
                    *next_id,
                    None,
                    &name,
                    "ssh",
                    Some(gw_host.to_owned()),
                    Some(gw_port),
                    gw_user.map(|u| u.to_owned()),
                    Auth::Password,
                    *next_id - 1,
                ));
                warnings.push(format!("created session “{name}” for a tunnel gateway"));
                *next_id
            }
        };
        by_session.entry(sid).or_default().push(fwd);
    }

    by_session
        .into_iter()
        .map(|(session_id, forwards)| SessionForwards {
            session_id,
            forwards,
        })
        .collect()
}

/// MobaXterm session types that ZapTerm can't open, by the type id the
/// reference documents (field 0 of the first `%` group).
fn unsupported_type(type_id: &str) -> Option<&'static str> {
    match type_id {
        "4" => Some("RDP"),
        "5" => Some("VNC"),
        "7" => Some("SFTP"),
        "11" => Some("browser"),
        _ => None,
    }
}

/// One SSH gateway hop: host, port, user.
type Hop = (String, u16, Option<String>);

/// A MobaXterm list field: `__PIPE__`-separated values (gateway hops).
fn pipe_list(s: Option<&str>) -> Vec<String> {
    s.map(|s| s.split("__PIPE__").map(|p| p.trim().to_owned()).collect())
        .unwrap_or_default()
}

fn decode_session(
    name: &str,
    value: &str,
    folder_id: Option<i64>,
    next_id: &mut i64,
    warnings: &mut Vec<String>,
) -> Option<core_persistence::SavedSession> {
    // value = <logout flag>#<icon>#<fields %-separated>#<terminal>#…
    // The flag is empty, a space, or ";  logout" when "Display reconnection
    // message" is unchecked — it must not hide the session.
    let mut groups = value.split('#');
    let _logout = groups.next();
    let (Some(icon), Some(rest)) = (groups.next(), groups.next()) else {
        warnings.push(format!("“{name}”: not a MobaXterm session line — skipped"));
        return None;
    };
    let fields: Vec<&str> = rest.split('%').collect();
    let type_id = fields.first().map(|s| s.trim()).unwrap_or("");
    let icon: u32 = icon.trim().parse().unwrap_or(0);

    // The type id decides; the icon (which the user can change) is only the
    // fallback for the types the reference doesn't give an id for: 98 and
    // 131 are MobaXterm's default Telnet and Serial icons.
    let protocol = if type_id == "0" {
        "ssh"
    } else if let Some(kind) = unsupported_type(type_id) {
        warnings.push(format!(
            "“{name}”: {kind} sessions aren't supported — skipped"
        ));
        return None;
    } else if icon == 98 {
        "telnet"
    } else if icon == 131 {
        "serial"
    } else {
        warnings.push(format!(
            "“{name}”: unsupported session type (type {type_id}, icon {icon}) — skipped"
        ));
        return None;
    };

    let field = |i: usize| fields.get(i).map(|s| s.trim()).filter(|s| !s.is_empty());

    if protocol == "serial" {
        // Best-effort: field 1 tends to be the COM/tty line, field 2 the speed.
        let device = field(1).unwrap_or("").to_owned();
        if device.is_empty() {
            warnings.push(format!("“{name}”: serial session without a port — skipped"));
            return None;
        }
        warnings.push(format!(
            "“{name}”: serial session imported on a best-effort basis — check its port and speed"
        ));
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
        let baud: u32 = field(2).and_then(|s| s.parse().ok()).unwrap_or(9600);
        row.options_json = format!(
            r#"{{"device":{},"baud_rate":{baud}}}"#,
            serde_json::to_string(&device).unwrap_or_else(|_| "\"\"".into())
        );
        return Some(row);
    }

    // SSH / Telnet: f1=host, f2=port, f3=username ("<default>" = MobaXterm's
    // global default login, i.e. no specific user).
    let Some(host) = field(1).map(|s| s.to_owned()) else {
        warnings.push(format!("“{name}”: no host — skipped"));
        return None;
    };
    let port = field(2)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or_else(|| common::default_port(protocol));
    let username = field(3)
        .filter(|u| !u.eq_ignore_ascii_case("<default>"))
        .map(|s| s.to_owned());
    // MobaXterm doesn't export stored passwords (they live encrypted in its
    // vault), so SSH defaults to password auth — ZapTerm then prompts on
    // connect — rather than agent, which hard-fails on the typical Windows
    // host with no agent running. A private key (field 14, with "C:" stored
    // as `_CurrentDrive_`) is used when that file exists on this computer.
    let mut auth = Auth::Password;
    if protocol == "ssh" {
        if let Some(key) = field(14) {
            let key = key.replace("_CurrentDrive_", "C:");
            if std::path::Path::new(&key).is_file() {
                auth = Auth::Key(key);
            } else {
                warnings.push(format!(
                    "“{name}”: private key {key} not found on this computer — set to password; pick the key in the session to use it"
                ));
            }
        }
    }
    *next_id += 1;
    let row = common::session(
        *next_id,
        folder_id,
        name,
        protocol,
        Some(host),
        Some(port),
        username,
        auth,
        *next_id - 1,
    );
    Some(row)
}

/// SSH gateways of a session line (fields 8, 9 and 10: hosts, ports and
/// users, `__PIPE__`-separated when there are several hops), first hop first.
fn gateways(value: &str) -> Vec<Hop> {
    let mut groups = value.split('#');
    let (_, _, Some(rest)) = (groups.next(), groups.next(), groups.next()) else {
        return vec![];
    };
    let fields: Vec<&str> = rest.split('%').collect();
    if fields.first().map(|s| s.trim()) != Some("0") {
        return vec![];
    }
    let field = |i: usize| fields.get(i).map(|s| s.trim()).filter(|s| !s.is_empty());
    let hosts = pipe_list(field(8));
    let ports = pipe_list(field(9));
    let users = pipe_list(field(10));
    hosts
        .into_iter()
        .enumerate()
        .filter(|(_, h)| !h.is_empty())
        .map(|(i, h)| {
            let port = ports.get(i).and_then(|p| p.parse().ok()).unwrap_or(22);
            let user = users
                .get(i)
                .filter(|u| !u.is_empty() && !u.eq_ignore_ascii_case("<default>"))
                .cloned();
            (h, port, user)
        })
        .collect()
}

/// Point each session that goes through SSH gateways at its last hop
/// (`via_session_id`), each hop at the one before it, reusing an imported
/// SSH session with the same host/port/user or creating one.
fn link_gateways(
    sessions: &mut Vec<SavedSession>,
    hops_of: &[(i64, Vec<Hop>)],
    next_id: &mut i64,
    warnings: &mut Vec<String>,
) {
    for (session_id, hops) in hops_of {
        let mut prev: Option<i64> = None;
        for (host, port, user) in hops {
            let found = sessions.iter().find(|s| {
                s.protocol == "ssh"
                    && s.host.as_deref() == Some(host.as_str())
                    && s.port == Some(*port)
                    && (user.is_none() || s.username == *user)
            });
            let hop_id = match found {
                Some(s) => s.id,
                None => {
                    *next_id += 1;
                    let name = match user {
                        Some(u) => format!("{u}@{host}"),
                        None => host.clone(),
                    };
                    let mut row = common::session(
                        *next_id,
                        None,
                        &name,
                        "ssh",
                        Some(host.clone()),
                        Some(*port),
                        user.clone(),
                        Auth::Password,
                        *next_id - 1,
                    );
                    row.via_session_id = prev;
                    sessions.push(row);
                    warnings.push(format!("created session “{name}” for an SSH gateway"));
                    *next_id
                }
            };
            prev = Some(hop_id);
        }
        if let (Some(last), Some(s)) = (prev, sessions.iter_mut().find(|s| s.id == *session_id)) {
            if last != s.id {
                s.via_session_id = Some(last);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[Bookmarks]
SubRep=
ImgNum=42
jump=#109#0%bastion.example%22%ops%%-1%-1%%%22%%0%0%0%%%-1%0%0%0

[Bookmarks_1]
SubRep=Datacenter\\Spines
ImgNum=41
spine-01=#109#0%10.0.0.1%2222%admin%%-1%-1
old-sw=#98#1%10.0.0.9%23%

[Bookmarks_2]
SubRep=Misc
ImgNum=41
desktop=#91#4%10.0.0.50%3389%user
";

    #[test]
    fn parses_bookmarks_with_folders() {
        let parsed = parse(SAMPLE);
        let s = &parsed.file.sessions;
        // jump, spine-01, old-sw  (desktop = RDP, skipped)
        assert_eq!(s.len(), 3);

        let jump = s.iter().find(|x| x.name == "jump").unwrap();
        assert_eq!(jump.protocol, "ssh");
        assert_eq!(jump.host.as_deref(), Some("bastion.example"));
        assert_eq!(jump.port, Some(22));
        assert_eq!(jump.username.as_deref(), Some("ops"));
        assert_eq!(jump.folder_id, None, "root section");
        // MobaXterm carries no exported secret → SSH defaults to password auth
        // (prompts on connect), never agent.
        assert_eq!(jump.auth_method.as_deref(), Some("password"));

        let spine = s.iter().find(|x| x.name == "spine-01").unwrap();
        assert_eq!(spine.port, Some(2222));
        // Nested folder Datacenter\Spines → spine sits in the leaf "Spines".
        let folders = &parsed.file.folders;
        let spines = folders.iter().find(|f| f.name == "Spines").unwrap();
        let dc = folders.iter().find(|f| f.name == "Datacenter").unwrap();
        assert_eq!(spines.parent_id, Some(dc.id));
        assert_eq!(spine.folder_id, Some(spines.id));

        let sw = s.iter().find(|x| x.name == "old-sw").unwrap();
        assert_eq!(sw.protocol, "telnet");
        assert_eq!(sw.port, Some(23));

        // RDP desktop was skipped with a warning.
        assert!(parsed.warnings.iter().any(|w| w.contains("desktop")));
    }

    // Real-world [PortForwarding] line layout (from an exported .mobaconf):
    // Type ; user@host:port ; dest:port ; bind_port ; ? ; key ; bind_addr ; proxy ; ?
    const FWD_SAMPLE: &str = "\
[Bookmarks]
SubRep=
ImgNum=42
jump=#109#0%bastion.example%22%ops%%-1%-1

[PortForwarding]
0000.=Local;ops@bastion.example:22;1.2.3.4:80;8080;0;No SSH key selected;0.0.0.0;No proxy selected;0
0001.=Dynamic;deploy@10.9.9.9:2200;;1080;0;No SSH key selected;127.0.0.1;No proxy selected;0
";

    #[test]
    fn imports_port_forwardings() {
        let parsed = parse(FWD_SAMPLE);
        let sessions = &parsed.file.sessions;

        // Local forward attaches to the existing "jump" gateway bookmark.
        let jump = sessions.iter().find(|s| s.name == "jump").unwrap();
        let jump_fwd = parsed
            .file
            .session_forwards
            .iter()
            .find(|sf| sf.session_id == jump.id)
            .expect("local forward attached to the jump session");
        assert_eq!(jump_fwd.forwards.len(), 1);
        let lf = &jump_fwd.forwards[0];
        assert_eq!(lf.kind, "local");
        assert_eq!(lf.bind_addr, "0.0.0.0");
        assert_eq!(lf.bind_port, 8080);
        assert_eq!(lf.target_host.as_deref(), Some("1.2.3.4"));
        assert_eq!(lf.target_port, Some(80));

        // Dynamic forward's gateway isn't a bookmark → a session is created.
        let gw = sessions
            .iter()
            .find(|s| s.name == "deploy@10.9.9.9")
            .unwrap();
        assert_eq!(gw.port, Some(2200));
        assert_eq!(gw.auth_method.as_deref(), Some("password"));
        let dyn_fwd = parsed
            .file
            .session_forwards
            .iter()
            .find(|sf| sf.session_id == gw.id)
            .expect("dynamic forward attached to the created gateway session");
        assert_eq!(dyn_fwd.forwards[0].kind, "dynamic");
        assert_eq!(dyn_fwd.forwards[0].bind_port, 1080);
        assert!(dyn_fwd.forwards[0].target_host.is_none());
    }

    /// Field layouts from the format reference (MobaXterm 26.3): a session
    /// with "Display reconnection message" unchecked starts with
    /// ";  logout"; `<default>` means no specific user; the type id, not the
    /// icon, decides the protocol.
    #[test]
    fn reads_real_line_variants() {
        let key = std::env::temp_dir().join(format!("zapterm-moba-key-{}", std::process::id()));
        std::fs::write(&key, "x").unwrap();
        let key_s = key.to_string_lossy().to_string();
        let sample = format!(
            "[Bookmarks]\nSubRep=\nImgNum=42\n\
             quiet=;  logout#109#0%10.1.1.1%22%admin%%-1%-1\n\
             anyuser= #109#0%10.1.1.2%22%<default>%%-1%-1%%%%%0%-1\n\
             rdp-ssh-icon=#109#4%10.1.1.3%3389%user\n\
             keyed=#109#0%10.1.1.4%22%ops%%-1%-1%%%%%0%0%0%{key_s}\n\
             nokey=#109#0%10.1.1.5%22%ops%%-1%-1%%%%%0%0%0%_CurrentDrive_\\\\nowhere\\\\id_rsa\n"
        );
        let parsed = parse(&sample);
        let s = &parsed.file.sessions;
        let names: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["quiet", "anyuser", "keyed", "nokey"]);
        assert_eq!(s[0].host.as_deref(), Some("10.1.1.1"));
        assert_eq!(s[1].username, None, "<default> is no specific user");
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("rdp-ssh-icon") && w.contains("RDP")));
        assert_eq!(s[2].auth_method.as_deref(), Some("key"));
        assert!(s[2].options_json.contains("zapterm-moba-key"));
        assert_eq!(s[3].auth_method.as_deref(), Some("password"));
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("nokey") && w.contains("C:")));
        let _ = std::fs::remove_file(key);
    }

    /// A CP1252 export (MobaXterm's encoding) with accents and € imports.
    #[test]
    fn imports_a_cp1252_file() {
        let bytes = b"[Bookmarks]\r\nSubRep=Producci\xF3n\r\nImgNum=41\r\nCaja \x80=#109#0%10.0.0.1%22%root\r\n";
        let parsed = parse(&common::decode_text(bytes));
        assert_eq!(parsed.file.folders[0].name, "Producción");
        assert_eq!(parsed.file.sessions[0].name, "Caja €");
    }

    /// SSH gateways (fields 8–10) become jump hosts: a gateway that is
    /// also a bookmark is reused; several hops chain one through another.
    #[test]
    fn ssh_gateways_become_jump_hosts() {
        let sample = "[Bookmarks]\nSubRep=\nImgNum=42\n\
            bastion=#109#0%bastion.example%22%ops%%-1%-1\n\
            inner=#109#0%10.0.0.1%22%admin%%-1%-1%%bastion.example%22%ops\n\
            deep=#109#0%10.0.0.2%22%admin%%-1%-1%%gw1.example__PIPE__gw2.example%22__PIPE__2222%u1__PIPE__<default>\n";
        let parsed = parse(sample);
        let s = &parsed.file.sessions;
        let by = |n: &str| s.iter().find(|x| x.name == n).unwrap();
        assert_eq!(by("inner").via_session_id, Some(by("bastion").id));
        let gw1 = by("u1@gw1.example");
        let gw2 = by("gw2.example");
        assert_eq!(gw2.port, Some(2222));
        assert_eq!(gw2.via_session_id, Some(gw1.id));
        assert_eq!(by("deep").via_session_id, Some(gw2.id));
        assert_eq!(by("bastion").via_session_id, None);
        assert_eq!(s.len(), 5);
    }
}
