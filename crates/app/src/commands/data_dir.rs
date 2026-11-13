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
/// - Otherwise the live DB is snapshotted there (`VACUUM INTO`), the
///   `session_secrets` fallback blobs are re-encrypted for the new seed
///   (the seed derives from the dir path), and `session_logs/` +
///   `catalogs/` are copied best-effort.
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
        return Err(AppError::Internal("la ruta debe ser absoluta".into()));
    }
    if new_dir == state.data_dir.dir {
        return Err(AppError::Internal(
            "esa ya es la carpeta de datos actual".into(),
        ));
    }
    if new_dir.starts_with(&state.data_dir.dir) {
        return Err(AppError::Internal(
            "la carpeta destino no puede estar dentro de la actual".into(),
        ));
    }
    std::fs::create_dir_all(&new_dir).map_err(|e| AppError::Internal(e.to_string()))?;

    let new_db = new_dir.join("zapterm.db");
    let summary = if new_db.exists() {
        "Carpeta con datos ZapTerm existentes adoptada (no se copió nada).".to_owned()
    } else {
        // 1. Clean DB snapshot (WAL-safe).
        state
            .db
            .vacuum_into(&new_db)
            .map_err(|e| AppError::Internal(format!("copia de la DB falló: {e}")))?;

        // 2. Re-encrypt the SQLite fallback secrets for the new seed. The
        //    OS keyring (primary store) is untouched. Blobs that fail to
        //    decrypt (foreign DB, older seed) are dropped — the keyring or
        //    a fresh prompt covers those sessions. The seed is the new dir's
        //    own random `vault.key` (created here), not the dir path.
        let new_seed = core_vault::load_or_create_seed(&new_dir)
            .map_err(|e| AppError::Internal(format!("vault keyfile en destino: {e}")))?;
        let migrated_db = core_persistence::Database::open(&new_db)
            .map_err(|e| AppError::Internal(format!("DB migrada no abre: {e}")))?;
        let mut reencrypted = 0usize;
        for session in state.db.list_sessions().unwrap_or_default() {
            let Ok(Some(blob)) = state.db.get_session_secret(session.id) else {
                continue;
            };
            match core_vault::decrypt_with_seed(&state.vault_seed, &blob) {
                Ok(secret) => {
                    if let Ok(new_blob) = core_vault::encrypt_with_seed(&new_seed, &secret) {
                        let _ = migrated_db.set_session_secret(session.id, &new_blob);
                        reencrypted += 1;
                    }
                }
                Err(_) => {
                    let _ = migrated_db.delete_session_secret(session.id);
                }
            }
        }

        // Vault entries' encrypted copies and the sync passphrase are sealed
        // with the same key: re-encrypt them too, or they'd be unreadable in
        // the new folder.
        for cred in state.db.list_vault_entries().unwrap_or_default() {
            let Ok(Some(blob)) = state.db.get_credential_secret(cred.id) else {
                continue;
            };
            match core_vault::decrypt_with_seed(&state.vault_seed, &blob) {
                Ok(secret) => {
                    if let Ok(new_blob) = core_vault::encrypt_with_seed(&new_seed, &secret) {
                        let _ = migrated_db.set_credential_secret(cred.id, &new_blob);
                        reencrypted += 1;
                    }
                }
                Err(_) => {
                    let _ = migrated_db.delete_credential_secret(cred.id);
                }
            }
        }
        if let Some(pass) = state
            .db
            .get_setting(super::sync::SYNC_PASSPHRASE_KEY)
            .ok()
            .flatten()
            .and_then(|hex| super::sync::hex_decode(&hex))
            .and_then(|blob| core_vault::decrypt_with_seed(&state.vault_seed, &blob).ok())
        {
            if let Ok(new_blob) = core_vault::encrypt_with_seed(&new_seed, &pass) {
                let _ = migrated_db.set_setting(
                    super::sync::SYNC_PASSPHRASE_KEY,
                    &super::sync::hex_encode(&new_blob),
                );
            }
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
            "Datos migrados: DB copiada, {reencrypted} credenciales re-cifradas, {logs} logs y {cats} catálogos copiados."
        )
    };

    let pointer = data_dir::pointer_path(&app)
        .ok_or_else(|| AppError::Internal("no se pudo resolver el directorio de config".into()))?;
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
