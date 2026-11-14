//! Commands on the `snippets` table, which today stores **macros** (rows
//! with `steps_json`). Plain snippets — named text blocks with a Snippets
//! dialog, a 1–9 bar and per-platform defaults — were retired in 1.5.0: the
//! ones the user wrote are moved once to the ★ Custom command list (see
//! [`list_user_plain_snippets`]); the rows stay so backups and sync keep
//! working with older versions.

use serde::{Deserialize, Serialize};
use tauri::State;

use core_persistence::{Database, Snippet};

use crate::error::AppError;
use crate::state::AppState;

/// All snippets — used by the manage dialog so the user can edit / delete
/// regardless of which session is focused.
#[tauri::command]
pub async fn list_snippets(state: State<'_, AppState>) -> Result<Vec<Snippet>, AppError> {
    state
        .db
        .list_snippets()
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Plain snippets (no macro steps) the user wrote, leaving out the curated
/// defaults older versions seeded per platform. The frontend moves these to
/// the ★ Custom command list once, when it first starts without snippets.
#[tauri::command]
pub async fn list_user_plain_snippets(
    state: State<'_, AppState>,
) -> Result<Vec<Snippet>, AppError> {
    user_plain_snippets(&state.db).map_err(|e| AppError::Internal(e.to_string()))
}

fn user_plain_snippets(db: &Database) -> Result<Vec<Snippet>, core_persistence::Error> {
    Ok(db
        .list_snippets()?
        .into_iter()
        .filter(|s| s.steps_json.is_none())
        .filter(|s| !s.content.trim().is_empty())
        .filter(|s| !is_seeded_default(s.platform.as_deref(), &s.content))
        .collect())
}

#[tauri::command]
pub async fn create_snippet(
    state: State<'_, AppState>,
    name: String,
    content: String,
    platform: Option<String>,
    color: Option<String>,
) -> Result<i64, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Internal("snippet name is required".into()));
    }
    state
        .db
        .create_snippet(name.trim(), &content, platform.as_deref(), color.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn set_snippets_order(
    state: State<'_, AppState>,
    ordered_ids: Vec<i64>,
) -> Result<(), AppError> {
    state
        .db
        .set_snippets_order(&ordered_ids)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn update_snippet(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    content: String,
    platform: Option<String>,
    color: Option<String>,
) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Internal("snippet name is required".into()));
    }
    state
        .db
        .update_snippet(
            id,
            name.trim(),
            &content,
            platform.as_deref(),
            color.as_deref(),
        )
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn delete_snippet(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state
        .db
        .delete_snippet(id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Set (`Some`) or clear (`None`) a snippet's macro steps (expect/send/wait
/// JSON). When set, the snippet runs as a macro on the focused session instead
/// of sending its plain `content`.
#[tauri::command]
pub async fn set_snippet_steps(
    state: State<'_, AppState>,
    id: i64,
    steps_json: Option<String>,
) -> Result<(), AppError> {
    state
        .db
        .set_snippet_steps(id, steps_json.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Set (`Some`) or clear (`None`) a snippet's folder (the sidebar Macros
/// library groups by this). Separate from `update_snippet`, mirroring
/// [`set_snippet_steps`].
#[tauri::command]
pub async fn set_snippet_folder(
    state: State<'_, AppState>,
    id: i64,
    folder: Option<String>,
) -> Result<(), AppError> {
    state
        .db
        .set_snippet_folder(id, folder.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Set (or clear with `None` / blank) the keyboard shortcut that fires a
/// snippet or macro. Clash checks against app shortcuts and other snippets
/// happen in the editor, which knows the user's current bindings.
#[tauri::command]
pub async fn set_snippet_shortcut(
    state: State<'_, AppState>,
    id: i64,
    shortcut: Option<String>,
) -> Result<(), AppError> {
    state
        .db
        .set_snippet_shortcut(id, shortcut.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Persist a new per-folder ordering for macros (the sidebar Macros zone's
/// ▲/▼ reorder). `ordered_ids` lists the macro ids in their new order; each
/// id's `position` becomes its index.
#[tauri::command]
pub async fn set_snippet_positions(
    state: State<'_, AppState>,
    ordered_ids: Vec<i64>,
) -> Result<(), AppError> {
    state
        .db
        .set_snippet_positions(&ordered_ids)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Top recently-typed commands for the focused saved session. Drives the
/// "Recents" zone of the bottom button bar. Trailing newline is appended so
/// dispatching the snippet executes the command exactly like the original
/// keystroke did.
#[tauri::command]
pub async fn list_recent_command_snippets(
    state: State<'_, AppState>,
    saved_session_id: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<RecentCommand>, AppError> {
    let entries = state
        .db
        .recent_commands(saved_session_id, limit.unwrap_or(5))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(entries
        .into_iter()
        .map(|e| RecentCommand {
            text: e.command,
            freq: e.freq,
        })
        .collect())
}

/// Frontend payload for [`list_recent_command_snippets`]. Smaller than
/// `Snippet` because a recent doesn't have an id, sort order, etc.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecentCommand {
    pub text: String,
    pub freq: i64,
}

// ── Macro export / import (versioned JSON) ───────────────────────────────────

const MACRO_EXPORT_VERSION: u32 = 2;

/// A single macro step in the export envelope, mirroring
/// [`crate::login_script::LoginStep`]'s wire shape. Kept local (rather than
/// reusing `LoginStep`) so the on-disk export format is stable and independent
/// of the runner's internal type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedStep {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub expect: String,
    #[serde(default)]
    pub is_regex: bool,
    #[serde(default)]
    pub send: String,
    #[serde(default)]
    pub timeout_ms: u64,
}

/// A single macro in the export envelope. Only the macro-relevant fields are
/// carried; ids, ordering and timestamps are local and re-derived on import.
///
/// Both `steps` (v2, a real nested array) and `steps_json` (legacy v1, a raw
/// string) are accepted on import for backward compatibility; export always
/// writes `steps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMacro {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    /// v2: steps as a real nested JSON array.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<ExportedStep>,
    /// Legacy v1: doubly-encoded steps string. Only read on import; never
    /// written by current exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps_json: Option<String>,
}

/// The versioned macro export envelope written to / read from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroExportFile {
    /// Schema version — bump on breaking changes; import rejects newer files.
    pub zapx_macro_export: u32,
    pub macros: Vec<ExportedMacro>,
}

/// Summary returned to the frontend after importing a macro file.
#[derive(Debug, Default, Serialize)]
pub struct MacroImportSummary {
    /// Macros created (name didn't exist yet).
    pub added: usize,
    /// Macros whose steps/folder were updated in place (name already existed).
    pub updated: usize,
}

/// Snapshot the requested macros (snippets with a non-null `steps_json`) into a
/// versioned envelope. `ids = None` exports every macro. Pure DB → struct.
fn build_macro_export(db: &Database, ids: Option<&[i64]>) -> Result<MacroExportFile, AppError> {
    let all = db
        .list_snippets()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let macros = all
        .into_iter()
        .filter(|s| s.steps_json.is_some())
        .filter(|s| ids.map(|ids| ids.contains(&s.id)).unwrap_or(true))
        .map(|s| ExportedMacro {
            name: s.name,
            color: s.color,
            folder: s.folder,
            // Parse the stored steps string into a real array so the export is
            // human-readable; on parse error fall back to an empty vec.
            steps: s
                .steps_json
                .as_deref()
                .and_then(|j| serde_json::from_str::<Vec<ExportedStep>>(j).ok())
                .unwrap_or_default(),
            steps_json: None,
        })
        .collect();
    Ok(MacroExportFile {
        zapx_macro_export: MACRO_EXPORT_VERSION,
        macros,
    })
}

/// Apply exported macros. A macro whose name already exists is UPDATED in place
/// (its steps and folder are overwritten from the file) rather than skipped, so
/// re-importing an edited export actually applies the changes; a new name is
/// created. Returns added/updated counts.
fn apply_macro_import(
    db: &Database,
    file: &MacroExportFile,
) -> Result<MacroImportSummary, AppError> {
    if file.zapx_macro_export > MACRO_EXPORT_VERSION {
        return Err(AppError::Internal(format!(
            "file is from a newer macro export version (v{}, supported v{MACRO_EXPORT_VERSION})",
            file.zapx_macro_export
        )));
    }
    let internal = |e: core_persistence::Error| AppError::Internal(e.to_string());
    let existing = db.list_snippets().map_err(internal)?;
    let mut summary = MacroImportSummary::default();
    for m in &file.macros {
        // Prefer the v2 `steps` array (re-serialized to the DB's string form);
        // fall back to a legacy v1 `steps_json` string when no array is present.
        let steps_json = if !m.steps.is_empty() {
            serde_json::to_string(&m.steps).map_err(|e| AppError::Internal(e.to_string()))?
        } else {
            m.steps_json.clone().unwrap_or_else(|| "[]".to_string())
        };
        if let Some(ex) = existing.iter().find(|e| e.name == m.name) {
            // Update the existing macro's steps and folder in place.
            db.set_snippet_steps(ex.id, Some(&steps_json))
                .map_err(internal)?;
            db.set_snippet_folder(ex.id, m.folder.as_deref())
                .map_err(internal)?;
            summary.updated += 1;
        } else {
            let id = db
                .create_snippet(&m.name, "", None, m.color.as_deref())
                .map_err(internal)?;
            db.set_snippet_steps(id, Some(&steps_json))
                .map_err(internal)?;
            if let Some(folder) = &m.folder {
                db.set_snippet_folder(id, Some(folder)).map_err(internal)?;
            }
            summary.added += 1;
        }
    }
    Ok(summary)
}

/// Export macros to a JSON file. `ids = None` exports all macros.
#[tauri::command]
pub async fn export_macros(
    state: State<'_, AppState>,
    path: String,
    ids: Option<Vec<i64>>,
) -> Result<(), AppError> {
    let file = build_macro_export(&state.db, ids.as_deref())?;
    let json =
        serde_json::to_string_pretty(&file).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| AppError::Internal(format!("{path}: {e}")))
}

/// Import macros from a JSON file. Existing macros (matched by name) are updated
/// in place; new names are created.
#[tauri::command]
pub async fn import_macros(
    state: State<'_, AppState>,
    path: String,
) -> Result<MacroImportSummary, AppError> {
    let raw = std::fs::read_to_string(&path).map_err(|e| AppError::Internal(e.to_string()))?;
    let file: MacroExportFile = serde_json::from_str(&raw)
        .map_err(|e| AppError::Internal(format!("not a valid macro export: {e}")))?;
    apply_macro_import(&state.db, &file)
}

/// The five rows migration 007 created; migration 012 tagged them
/// `cisco_ios` (a user may have cleared the platform since).
const MIGRATION_007_SEEDS: [&str; 5] = [
    "show interface description\n",
    "show ip bgp summary\n",
    "show running-config\n",
    "configure terminal\n",
    "write memory\n",
];

/// Whether `content` is one of the default snippets older versions created
/// by themselves (as opposed to one the user wrote).
fn is_seeded_default(platform: Option<&str>, content: &str) -> bool {
    if matches!(platform, None | Some("cisco_ios")) && MIGRATION_007_SEEDS.contains(&content) {
        return true;
    }
    platform.is_some_and(|p| {
        default_snippet_batches(p)
            .iter()
            .flatten()
            .any(|(_, c)| *c == content)
    })
}

/// The default snippets older versions seeded per platform, as the batches
/// they shipped in. No longer created; kept only so
/// [`user_plain_snippets`] can tell them from the user's own.
fn default_snippet_batches(platform: &str) -> Vec<Vec<(&'static str, &'static str)>> {
    let mut batches = vec![default_snippets_for(platform)];
    // 0.24: more FortiGate / FortiManager operations. Every command was
    // checked against Fortinet's CLI reference / administration guides.
    // Commands that need arguments carry no trailing newline.
    match platform {
        "fortigate" => batches.push(vec![
            (
                "get system performance status",
                "get system performance status\n",
            ),
            ("get system ha status", "get system ha status\n"),
            ("get system interface", "get system interface\n"),
            ("get system arp", "get system arp\n"),
            (
                "get router info bgp summary",
                "get router info bgp summary\n",
            ),
            ("diagnose vpn tunnel list", "diagnose vpn tunnel list\n"),
            (
                "diagnose vpn ike gateway list",
                "diagnose vpn ike gateway list\n",
            ),
            ("diagnose sys session stat", "diagnose sys session stat\n"),
            ("diagnose sys top", "diagnose sys top\n"),
            (
                "diagnose sys sdwan health-check",
                "diagnose sys sdwan health-check\n",
            ),
            (
                "get system fortiguard-service status",
                "get system fortiguard-service status\n",
            ),
            (
                "execute revision list config",
                "execute revision list config\n",
            ),
            (
                "Stop debug (disable + reset)",
                "diagnose debug disable\ndiagnose debug reset\n",
            ),
        ]),
        "fortimanager" => batches.push(vec![
            ("get system ha", "get system ha\n"),
            ("diagnose dvm adom list", "diagnose dvm adom list\n"),
            (
                "diagnose dvm check-integrity",
                "diagnose dvm check-integrity\n",
            ),
            // Can offer to fix the DB after a y/n prompt — left for the user
            // to confirm with Enter.
            (
                "diagnose cdb check policy-packages",
                "diagnose cdb check policy-packages",
            ),
            (
                "diagnose fmupdate view-serverlist fds",
                "diagnose fmupdate view-serverlist fds\n",
            ),
            ("diagnose dvm task list ", "diagnose dvm task list "),
        ]),
        _ => {}
    }
    batches.retain(|b| !b.is_empty());
    batches
}

/// The original (batch 1) curated set — what most users would benefit from on
/// day one. Returns `(name, content_with_trailing_newline)` pairs; empty for
/// platforms without curated defaults.
fn default_snippets_for(platform: &str) -> Vec<(&'static str, &'static str)> {
    fn nl(commands: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
        // The newline is appended by callers via `&'static str` interpolation;
        // we can't do that at runtime for &'static, so the curated tables
        // below carry the `\n` already.
        commands.to_vec()
    }
    match platform {
        "linux" => nl(&[
            ("ls -la", "ls -la\n"),
            ("df -h", "df -h\n"),
            ("free -h", "free -h\n"),
            ("ps aux | grep ", "ps aux | grep "),
            ("systemctl status ", "systemctl status "),
            ("journalctl -xe", "journalctl -xe\n"),
        ]),
        "cisco_ios" => nl(&[
            ("show ip int brief", "show ip int brief\n"),
            ("show ip route", "show ip route\n"),
            ("show interfaces status", "show interfaces status\n"),
            ("show running-config", "show running-config\n"),
            ("clear counters", "clear counters\n"),
            ("wr mem", "wr mem\n"),
        ]),
        "juniper" => nl(&[
            ("show interfaces terse", "show interfaces terse\n"),
            ("show route", "show route\n"),
            ("show bgp summary", "show bgp summary\n"),
            ("show chassis hardware", "show chassis hardware\n"),
            ("commit confirmed", "commit confirmed\n"),
        ]),
        "mikrotik" => nl(&[
            ("/interface print", "/interface print\n"),
            ("/ip address print", "/ip address print\n"),
            ("/ip route print", "/ip route print\n"),
            ("/log print", "/log print\n"),
        ]),
        "aruba" => nl(&[
            ("show vlan", "show vlan\n"),
            ("show ip route", "show ip route\n"),
            ("show interfaces brief", "show interfaces brief\n"),
        ]),
        "fortigate" => nl(&[
            ("get system status", "get system status\n"),
            (
                "get router info routing-table all",
                "get router info routing-table all\n",
            ),
            (
                "get vpn ipsec tunnel summary",
                "get vpn ipsec tunnel summary\n",
            ),
            (
                "diagnose sniffer packet any 'icmp' 4",
                "diagnose sniffer packet any 'icmp' 4\n",
            ),
            (
                "execute backup config flash",
                "execute backup config flash\n",
            ),
        ]),
        "fortimanager" => nl(&[
            ("get system status", "get system status\n"),
            ("diagnose dvm device list", "diagnose dvm device list\n"),
            (
                "execute backup all-settings ftp",
                "execute backup all-settings ftp ",
            ),
        ]),
        "paloalto" => nl(&[
            ("show system info", "show system info\n"),
            ("show session info", "show session info\n"),
            (
                "show running resource-monitor minute",
                "show running resource-monitor minute\n",
            ),
            ("show routing route", "show routing route\n"),
            ("commit", "commit\n"),
        ]),
        "f5_bigip" => nl(&[
            ("tmsh show sys version", "tmsh show sys version\n"),
            ("tmsh show ltm virtual", "tmsh show ltm virtual\n"),
            ("tmsh show net interface", "tmsh show net interface\n"),
            ("tmsh save sys config", "tmsh save sys config\n"),
        ]),
        "checkpoint_gaia" => nl(&[
            ("show configuration", "show configuration\n"),
            ("cphaprob state", "cphaprob state\n"),
            ("fw stat", "fw stat\n"),
            ("save config", "save config\n"),
        ]),
        "hp_comware" => nl(&[
            (
                "display current-configuration",
                "display current-configuration\n",
            ),
            ("display interface brief", "display interface brief\n"),
            ("display ip routing-table", "display ip routing-table\n"),
            ("save force", "save force\n"),
        ]),
        "brocade_fos" => nl(&[
            ("switchshow", "switchshow\n"),
            ("cfgshow", "cfgshow\n"),
            ("nsshow", "nsshow\n"),
            ("portshow", "portshow\n"),
        ]),
        _ => Vec::new(),
    }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "zapterm-macro-export-test-{tag}-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        (Database::open(&path).expect("open temp db"), path)
    }

    /// The user's own plain snippets are listed for the move to ★ Custom;
    /// macros, empty rows and the defaults older versions seeded are not.
    #[test]
    fn user_plain_snippets_leave_out_macros_and_seeded_defaults() {
        let (db, path) = temp_db("user-plain");
        // What older versions seeded for FortiGate, both batches.
        for (name, content) in default_snippet_batches("fortigate").iter().flatten() {
            db.create_snippet(name, content, Some("fortigate"), None)
                .unwrap();
        }
        let mine = db
            .create_snippet(
                "vpn",
                "diagnose vpn tunnel list name {{tunnel}}\n",
                Some("fortigate"),
                None,
            )
            .unwrap();
        // Same text as a FortiGate default, but global: the user wrote it.
        db.create_snippet("status", "get system status\n", None, None)
            .unwrap();
        // A migration-007 seed whose platform the user cleared: still a seed.
        db.create_snippet("wr", "write memory\n", None, None)
            .unwrap();
        db.create_snippet("empty", "  ", None, None).unwrap();
        let m = db.create_snippet("login", "", None, None).unwrap();
        db.set_snippet_steps(m, Some(r#"[{"kind":"send","send":"a\r"}]"#))
            .unwrap();

        let got = user_plain_snippets(&db).unwrap();
        let names: Vec<&str> = got.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(got.iter().any(|s| s.id == mine));
        assert!(names.contains(&"status"));
        assert!(is_seeded_default(Some("linux"), "df -h\n"));
        assert!(!is_seeded_default(Some("generic"), "df -h\n"));
        assert!(!is_seeded_default(None, "df -h\n"));
        let _ = std::fs::remove_file(path);
    }

    /// Macro export envelope round-trips: a macro survives export → import into a
    /// fresh DB (name, color, folder, steps), and a second import is a no-op.
    #[test]
    fn macro_export_roundtrip() {
        let (src, src_path) = temp_db("src");
        let id = src.create_snippet("login", "", None, Some("#0af")).unwrap();
        src.set_snippet_steps(id, Some(r#"[{"kind":"send","send":"a\r"}]"#))
            .unwrap();
        src.set_snippet_folder(id, Some("Net")).unwrap();
        // A plain snippet (no steps) must NOT be exported as a macro.
        src.create_snippet("plain", "ls\n", None, None).unwrap();

        let file = build_macro_export(&src, None).unwrap();
        assert_eq!(file.zapx_macro_export, MACRO_EXPORT_VERSION);
        assert_eq!(file.macros.len(), 1);
        assert_eq!(file.macros[0].name, "login");
        assert_eq!(file.macros[0].color.as_deref(), Some("#0af"));
        assert_eq!(file.macros[0].folder.as_deref(), Some("Net"));
        // Steps export as a real nested array (v2), not a doubly-encoded string.
        assert_eq!(file.macros[0].steps.len(), 1);
        assert_eq!(file.macros[0].steps[0].kind, "send");
        assert_eq!(file.macros[0].steps[0].send, "a\r");
        assert!(file.macros[0].steps_json.is_none());

        // Serialize + parse back, exercising the on-disk shape.
        let json = serde_json::to_string(&file).unwrap();
        let parsed: MacroExportFile = serde_json::from_str(&json).unwrap();

        let (dst, dst_path) = temp_db("dst");
        let s1 = apply_macro_import(&dst, &parsed).unwrap();
        assert_eq!(s1.added, 1);
        assert_eq!(s1.updated, 0);
        let imported = dst.list_snippets().unwrap();
        let m = imported.iter().find(|s| s.name == "login").unwrap();
        assert_eq!(m.color.as_deref(), Some("#0af"));
        assert_eq!(m.folder.as_deref(), Some("Net"));
        // Imported steps parse back to the same array (fields are normalized to
        // the full step shape on the round-trip through the v2 array form).
        let steps: Vec<ExportedStep> =
            serde_json::from_str(m.steps_json.as_deref().unwrap()).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, "send");
        assert_eq!(steps[0].send, "a\r");

        // Re-import updates the existing macro in place (by name), not skipped.
        let s2 = apply_macro_import(&dst, &parsed).unwrap();
        assert_eq!(s2.added, 0);
        assert_eq!(s2.updated, 1);
        // Still a single macro (updated, not duplicated).
        assert_eq!(
            dst.list_snippets()
                .unwrap()
                .iter()
                .filter(|s| s.name == "login")
                .count(),
            1
        );

        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(dst_path);
    }

    /// Import accepts BOTH formats: a v2 file with a `steps` array, and a legacy
    /// v1 file with a `steps_json` string. A regex step whose `expect` contains a
    /// backslash must survive export → import intact.
    #[test]
    fn macro_import_v2_and_legacy_v1() {
        // v2 file: steps as a real nested array, including a regex step whose
        // `expect` carries a raw backslash escape.
        let v2 = MacroExportFile {
            zapx_macro_export: 2,
            macros: vec![ExportedMacro {
                name: "v2mac".into(),
                color: None,
                folder: Some("Telefonica".into()),
                steps: vec![
                    ExportedStep {
                        kind: "wait".into(),
                        expect: String::new(),
                        is_regex: false,
                        send: String::new(),
                        timeout_ms: 1000,
                    },
                    ExportedStep {
                        kind: "expect".into(),
                        expect: r"password:\s*$".into(),
                        is_regex: true,
                        send: String::new(),
                        timeout_ms: 10000,
                    },
                ],
                steps_json: None,
            }],
        };
        // Round-trip through the on-disk (pretty) shape.
        let json = serde_json::to_string_pretty(&v2).unwrap();
        // The array must be human-readable: no doubly-encoded quotes.
        assert!(json.contains("\"steps\""));
        assert!(!json.contains("steps_json"));
        let parsed: MacroExportFile = serde_json::from_str(&json).unwrap();

        let (dst, dst_path) = temp_db("v2");
        let s = apply_macro_import(&dst, &parsed).unwrap();
        assert_eq!(s.added, 1);
        let m = dst
            .list_snippets()
            .unwrap()
            .into_iter()
            .find(|s| s.name == "v2mac")
            .unwrap();
        assert_eq!(m.folder.as_deref(), Some("Telefonica"));
        let steps: Vec<ExportedStep> =
            serde_json::from_str(m.steps_json.as_deref().unwrap()).unwrap();
        assert_eq!(steps.len(), 2);
        // Regex step survived intact: backslash and flag preserved.
        assert!(steps[1].is_regex);
        assert_eq!(steps[1].expect, r"password:\s*$");
        let _ = std::fs::remove_file(dst_path);

        // Legacy v1 file: steps carried as a raw JSON string.
        let v1_raw = r#"{"zapx_macro_export":1,"macros":[{"name":"v1mac","color":null,"folder":null,"steps_json":"[{\"kind\":\"expect\",\"expect\":\"password:\\\\s*$\",\"is_regex\":true,\"send\":\"\",\"timeout_ms\":5000}]"}]}"#;
        let v1: MacroExportFile = serde_json::from_str(v1_raw).unwrap();
        let (dst2, dst2_path) = temp_db("v1");
        let s2 = apply_macro_import(&dst2, &v1).unwrap();
        assert_eq!(s2.added, 1);
        let m2 = dst2
            .list_snippets()
            .unwrap()
            .into_iter()
            .find(|s| s.name == "v1mac")
            .unwrap();
        let steps2: Vec<ExportedStep> =
            serde_json::from_str(m2.steps_json.as_deref().unwrap()).unwrap();
        assert_eq!(steps2.len(), 1);
        assert!(steps2[0].is_regex);
        assert_eq!(steps2[0].expect, r"password:\s*$");
        let _ = std::fs::remove_file(dst2_path);
    }

    /// A newer-schema file is rejected.
    #[test]
    fn macro_import_rejects_newer_version() {
        let (db, path) = temp_db("ver");
        let file = MacroExportFile {
            zapx_macro_export: MACRO_EXPORT_VERSION + 1,
            macros: vec![],
        };
        assert!(apply_macro_import(&db, &file).is_err());
        let _ = std::fs::remove_file(path);
    }
}
