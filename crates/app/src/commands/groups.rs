use tauri::State;

use core_persistence::BroadcastGroup;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_broadcast_groups(
    state: State<'_, AppState>,
) -> Result<Vec<BroadcastGroup>, AppError> {
    state
        .db
        .list_broadcast_groups()
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn create_broadcast_group(
    state: State<'_, AppState>,
    name: String,
) -> Result<i64, AppError> {
    state
        .db
        .create_broadcast_group(&name)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn rename_broadcast_group(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), AppError> {
    state
        .db
        .rename_broadcast_group(id, &name)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn delete_broadcast_group(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state
        .db
        .delete_broadcast_group(id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn set_broadcast_group_members(
    state: State<'_, AppState>,
    group_id: i64,
    session_ids: Vec<i64>,
) -> Result<(), AppError> {
    state
        .db
        .set_broadcast_group_members(group_id, &session_ids)
        .map_err(|e| AppError::Internal(e.to_string()))
}
