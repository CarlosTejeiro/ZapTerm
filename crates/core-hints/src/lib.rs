#![forbid(unsafe_code)]

//! `core-hints` — suggestion engine for the inline ghost-text and the
//! Ctrl+Space popup. Combines three sources:
//!
//! 1. Per-session command history (persisted, frecency-scored).
//! 2. Built-in command catalogs for common platforms (Linux, Cisco IOS, …).
//! 3. User-defined snippets.
//!
//! The engine never persists commands that look like they contain
//! credentials — see [`is_sensitive`].

pub mod catalog;
pub mod detector;
pub mod error;

use std::str::FromStr;
use std::sync::OnceLock;

use core_persistence::Database;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub use error::Error;

// ---------------------------------------------------------------------------
// Platform
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Generic,
    Linux,
    CiscoIos,
    CiscoIosXr,
    AristaEos,
    Windows,
    Juniper,
    Mikrotik,
    Aruba,
    Fortigate,
    Fortimanager,
    PaloAlto,
    F5Bigip,
    CheckpointGaia,
    HpComware,
    BrocadeFos,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Generic => "generic",
            Platform::Linux => "linux",
            Platform::CiscoIos => "cisco_ios",
            Platform::CiscoIosXr => "cisco_iosxr",
            Platform::AristaEos => "arista_eos",
            Platform::Windows => "windows",
            Platform::Juniper => "juniper",
            Platform::Mikrotik => "mikrotik",
            Platform::Aruba => "aruba",
            Platform::Fortigate => "fortigate",
            Platform::Fortimanager => "fortimanager",
            Platform::PaloAlto => "paloalto",
            Platform::F5Bigip => "f5_bigip",
            Platform::CheckpointGaia => "checkpoint_gaia",
            Platform::HpComware => "hp_comware",
            Platform::BrocadeFos => "brocade_fos",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Platform::Generic => "Generic",
            Platform::Linux => "Linux / bash",
            Platform::CiscoIos => "Cisco IOS",
            Platform::CiscoIosXr => "Cisco IOS XR",
            Platform::AristaEos => "Arista EOS",
            Platform::Windows => "Windows (PowerShell)",
            Platform::Juniper => "Juniper Junos",
            Platform::Mikrotik => "MikroTik RouterOS",
            Platform::Aruba => "Aruba (HP ProCurve)",
            Platform::Fortigate => "Fortinet FortiGate (FortiOS)",
            Platform::Fortimanager => "Fortinet FortiManager",
            Platform::PaloAlto => "Palo Alto (PAN-OS)",
            Platform::F5Bigip => "F5 BIG-IP (TMOS / tmsh)",
            Platform::CheckpointGaia => "Check Point Gaia (CLISH)",
            Platform::HpComware => "HPE Comware (HP / H3C)",
            Platform::BrocadeFos => "Brocade Fabric OS",
        }
    }
}

impl FromStr for Platform {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "generic" => Ok(Platform::Generic),
            "linux" => Ok(Platform::Linux),
            "cisco_ios" => Ok(Platform::CiscoIos),
            "cisco_iosxr" => Ok(Platform::CiscoIosXr),
            "arista_eos" => Ok(Platform::AristaEos),
            "windows" => Ok(Platform::Windows),
            "juniper" => Ok(Platform::Juniper),
            "mikrotik" => Ok(Platform::Mikrotik),
            "aruba" => Ok(Platform::Aruba),
            "fortigate" => Ok(Platform::Fortigate),
            "fortimanager" => Ok(Platform::Fortimanager),
            "paloalto" => Ok(Platform::PaloAlto),
            "f5_bigip" => Ok(Platform::F5Bigip),
            "checkpoint_gaia" => Ok(Platform::CheckpointGaia),
            "hp_comware" => Ok(Platform::HpComware),
            "brocade_fos" => Ok(Platform::BrocadeFos),
            _ => Err(Error::InvalidPlatform(s.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Hint output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HintSource {
    History,
    Catalog {
        platform: Platform,
    },
    Snippet,
    /// A command the user usually runs right after the previous one (learned
    /// per platform). Surfaced/boosted by the sequence learner.
    Sequence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub text: String,
    pub source: HintSource,
    pub score: f32,
    /// User-facing label (only present for snippets).
    pub label: Option<String>,
}

// ---------------------------------------------------------------------------
// Security filter
// ---------------------------------------------------------------------------

const MAX_RECORD_LEN: usize = 200;

fn sensitive_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Matches things like "password: hunter2", "api_key=...", "token =",
        // "Bearer eyJ…". Conservative — we'd rather drop a legit command
        // than persist a credential.
        Regex::new(r"(?i)(password|passwd|secret|token|api[-_]?key|bearer)\s*[:=]")
            .expect("static regex compiles")
    })
}

/// Heuristic: should this command line be excluded from the history?
pub fn is_sensitive(command: &str) -> bool {
    if command.len() > MAX_RECORD_LEN {
        return true;
    }
    if sensitive_regex().is_match(command) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub struct HintEngine<'a> {
    db: &'a Database,
}

impl<'a> HintEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Persist a successful command line to the history. No-op for sensitive
    /// or empty inputs.
    pub fn record(&self, session_id: Option<i64>, command: &str) -> Result<(), Error> {
        let trimmed = command.trim();
        if trimmed.is_empty() || is_sensitive(trimmed) {
            return Ok(());
        }
        self.db.record_command(session_id, trimmed)?;
        Ok(())
    }

    /// Learn that `command` followed `prev` on `platform`. No-op for the
    /// generic platform (too noisy to be useful) or sensitive/empty lines.
    pub fn record_transition(
        &self,
        platform: Platform,
        prev: &str,
        command: &str,
    ) -> Result<(), Error> {
        let (prev, next) = (prev.trim(), command.trim());
        if platform == Platform::Generic
            || prev.is_empty()
            || next.is_empty()
            || prev == next
            || is_sensitive(prev)
            || is_sensitive(next)
        {
            return Ok(());
        }
        self.db.record_transition(platform.as_str(), prev, next)?;
        Ok(())
    }

    /// Return top-scoring suggestions for `prefix`. Combines all three
    /// sources, dedupes by exact command text (snippet > history > catalog),
    /// and trims to `limit` entries.
    pub fn suggest(
        &self,
        session_id: Option<i64>,
        platform: Platform,
        prefix: &str,
        limit: usize,
        last_command: Option<&str>,
    ) -> Result<Vec<Hint>, Error> {
        if prefix.is_empty() {
            return Ok(Vec::new());
        }
        let prefix_lower = prefix.to_ascii_lowercase();

        // --- 1. Snippets (highest priority) ---
        // Snippets are global and managed by the dedicated dialog; we pull
        // them from the same table so the popup and the dialog stay in sync.
        let mut candidates: Vec<Hint> = Vec::new();
        let snippets = self.db.list_snippet_contents_by_prefix(prefix, 32)?;
        for (name, content) in snippets {
            // Strip a trailing newline that the snippets-dialog seeds add
            // for one-click execution; the popup compares against typed text.
            let text = content.trim_end_matches('\n').to_string();
            candidates.push(Hint {
                text,
                source: HintSource::Snippet,
                score: 100.0 + (prefix.len() as f32),
                label: Some(name),
            });
        }

        // --- 2. History (frecency-scored) ---
        // Pull a generous slice; we will rescore in-memory.
        let history = self.db.history_by_prefix(session_id, prefix, 64)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for entry in history {
            let age_days = age_days(&entry.last_used, now);
            let recency = (-age_days / 14.0).exp(); // decay over ~2 weeks
            let frequency = (1.0 + entry.freq as f32).ln();
            let score = 50.0 + 20.0 * recency * frequency;
            candidates.push(Hint {
                text: entry.command,
                source: HintSource::History,
                score,
                label: None,
            });
        }

        // --- 3. Catalog (bundled + user overrides) ---
        let cat = catalog::for_platform(platform);
        for cmd in cat.iter() {
            if cmd.to_ascii_lowercase().starts_with(&prefix_lower) {
                candidates.push(Hint {
                    text: cmd.clone(),
                    source: HintSource::Catalog { platform },
                    score: 20.0 + (prefix.len() as f32) * 0.5,
                    label: None,
                });
            }
        }

        // --- 4. Sequence learning ---
        // Commands the user usually runs right after `last_command` get a
        // boost (if already a candidate) or are surfaced as their own source
        // (when they match the prefix but aren't otherwise in range). This is
        // what makes "after `conf t` → `interface …`" float to the top.
        if let Some(prev) = last_command.map(str::trim).filter(|p| !p.is_empty()) {
            if platform != Platform::Generic {
                for (next, freq) in self.db.next_commands_after(platform.as_str(), prev, 16)? {
                    if !next.to_ascii_lowercase().starts_with(&prefix_lower) {
                        continue;
                    }
                    let boost = 25.0 * (1.0 + freq as f32).ln();
                    match candidates.iter_mut().find(|h| h.text == next) {
                        Some(hit) => hit.score += boost,
                        None => candidates.push(Hint {
                            text: next,
                            source: HintSource::Sequence,
                            // Base above plain history (~50–70) so a learned
                            // continuation leads, but below snippets (100).
                            score: 70.0 + boost,
                            label: None,
                        }),
                    }
                }
            }
        }

        // --- Dedupe by text, keeping the highest-scoring source ---
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|h| seen.insert(h.text.clone()));
        candidates.truncate(limit);
        Ok(candidates)
    }
}

/// Parse an ISO-8601 / `datetime('now')` style SQLite timestamp into days
/// elapsed. Returns 0 on parse failure (so a malformed row simply doesn't
/// get a recency boost).
fn age_days(last_used: &str, now_secs: i64) -> f32 {
    // Try common SQLite shapes: "YYYY-MM-DD HH:MM:SS" or with 'T' separator.
    let normalized = last_used.replacen('T', " ", 1);
    let parts: Vec<&str> = normalized.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return 0.0;
    }
    let date = parts[0];
    let time = parts[1].split('.').next().unwrap_or("00:00:00");
    let date_parts: Vec<&str> = date.split('-').collect();
    let time_parts: Vec<&str> = time.split(':').collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return 0.0;
    }
    let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi), Ok(s)) = (
        date_parts[0].parse::<i32>(),
        date_parts[1].parse::<u32>(),
        date_parts[2].parse::<u32>(),
        time_parts[0].parse::<u32>(),
        time_parts[1].parse::<u32>(),
        time_parts[2].parse::<u32>(),
    ) else {
        return 0.0;
    };
    let last_secs =
        days_from_civil(y, mo, d) * 86_400 + (h as i64) * 3600 + (mi as i64) * 60 + s as i64;
    let elapsed = (now_secs - last_secs).max(0) as f32;
    elapsed / 86_400.0
}

/// Convert (year, month, day) into days since 1970-01-01 using Howard
/// Hinnant's algorithm. Avoids pulling in chrono in this crate.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * if m > 2 { m - 3 } else { m + 9 } + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_password() {
        assert!(is_sensitive("password: hunter2"));
        assert!(is_sensitive("PASSWORD=foo"));
        assert!(is_sensitive("api_key = abc"));
        assert!(is_sensitive("bearer: xyz"));
        assert!(is_sensitive("export SECRET_TOKEN=zzz"));
    }

    #[test]
    fn not_sensitive() {
        assert!(!is_sensitive("ls -la"));
        assert!(!is_sensitive("show running-config"));
        assert!(!is_sensitive("git commit -m 'fix password reset bug'"));
    }

    #[test]
    fn over_length() {
        let long = "a".repeat(MAX_RECORD_LEN + 1);
        assert!(is_sensitive(&long));
    }

    #[test]
    fn platform_roundtrip() {
        for p in catalog::all_platforms() {
            assert_eq!(Platform::from_str(p.as_str()).unwrap(), *p);
        }
    }

    #[test]
    fn catalogs_parse() {
        // Touch every embedded catalog at least once to make sure JSON is valid.
        for p in catalog::all_platforms() {
            let _ = catalog::for_platform(*p);
        }
    }
}
