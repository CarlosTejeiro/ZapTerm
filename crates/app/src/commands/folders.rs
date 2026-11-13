use tauri::State;

use core_persistence::Folder;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, AppError> {
    state
        .db
        .list_folders()
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<i64>,
) -> Result<i64, AppError> {
    state
        .db
        .create_folder(&name, parent_id)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn rename_folder(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), AppError> {
    state
        .db
        .rename_folder(id, &name)
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state
        .db
        .delete_folder(id)
        .map_err(|e| AppError::Internal(e.to_string()))
}
