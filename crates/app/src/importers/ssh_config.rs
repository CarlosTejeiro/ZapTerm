//! `~/.ssh/config` → native [`ExportFile`] parser.
//!
//! Supported per-Host directives: `HostName`, `User`, `Port`,
//! `IdentityFile` (→ key auth, `~` expanded), `ProxyJump` (first hop,
//! resolved against other imported aliases). `Include` is followed (one
//! level of `*` filename glob). Everything ZapTerm can't represent is skipped
//! and surfaced as a warning or counted, never an error:
//!
//! - `Host` patterns with wildcards (`*`, `?`) or negations (`!`) — these
//!   are defaults/exclusions, not concrete hosts.
//! - `Match` blocks (conditional config).
//! - Any other directive (LocalForward, Ciphers, …).
//!
//! Hosts without `IdentityFile` default to `agent` auth — that mirrors what
//! plain `ssh` would try first and never blocks the UI with a prompt.

use std::path::{Path, PathBuf};

use core_persistence::SavedSession;

use crate::commands::transfer::ExportFile;

/// Result of a parse: the export envelope plus parser-level warnings
/// (the import pipeline adds its own on apply).
pub struct ParsedSshConfig {
    pub file: ExportFile,
    pub warnings: Vec<String>,
}

#[derive(Default, Clone)]
struct HostEntry {
    alias: String,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
}

/// Parse the ssh_config at `path`. `home` is used to expand `~` in
/// `IdentityFile`/`Include` values (passed in so tests don't depend on the
/// environment).
pub fn parse(path: &Path, home: &Path) -> std::io::Result<ParsedSshConfig> {
    let mut warnings = Vec::new();
    let mut hosts: Vec<HostEntry> = Vec::new();
    let mut skipped_patterns = 0usize;

    let mut lines = Vec::new();
    collect_lines(path, home, &mut lines, &mut warnings, 0)?;

    // State machine: `current` holds the Host block being filled. `None`
    // also covers Match blocks and wildcard Hosts (directives ignored).
    let mut current: Option<HostEntry> = None;
    for (raw_key, value) in lines {
        let key = raw_key.to_ascii_lowercase();
        match key.as_str() {
            "host" => {
                if let Some(h) = current.take() {
                    hosts.push(h);
                }
                // First concrete alias wins; pattern-only Host lines are
                // configuration defaults, not importable sessions.
                let alias = value
                    .split_whitespace()
                    .find(|a| !a.contains(['*', '?']) && !a.starts_with('!'));
                match alias {
                    Some(a) => {
                        current = Some(HostEntry {
                            alias: a.to_owned(),
                            ..Default::default()
                        })
                    }
                    None => {
                        skipped_patterns += 1;
                        current = None;
                    }
                }
            }
            "match" => {
                if let Some(h) = current.take() {
                    hosts.push(h);
                }
                warnings.push("bloque Match ignorado (config condicional no soportada)".into());
                current = None;
            }
            _ => {
                let Some(h) = current.as_mut() else { continue };
                match key.as_str() {
                    "hostname" => h.hostname = Some(value),
                    "user" => h.user = Some(value),
                    "port" => match value.parse::<u16>() {
                        Ok(p) => h.port = Some(p),
                        Err(_) => warnings.push(format!(
                            "host “{}”: invalid Port “{value}”, using 22",
                            h.alias
                        )),
                    },
                    // ssh tries identities in order; the first is ours.
                    "identityfile" if h.identity_file.is_none() => {
                        h.identity_file = Some(expand_home(&value, home))
                    }
                    "proxyjump" => h.proxy_jump = Some(value),
                    _ => {} // unsupported directive — fine
                }
            }
        }
    }
    if let Some(h) = current.take() {
        hosts.push(h);
    }
    if skipped_patterns > 0 {
        warnings.push(format!(
            "{skipped_patterns} Host block(s) with wildcards skipped (they hold defaults, not hosts)"
        ));
    }

    // Synthesize SavedSession rows. Ids are local to the file (1..n) — the
    // import pipeline remaps them; via_session_id resolves ProxyJump hops
    // that point at another imported alias.
    let mut sessions: Vec<SavedSession> = Vec::with_capacity(hosts.len());
    for (i, h) in hosts.iter().enumerate() {
        let (auth_method, options_json) = match &h.identity_file {
            Some(key) => (
                "key",
                format!(
                    r#"{{"key_path":{}}}"#,
                    serde_json::to_string(key).unwrap_or_default()
                ),
            ),
            None => ("agent", "{}".to_owned()),
        };
        sessions.push(SavedSession {
            id: (i + 1) as i64,
            folder_id: None,
            name: h.alias.clone(),
            protocol: "ssh".into(),
            host: Some(h.hostname.clone().unwrap_or_else(|| h.alias.clone())),
            port: Some(h.port.unwrap_or(22)),
            username: h.user.clone(),
            credential_id: None,
            options_json,
            last_used_at: None,
            auth_method: Some(auth_method.into()),
            via_session_id: None,
            login_script_json: None,
            sort_order: i as i64,
        });
    }
    for (i, h) in hosts.iter().enumerate() {
        let Some(jump) = &h.proxy_jump else { continue };
        if jump.eq_ignore_ascii_case("none") {
            continue;
        }
        let hops: Vec<&str> = jump.split(',').collect();
        if hops.len() > 1 {
            warnings.push(format!(
                "host “{}”: ProxyJump with {} hops — only the first is imported",
                h.alias,
                hops.len()
            ));
        }
        // Hop syntax is [user@]host[:port]; it matches an imported session
        // only when it's a bare alias defined in this same file.
        let hop = hops[0].trim();
        match hosts.iter().position(|o| o.alias == hop) {
            Some(idx) => sessions[i].via_session_id = Some((idx + 1) as i64),
            None => warnings.push(format!(
                "host “{}”: ProxyJump “{hop}” is not a host in this file — no jump host set",
                h.alias
            )),
        }
    }

    Ok(ParsedSshConfig {
        file: ExportFile {
            zapx_export: 1,
            app_version: String::new(),
            exported_at: String::new(),
            folders: vec![],
            sessions,
            broadcast_groups: vec![],
            snippets: vec![],
            highlight_rules: vec![],
            session_forwards: vec![],
            session_triggers: vec![],
        },
        warnings,
    })
}

/// Read `path` into (key, value) pairs, following `Include` directives
/// (depth-limited; one `*` glob in the filename component).
fn collect_lines(
    path: &Path,
    home: &Path,
    out: &mut Vec<(String, String)>,
    warnings: &mut Vec<String>,
    depth: u8,
) -> std::io::Result<()> {
    if depth > 4 {
        warnings.push(format!(
            "Include demasiado anidado, ignorado: {}",
            path.display()
        ));
        return Ok(());
    }
    let content = std::fs::read_to_string(path)?;
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `Key value`, `Key=value` and quoted values are all valid.
        let (key, value) = match line.split_once(['=', ' ', '\t']) {
            Some((k, v)) => (k.trim(), v.trim().trim_matches('"').to_owned()),
            None => continue,
        };
        if key.eq_ignore_ascii_case("include") {
            for inc in value.split_whitespace() {
                let expanded = expand_home(inc, home);
                let inc_path = if Path::new(&expanded).is_absolute() {
                    PathBuf::from(&expanded)
                } else {
                    // Relative includes resolve against ~/.ssh per ssh_config(5).
                    home.join(".ssh").join(&expanded)
                };
                for resolved in glob_simple(&inc_path) {
                    if let Err(e) = collect_lines(&resolved, home, out, warnings, depth + 1) {
                        warnings.push(format!("Include {}: {e}", resolved.display()));
                    }
                }
            }
            continue;
        }
        out.push((key.to_owned(), value));
    }
    Ok(())
}

fn expand_home(value: &str, home: &Path) -> String {
    if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest).to_string_lossy().into_owned()
    } else {
        value.to_owned()
    }
}

/// Minimal `*` glob over the FILENAME component only (`config.d/*` style).
/// Paths without `*` pass through untouched (existence checked by caller).
fn glob_simple(path: &Path) -> Vec<PathBuf> {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return vec![path.to_path_buf()];
    };
    if !name.contains('*') {
        return vec![path.to_path_buf()];
    }
    let Some(parent) = path.parent() else {
        return vec![];
    };
    let (prefix, suffix) = name.split_once('*').unwrap_or((name, ""));
    let Ok(entries) = std::fs::read_dir(parent) else {
        return vec![];
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| {
                    n.starts_with(prefix)
                        && n.ends_with(suffix)
                        && n.len() >= prefix.len() + suffix.len()
                })
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, content: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn parses_hosts_with_jump_keys_and_skips_patterns() {
        let dir = std::env::temp_dir().join(format!("zapterm-sshconf-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".ssh")).unwrap();
        let cfg = dir.join(".ssh/config");
        write(
            &cfg,
            r#"
# defaults for everything — must be skipped
Host *
  ServerAliveInterval 60

Host jump
  HostName bastion.example.com
  User ops

Host spine-01
  HostName 10.0.0.1
  Port 2222
  User admin
  IdentityFile ~/.ssh/id_lab
  ProxyJump jump

Host web !web-prod
  HostName web.internal
"#,
        );
        let parsed = parse(&cfg, &dir).unwrap();
        let s = &parsed.file.sessions;
        assert_eq!(s.len(), 3, "jump, spine-01, web");

        let jump = s.iter().find(|x| x.name == "jump").unwrap();
        assert_eq!(jump.host.as_deref(), Some("bastion.example.com"));
        assert_eq!(jump.port, Some(22));
        assert_eq!(jump.auth_method.as_deref(), Some("agent"));

        let spine = s.iter().find(|x| x.name == "spine-01").unwrap();
        assert_eq!(spine.port, Some(2222));
        assert_eq!(spine.username.as_deref(), Some("admin"));
        assert_eq!(spine.auth_method.as_deref(), Some("key"));
        assert!(spine.options_json.contains("id_lab"));
        assert!(!spine.options_json.contains('~'), "~ expanded");
        assert_eq!(spine.via_session_id, Some(jump.id));

        // `Host *` skipped silently into the patterns counter warning.
        assert!(parsed.warnings.iter().any(|w| w.contains("wildcards")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn follows_includes_with_glob() {
        let dir = std::env::temp_dir().join(format!("zapterm-sshinc-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".ssh/config.d")).unwrap();
        write(
            &dir.join(".ssh/config"),
            "Include config.d/*.conf\nHost main\n  HostName m.example\n",
        );
        write(
            &dir.join(".ssh/config.d/lab.conf"),
            "Host lab\n  HostName lab.example\n",
        );
        let parsed = parse(&dir.join(".ssh/config"), &dir).unwrap();
        let names: Vec<_> = parsed
            .file
            .sessions
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"lab"));
        assert!(names.contains(&"main"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
