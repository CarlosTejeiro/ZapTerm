//! Tauri commands for editing per-session output triggers.
//!
//! Triggers are stored as a JSON array on `sessions.triggers_json`
//! (migration 017). On `open_saved_session` the JSON is parsed and a
//! [`TriggerRunner`](crate::triggers::TriggerRunner) is attached to the live
//! session.

use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use crate::triggers::Trigger;

/// Return the triggers attached to a saved session (empty if none).
#[tauri::command]
pub async fn get_session_triggers(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<Vec<Trigger>, AppError> {
    let json = state
        .db
        .get_session_triggers(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let triggers: Vec<Trigger> = match json {
        Some(j) if !j.trim().is_empty() => serde_json::from_str(&j)
            .map_err(|e| AppError::Internal(format!("invalid triggers_json: {e}")))?,
        _ => Vec::new(),
    };
    Ok(triggers)
}

/// Replace (or clear) the triggers for a saved session.
#[tauri::command]
pub async fn set_session_triggers(
    state: State<'_, AppState>,
    saved_session_id: i64,
    triggers: Vec<Trigger>,
) -> Result<(), AppError> {
    let json_opt = if triggers.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&triggers)
                .map_err(|e| AppError::Internal(format!("serialise triggers: {e}")))?,
        )
    };
    state
        .db
        .set_session_triggers(saved_session_id, json_opt.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}
