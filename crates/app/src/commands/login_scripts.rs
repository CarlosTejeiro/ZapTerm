//! Tauri commands for editing per-session login automation scripts.
//!
//! The script is stored as a JSON array on `sessions.login_script_json`
//! (migration 008). On `open_saved_session` the JSON is parsed and a
//! [`LoginRunner`](crate::login_script::LoginRunner) is attached to the live
//! session.

use tauri::State;

use crate::error::AppError;
use crate::login_script::LoginStep;
use crate::state::AppState;

/// Return the login script (a JSON-decoded `Vec<LoginStep>`) attached to a
/// saved session, or an empty list if none is set.
#[tauri::command]
pub async fn get_login_script(
    state: State<'_, AppState>,
    saved_session_id: i64,
) -> Result<Vec<LoginStep>, AppError> {
    let json = state
        .db
        .get_login_script(saved_session_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let steps: Vec<LoginStep> = match json {
        Some(j) if !j.trim().is_empty() => serde_json::from_str(&j)
            .map_err(|e| AppError::Internal(format!("invalid login_script_json: {e}")))?,
        _ => Vec::new(),
    };
    Ok(steps)
}

/// Replace (or clear) the login script for a saved session.
#[tauri::command]
pub async fn set_login_script(
    state: State<'_, AppState>,
    saved_session_id: i64,
    steps: Vec<LoginStep>,
) -> Result<(), AppError> {
    let json_opt = if steps.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&steps)
                .map_err(|e| AppError::Internal(format!("serialise login script: {e}")))?,
        )
    };
    state
        .db
        .set_login_script(saved_session_id, json_opt.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}
