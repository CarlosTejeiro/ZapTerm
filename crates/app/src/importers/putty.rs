//! PuTTY → native [`ExportFile`].
//!
//! Two sources:
//! - A `.reg` export of `HKCU\Software\SimonTatham\PuTTY\Sessions` (exported
//!   from regedit, or carried between machines) — parsed everywhere, tested.
//! - On Windows, the live registry under that key (no export needed).
//!
//! Both feed [`from_sessions`], which maps PuTTY's per-session values onto
//! ZapTerm sessions:
//! - `Protocol` ssh/telnet/serial → protocol (raw and others skipped, warned)
//! - `HostName`, `PortNumber`, `UserName`
//! - `PublicKeyFile` → key auth (path kept verbatim — may be a `.ppk` that
//!   ZapTerm's russh backend can't read directly; warned)
//! - `SerialLine` + `SerialSpeed` → serial device/baud
//!
//! PuTTY's "Default Settings" profile is a template, not a host, so it's
//! skipped. Folders aren't modelled (PuTTY session names sometimes encode a
//! path with `/`, but that's not standardised, so names are kept as-is).

use core_persistence::SavedForward;

use crate::commands::transfer::SessionForwards;
use crate::importers::common::{self, Auth, Parsed};

/// One PuTTY session's raw values (strings + dword ints), name decoded.
#[derive(Default, Clone)]
pub struct PuttySession {
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u32>,
    pub protocol: Option<String>,
    pub username: Option<String>,
    pub public_key_file: Option<String>,
    pub serial_line: Option<String>,
    pub serial_speed: Option<u32>,
    /// PuTTY's `PortForwardings` string, e.g. `L8080=10.0.0.1:80,D1080`.
    pub port_forwardings: Option<String>,
}

/// Parse a `.reg` export into PuTTY sessions.
pub fn parse_reg(content: &str) -> Vec<PuttySession> {
    let mut out: Vec<PuttySession> = Vec::new();
    let mut current: Option<PuttySession> = None;
    let marker = r"\PuTTY\Sessions\";

    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            if let Some(s) = current.take() {
                out.push(s);
            }
            // [HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\<name>]
            if let Some(pos) = line.find(marker) {
                let name_enc = line[pos + marker.len()..].trim_end_matches(']');
                current = Some(PuttySession {
                    name: percent_decode(name_enc),
                    ..Default::default()
                });
            } else {
                current = None; // some other registry key in the same export
            }
            continue;
        }
        let Some(s) = current.as_mut() else { continue };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        match key {
            "HostName" => s.host = unquote(value),
            "Protocol" => s.protocol = unquote(value).map(|p| p.to_ascii_lowercase()),
            "UserName" => s.username = unquote(value),
            "PublicKeyFile" => s.public_key_file = unquote(value),
            "SerialLine" => s.serial_line = unquote(value),
            "PortNumber" => s.port = parse_dword(value),
            "SerialSpeed" => s.serial_speed = parse_dword(value),
            "PortForwardings" => s.port_forwardings = unquote(value).filter(|v| !v.is_empty()),
            _ => {}
        }
    }
    if let Some(s) = current.take() {
        out.push(s);
    }
    out
}

/// Read PuTTY sessions from the live Windows registry. Errors (key missing,
/// access denied) yield an empty list — the caller turns that into a clear
/// "no sessions found" rather than failing the whole import.
#[cfg(target_os = "windows")]
pub fn read_registry() -> Vec<PuttySession> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(sessions) = hkcu.open_subkey(r"Software\SimonTatham\PuTTY\Sessions") else {
        return vec![];
    };
    let mut out = Vec::new();
    for name_enc in sessions.enum_keys().flatten() {
        let Ok(key) = sessions.open_subkey(&name_enc) else {
            continue;
        };
        let mut s = PuttySession {
            name: percent_decode(&name_enc),
            ..Default::default()
        };
        s.host = key.get_value("HostName").ok();
        s.protocol = key
            .get_value::<String, _>("Protocol")
            .ok()
            .map(|p| p.to_ascii_lowercase());
        s.username = key.get_value("UserName").ok();
        s.public_key_file = key.get_value("PublicKeyFile").ok();
        s.serial_line = key.get_value("SerialLine").ok();
        s.port = key.get_value::<u32, _>("PortNumber").ok();
        s.serial_speed = key.get_value::<u32, _>("SerialSpeed").ok();
        s.port_forwardings = key
            .get_value::<String, _>("PortForwardings")
            .ok()
            .filter(|v| !v.is_empty());
        out.push(s);
    }
    out
}

#[cfg(not(target_os = "windows"))]
pub fn read_registry() -> Vec<PuttySession> {
    vec![]
}

/// Parse PuTTY's `PortForwardings` string into native forwards.
///
/// Comma-separated specs: `L[bind:]port=host:port` (local), `R…` (remote),
/// `D[bind:]port` (dynamic SOCKS). An optional `4`/`6` after the type letter
/// forces the IP version and is ignored. Unparseable specs are warned, not
/// fatal.
pub fn parse_port_forwardings(
    spec: &str,
    session: &str,
    warnings: &mut Vec<String>,
) -> Vec<SavedForward> {
    let mut out = Vec::new();
    for raw in spec.split(',') {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let mut chars = item.chars();
        let kind = match chars.next().map(|c| c.to_ascii_uppercase()) {
            Some('L') => "local",
            Some('R') => "remote",
            Some('D') => "dynamic",
            _ => {
                warnings.push(format!(
                    "“{session}”: forward “{item}” not recognised — skipped"
                ));
                continue;
            }
        };
        let mut rest = chars.as_str();
        // Optional IP-version selector right after the type letter.
        if rest.starts_with('4') || rest.starts_with('6') {
            rest = &rest[1..];
        }
        let (source, dest) = match rest.split_once('=') {
            Some((s, d)) => (s, Some(d)),
            None => (rest, None),
        };
        // source = [bindaddr:]port
        let (bind_addr, bind_port_s) = match source.rsplit_once(':') {
            Some((a, p)) => (a.to_owned(), p),
            None => ("127.0.0.1".to_owned(), source),
        };
        let Ok(bind_port) = bind_port_s.trim().parse::<u16>() else {
            warnings.push(format!(
                "“{session}”: forward “{item}”: invalid port — skipped"
            ));
            continue;
        };
        if kind == "dynamic" {
            out.push(SavedForward {
                kind: kind.into(),
                bind_addr,
                bind_port,
                target_host: None,
                target_port: None,
                name: None,
            });
            continue;
        }
        // local/remote need host:port
        let Some(dest) = dest else {
            warnings.push(format!(
                "“{session}”: forward “{item}” has no destination — skipped"
            ));
            continue;
        };
        let Some((th, tp_s)) = dest.rsplit_once(':') else {
            warnings.push(format!(
                "“{session}”: forward “{item}”: invalid destination — skipped"
            ));
            continue;
        };
        let Ok(tp) = tp_s.trim().parse::<u16>() else {
            warnings.push(format!(
                "“{session}”: forward “{item}”: invalid destination port — skipped"
            ));
            continue;
        };
        out.push(SavedForward {
            kind: kind.into(),
            bind_addr,
            bind_port,
            target_host: Some(th.to_owned()),
            target_port: Some(tp),
            name: None,
        });
    }
    out
}

/// Map decoded PuTTY sessions onto the native envelope.
pub fn from_sessions(sessions: &[PuttySession]) -> Parsed {
    let mut warnings = Vec::new();
    let mut rows = Vec::new();
    let mut forwards: Vec<SessionForwards> = Vec::new();
    let mut idx: i64 = 0;
    for s in sessions {
        // The default profile is a template, not a connectable host.
        if s.name.eq_ignore_ascii_case("Default Settings") || s.name.is_empty() {
            continue;
        }
        let proto = s.protocol.as_deref().unwrap_or("ssh");
        match proto {
            "ssh" | "telnet" => {
                let Some(host) = s.host.clone().filter(|h| !h.is_empty()) else {
                    warnings.push(format!("“{}”: no HostName — skipped", s.name));
                    continue;
                };
                let auth = match (proto, &s.public_key_file) {
                    ("ssh", Some(k)) if !k.is_empty() => {
                        if k.to_ascii_lowercase().ends_with(".ppk") {
                            warnings.push(format!(
                                "“{}”: PuTTY key {k} (.ppk) — convert it to OpenSSH format to use it",
                                s.name
                            ));
                        }
                        Auth::Key(k.clone())
                    }
                    ("ssh", _) => Auth::Agent,
                    _ => Auth::Password, // telnet has no auth method
                };
                idx += 1;
                rows.push(common::session(
                    idx,
                    None,
                    &s.name,
                    proto,
                    Some(host),
                    Some(
                        s.port
                            .and_then(|p| u16::try_from(p).ok())
                            .unwrap_or_else(|| common::default_port(proto)),
                    ),
                    s.username.clone().filter(|u| !u.is_empty()),
                    auth,
                    idx - 1,
                ));
                // Saved port-forwards (SSH only).
                if proto == "ssh" {
                    if let Some(pf) = &s.port_forwardings {
                        let fwds = parse_port_forwardings(pf, &s.name, &mut warnings);
                        if !fwds.is_empty() {
                            forwards.push(SessionForwards {
                                session_id: idx,
                                forwards: fwds,
                            });
                        }
                    }
                }
            }
            "serial" => {
                let Some(device) = s.serial_line.clone().filter(|d| !d.is_empty()) else {
                    warnings.push(format!(
                        "“{}”: serial session without SerialLine — skipped",
                        s.name
                    ));
                    continue;
                };
                idx += 1;
                let mut row = common::session(
                    idx,
                    None,
                    &s.name,
                    "serial",
                    None,
                    None,
                    None,
                    Auth::Password,
                    idx - 1,
                );
                let baud = s.serial_speed.unwrap_or(9600);
                row.options_json = format!(
                    r#"{{"device":{},"baud_rate":{baud}}}"#,
                    serde_json::to_string(&device).unwrap_or_else(|_| "\"\"".into())
                );
                rows.push(row);
            }
            other => warnings.push(format!(
                "“{}”: protocol “{other}” isn't supported — skipped",
                s.name
            )),
        }
    }
    let mut file = common::envelope(vec![], rows);
    file.session_forwards = forwards;
    Parsed { file, warnings }
}

/// PuTTY percent-decodes session names (`%XX`, hex). Bad escapes pass through.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `"value"` → `value`, unescaping `\\` and `\"`. Non-quoted input → None.
fn unquote(v: &str) -> Option<String> {
    let v = v.trim();
    if !v.starts_with('"') || !v.ends_with('"') || v.len() < 2 {
        return None;
    }
    let inner = &v[1..v.len() - 1];
    Some(inner.replace("\\\\", "\\").replace("\\\"", "\""))
}

/// `dword:0000001a` → 26.
fn parse_dword(v: &str) -> Option<u32> {
    let hex = v.trim().strip_prefix("dword:")?;
    u32::from_str_radix(hex.trim(), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Default%20Settings]
"HostName"=""
"Protocol"="ssh"

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\edge%20router]
"HostName"="10.0.0.1"
"PortNumber"=dword:0000001a
"Protocol"="ssh"
"UserName"="admin"
"PublicKeyFile"="C:\\keys\\id.ppk"
"PortForwardings"="L8080=10.0.0.1:80,D1080,R2000=localhost:22"

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\console]
"Protocol"="serial"
"SerialLine"="COM3"
"SerialSpeed"=dword:00002580

[HKEY_CURRENT_USER\Software\SomethingElse\Foo]
"Bar"="baz"
"#;

    #[test]
    fn parses_reg_export() {
        let parsed = from_sessions(&parse_reg(SAMPLE));
        let s = &parsed.file.sessions;
        // Default Settings + the unrelated key are skipped.
        assert_eq!(s.len(), 2, "edge router + console");

        let edge = s.iter().find(|x| x.name == "edge router").unwrap();
        assert_eq!(edge.protocol, "ssh");
        assert_eq!(edge.host.as_deref(), Some("10.0.0.1"));
        assert_eq!(edge.port, Some(26)); // 0x1a
        assert_eq!(edge.username.as_deref(), Some("admin"));
        assert_eq!(edge.auth_method.as_deref(), Some("key"));
        assert!(edge.options_json.contains("id.ppk"));
        assert!(parsed.warnings.iter().any(|w| w.contains(".ppk")));

        let console = s.iter().find(|x| x.name == "console").unwrap();
        assert_eq!(console.protocol, "serial");
        assert!(console.options_json.contains("COM3"));
        assert!(console.options_json.contains("9600")); // 0x2580
    }

    #[test]
    fn parses_port_forwardings() {
        let parsed = from_sessions(&parse_reg(SAMPLE));
        // The edge router is the first SSH session (file-local id 1).
        let sf = parsed
            .file
            .session_forwards
            .iter()
            .find(|f| f.session_id == 1)
            .unwrap();
        assert_eq!(sf.forwards.len(), 3);
        let local = sf.forwards.iter().find(|f| f.kind == "local").unwrap();
        assert_eq!(local.bind_port, 8080);
        assert_eq!(local.target_host.as_deref(), Some("10.0.0.1"));
        assert_eq!(local.target_port, Some(80));
        let dyn_f = sf.forwards.iter().find(|f| f.kind == "dynamic").unwrap();
        assert_eq!(dyn_f.bind_port, 1080);
        assert!(dyn_f.target_host.is_none());
        let remote = sf.forwards.iter().find(|f| f.kind == "remote").unwrap();
        assert_eq!(remote.bind_port, 2000);
        assert_eq!(remote.target_host.as_deref(), Some("localhost"));
    }

    #[test]
    fn percent_decode_roundtrip() {
        assert_eq!(percent_decode("my%20host"), "my host");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("100%done"), "100%done"); // bad escape kept
    }
}
