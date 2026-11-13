use std::collections::HashMap;

use tauri::State;

use core_persistence::ColorScheme;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, AppError> {
    state
        .db
        .get_all_settings()
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppError> {
    state
        .db
        .get_setting(&key)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    state
        .db
        .set_setting(&key, &value)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Settings key for the SSH keepalive interval (seconds; "0" disables).
pub const SSH_KEEPALIVE_KEY: &str = "ssh.keepalive_secs";
/// Default keepalive when the setting is absent — sends a server-alive probe
/// every 60s so idle sessions aren't dropped by firewalls/servers.
pub const SSH_KEEPALIVE_DEFAULT: u64 = 60;

/// Persist the SSH keepalive interval AND apply it live (affects sessions
/// opened afterwards). Clamped to a sane range.
#[tauri::command]
pub async fn set_ssh_keepalive(state: State<'_, AppState>, secs: u64) -> Result<(), AppError> {
    let secs = secs.min(3600);
    state
        .db
        .set_setting(SSH_KEEPALIVE_KEY, &secs.to_string())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    core_transport::set_keepalive_secs(secs);
    Ok(())
}

/// Read the persisted keepalive interval (falling back to the default) and push
/// it into core-transport. Called once at startup.
pub fn apply_persisted_keepalive(db: &core_persistence::Database) {
    let secs = db
        .get_setting(SSH_KEEPALIVE_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(SSH_KEEPALIVE_DEFAULT);
    core_transport::set_keepalive_secs(secs);
}

#[tauri::command]
pub async fn list_color_schemes(state: State<'_, AppState>) -> Result<Vec<ColorScheme>, AppError> {
    state
        .db
        .list_color_schemes()
        .map_err(|e| AppError::Internal(e.to_string()))
}
