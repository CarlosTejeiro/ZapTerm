//! Tauri commands for the "Data folder" setting: inspect where ZapTerm keeps
//! its data, migrate it to a user-chosen folder, or revert to the default.
//! Changing the folder writes the pointer file read at startup by
//! [`crate::data_dir::resolve`]; the new location takes effect after a
//! restart (the running process keeps its open DB/loggers untouched).

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::Manager;

use crate::data_dir::{self, Source};
use crate::{AppError, AppState};

#[derive(Debug, Serialize)]
pub struct DataDirInfo {
    /// Directory currently in use by this process.
    pub path: String,
    /// How it was selected: `cli-flag` | `env-var` | `portable` | `pointer` | `default`.
    pub source: Source,
    pub portable: bool,
    /// The platform default, for the "use default" affordance in Settings.
    pub default_path: String,
    /// Whether a custom dir can be configured from the UI. False when the
    /// location is forced by CLI flag / env var / portable marker — the
    /// pointer file would be shadowed by those higher-priority sources.
    pub changeable: bool,
}

#[tauri::command]
pub async fn get_data_dir_info(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<DataDirInfo, AppError> {
    let default_path = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let source = state.data_dir.source;
    Ok(DataDirInfo {
        path: state.data_dir.dir.to_string_lossy().into_owned(),
        source,
        portable: state.data_dir.is_portable(),
        default_path: default_path.to_string_lossy().into_owned(),
        changeable: matches!(source, Source::Pointer | Source::Default),
    })
}

/// Migrate the data to `new_dir` and persist the pointer file.
///
/// - If `new_dir` already holds a `zapterm.db`, it is adopted as-is (no copy) —
///   this is how a second machine attaches to a synced folder.
/// - Otherwise the live DB is snapshotted there (`VACUUM INTO`) — the vault
///   and its secrets are inside it — and `session_logs/` + `catalogs/` are
///   copied best-effort.
///
/// Returns a human-readable summary. The caller should prompt to restart.
#[tauri::command]
pub async fn set_data_dir(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_dir: String,
) -> Result<String, AppError> {
    let new_dir = PathBuf::from(new_dir.trim());
    if !new_dir.is_absolute() {
        return Err(AppError::Internal("the path must be absolute".into()));
    }
    if new_dir == state.data_dir.dir {
        return Err(AppError::Internal(
            "that is already the current data folder".into(),
        ));
    }
    if new_dir.starts_with(&state.data_dir.dir) {
        return Err(AppError::Internal(
            "the new folder can't be inside the current one".into(),
        ));
    }
    std::fs::create_dir_all(&new_dir).map_err(|e| AppError::Internal(e.to_string()))?;

    let new_db = new_dir.join("zapterm.db");
    let summary = if new_db.exists() {
        "That folder already holds ZapTerm data: using it as it is (nothing was copied).".to_owned()
    } else {
        // 1. Clean DB snapshot (WAL-safe).
        state
            .db
            .vacuum_into(&new_db)
            .map_err(|e| AppError::Internal(format!("copying the database failed: {e}")))?;

        // 2. Secrets are in the database copy, encrypted under the vault
        //    key (itself wrapped under the master password), so they need no
        //    re-encryption. A `vault.key` an older version left here travels
        //    too while it is still needed to read copies not yet moved into
        //    the vault.
        let old_key = state.data_dir.dir.join("vault.key");
        if old_key.is_file() && !new_dir.join("vault.key").exists() {
            std::fs::copy(&old_key, new_dir.join("vault.key"))
                .map_err(|e| AppError::Internal(format!("copying vault.key failed: {e}")))?;
        }

        // 3. Logs + catalogs travel too (best-effort; sizes can be large).
        let logs = copy_dir_recursive(
            &state.data_dir.dir.join("session_logs"),
            &new_dir.join("session_logs"),
        );
        let cats = copy_dir_recursive(
            &state.data_dir.dir.join("catalogs"),
            &new_dir.join("catalogs"),
        );

        format!(
            "Data moved: database copied (saved passwords included), {logs} logs and {cats} catalogs copied."
        )
    };

    let pointer = data_dir::pointer_path(&app)
        .ok_or_else(|| AppError::Internal("could not find the configuration folder".into()))?;
    if let Some(parent) = pointer.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Internal(e.to_string()))?;
    }
    std::fs::write(&pointer, new_dir.to_string_lossy().as_bytes())
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(summary)
}

/// Remove the pointer file so the next launch uses the platform default.
/// Existing data is left where it is.
#[tauri::command]
pub async fn reset_data_dir(app: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(pointer) = data_dir::pointer_path(&app) {
        if pointer.exists() {
            std::fs::remove_file(pointer).map_err(|e| AppError::Internal(e.to_string()))?;
        }
    }
    Ok(())
}

/// Relaunch the app so a new data dir takes effect.
#[tauri::command]
pub async fn restart_app(app: tauri::AppHandle) -> Result<(), AppError> {
    app.restart();
}

/// Copy `src` into `dest` recursively, returning the number of files copied.
/// Missing `src` is fine (0). Individual file failures are logged, not fatal.
fn copy_dir_recursive(src: &Path, dest: &Path) -> usize {
    if !src.is_dir() {
        return 0;
    }
    let mut copied = 0;
    if std::fs::create_dir_all(dest).is_err() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(src) else {
        return 0;
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copied += copy_dir_recursive(&from, &to);
        } else if let Err(e) = std::fs::copy(&from, &to) {
            tracing::warn!("copy {} failed: {e}", from.display());
        } else {
            copied += 1;
        }
    }
    copied
}
