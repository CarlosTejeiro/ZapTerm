//! Tauri commands for the command-hint engine.
//!
//! Snippet CRUD lives in [`crate::commands::snippets`] — the hint engine just
//! reads from the same table so the dialog and the popup stay in sync.

use std::str::FromStr;

use tauri::State;

use core_hints::{Hint, HintEngine, Platform};

use crate::error::AppError;
use crate::state::AppState;

fn platform_from_opt(s: Option<&str>) -> Platform {
    s.and_then(|raw| Platform::from_str(raw).ok())
        .unwrap_or(Platform::Generic)
}

/// Resolve the platform for a saved session: explicit override beats the
/// global default in `settings`.
fn resolve_platform(state: &AppState, saved_session_id: Option<i64>) -> Platform {
    if let Some(id) = saved_session_id {
        if let Ok(Some(p)) = state.db.get_session_platform(id) {
            return platform_from_opt(Some(p.as_str()));
        }
    }
    let default = state
        .db
        .get_setting("hints.default_platform")
        .ok()
        .flatten();
    platform_from_opt(default.as_deref())
}

#[tauri::command]
pub async fn get_hints(
    state: State<'_, AppState>,
    saved_session_id: Option<i64>,
    prefix: String,
    limit: Option<usize>,
    // The command the user most recently ran in this session — boosts its
    // usual continuations (sequence learning).
    last_command: Option<String>,
) -> Result<Vec<Hint>, AppError> {
    let platform = resolve_platform(&state, saved_session_id);
    let engine = HintEngine::new(&state.db);
    engine
        .suggest(
            saved_session_id,
            platform,
            &prefix,
            limit.unwrap_or(5),
            last_command.as_deref(),
        )
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Reverse-i-search (Ctrl+R) over the command history. Substring match, scoped
/// to this session (plus session-less entries), most-recent first. An empty
/// `query` returns the most recent commands.
#[tauri::command]
pub async fn search_command_history(
    state: State<'_, AppState>,
    saved_session_id: Option<i64>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<String>, AppError> {
    state
        .db
        .search_history(saved_session_id, &query, limit.unwrap_or(50))
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Stateless prompt-return check for the multi-host command runner. Given a
/// platform string and a tail of captured output, returns whether that output
/// ends at the device's idle prompt (i.e. the command finished). Unknown /
/// unparseable platforms resolve to `Generic`, which never matches — the
/// runner then falls back to its time-window capture.
#[tauri::command]
pub async fn prompt_returned(platform: String, tail: String) -> Result<bool, AppError> {
    let p = Platform::from_str(&platform).unwrap_or(Platform::Generic);
    Ok(core_hints::detector::ends_with_prompt(p, &tail))
}

#[tauri::command]
pub async fn record_command(
    state: State<'_, AppState>,
    saved_session_id: Option<i64>,
    command: String,
    // The previously-executed command in this session, if any — recorded as
    // a `prev → command` transition for sequence learning.
    prev_command: Option<String>,
) -> Result<(), AppError> {
    let engine = HintEngine::new(&state.db);
    engine
        .record(saved_session_id, &command)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if let Some(prev) = prev_command.as_deref() {
        let platform = resolve_platform(&state, saved_session_id);
        // Best-effort: a transition that fails to record shouldn't fail the
        // history write the user actually cares about.
        let _ = engine.record_transition(platform, prev, &command);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_session_platform(
    state: State<'_, AppState>,
    saved_session_id: i64,
    platform: String,
) -> Result<(), AppError> {
    Platform::from_str(&platform).map_err(|e| AppError::Internal(e.to_string()))?;
    state
        .db
        .set_session_platform(saved_session_id, &platform)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Drop the session's platform (detected or chosen), so it follows the global
/// default again — the only way to undo a wrong auto-detection.
#[tauri::command]
pub async fn clear_session_platform(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<(), AppError> {
    state
        .db
        .clear_session_platform(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn get_session_platform(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<Option<String>, AppError> {
    state
        .db
        .get_session_platform(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn list_platforms() -> Result<Vec<PlatformInfo>, AppError> {
    Ok(core_hints::catalog::all_platforms()
        .iter()
        .map(|p| PlatformInfo {
            id: p.as_str().to_string(),
            name: p.display_name().to_string(),
        })
        .collect())
}

#[tauri::command]
pub async fn clear_command_history(
    state: State<'_, AppState>,
    saved_session_id: Option<i64>,
) -> Result<(), AppError> {
    state
        .db
        .clear_history(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PlatformInfo {
    pub id: String,
    pub name: String,
}

/// Re-scan the user-override directory and apply any `<platform>.json`
/// files. Returns the number of platforms whose catalog was replaced.
#[tauri::command]
pub async fn reload_hint_catalogs() -> Result<usize, AppError> {
    core_hints::catalog::reload().map_err(|e| AppError::Internal(e.to_string()))
}

/// Open the catalog-override directory in the host file explorer and return
/// its absolute path. Creates the directory and seeds it with starter files
/// (one bundled catalog per platform) on first invocation so the user has
/// something concrete to edit instead of an empty folder.
#[tauri::command]
pub async fn hint_catalogs_dir() -> Result<Option<String>, AppError> {
    let Some(dir) = core_hints::catalog::user_dir() else {
        return Ok(None);
    };
    let created = !dir.exists();
    if created {
        std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.to_string()))?;
        seed_starter_catalogs(&dir);
    }
    // Reveal in the host file explorer — best-effort.
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
        tracing::warn!("hint_catalogs_dir spawn failed: {e}");
    }
    Ok(Some(dir.to_string_lossy().into_owned()))
}

/// On first visit, write a few `*.example.json` files into the user dir so
/// the format is self-documenting. Files ending in `.example.json` are NOT
/// picked up by [`core_hints::catalog::reload`] — the user has to drop the
/// `.example` suffix to activate one.
fn seed_starter_catalogs(dir: &std::path::Path) {
    use core_hints::Platform;
    for p in [
        Platform::Fortigate,
        Platform::Fortimanager,
        Platform::Fortianalyzer,
        Platform::CiscoIos,
        Platform::CiscoIosXr,
        Platform::AristaEos,
        Platform::Windows,
        Platform::Linux,
    ] {
        let dst = dir.join(format!("{}.example.json", p.as_str()));
        let bundled = core_hints::catalog::for_platform(p);
        if dst.exists() {
            continue;
        }
        if let Ok(json) = serde_json::to_string_pretty(&*bundled) {
            let _ = std::fs::write(&dst, json);
        }
    }
    // Drop a tiny README too so the format is obvious without docs.
    let readme = dir.join("README.md");
    if !readme.exists() {
        let body = "# ZapTerm · hint catalogs\n\n\
            Drop `<platform>.json` files in this folder to override the bundled\n\
            command catalog for that platform. Each file must be a JSON array\n\
            of strings, ordered however you like (the engine sorts by prefix\n\
            match, not file order).\n\n\
            Valid filenames: `linux.json`, `cisco_ios.json`, `cisco_iosxr.json`,\n\
            `arista_eos.json`, `windows.json`, `juniper.json`, `mikrotik.json`, `aruba.json`, `fortigate.json`,\n\
            `fortimanager.json`, `fortianalyzer.json`.\n\n\
            The `*.example.json` files are starter copies of the bundled\n\
            catalogs — rename them (drop `.example`) to activate. Use the\n\
            \"Reload catalogs\" button in Settings → Hints to re-scan without\n\
            restarting the app.\n";
        let _ = std::fs::write(&readme, body);
    }
}
