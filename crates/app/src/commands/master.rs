//! Tauri commands for the vault's lock state (see `crate::master`).

use tauri::State;

use crate::commands::backup::KeyringStore;
use crate::error::AppError;
use crate::master::{Ctl, VaultStatus};
use crate::state::AppState;

fn ctl<'a>(state: &'a AppState, keyring: &'a KeyringStore) -> Ctl<'a> {
    Ctl {
        db: &state.db,
        keyring,
        portable: state.data_dir.is_portable(),
        vault: &state.master_vault,
    }
}

/// The vault just opened: move whatever older versions stored (OS keyring
/// entries, `vault.key` copies) into it.
fn opened(state: &AppState) {
    let store = state.secret_store();
    let moved = crate::secrets::migrate_all(&store, &state.db, &state.vault_seed);
    super::sync::migrate_passphrase(&store, &state.db, &state.vault_seed);
    tracing::info!(secrets = moved, "vault open");
    retire_key_file(&state.db, &state.data_dir.dir);
}

/// Delete `vault.key` once nothing is encrypted under it any more. While any
/// old copy remains (a secret that could not be moved), the file stays.
fn retire_key_file(db: &core_persistence::Database, data_dir: &std::path::Path) {
    let key_file = data_dir.join("vault.key");
    if !key_file.exists() {
        return;
    }
    let copies_left = db.count_legacy_secret_copies().unwrap_or(1) > 0
        || matches!(
            db.get_setting(super::sync::SYNC_PASSPHRASE_KEY),
            Ok(Some(_)) | Err(_)
        );
    if copies_left {
        return;
    }
    match std::fs::remove_file(&key_file) {
        Ok(()) => tracing::info!("vault.key removed: every secret is in the vault"),
        Err(e) => tracing::warn!("vault.key could not be removed: {e}"),
    }
}

/// At start: open a vault remembered on this device, so sessions restored
/// from the last run find their passwords without waiting for the UI.
pub(crate) fn open_at_start(state: &AppState) {
    let keyring = KeyringStore;
    if ctl(state, &keyring).auto_unlock() {
        opened(state);
    }
}

/// Current state. A vault remembered on this device is opened here if it
/// isn't yet, so the UI learns whether to ask for the master password.
#[tauri::command]
pub async fn master_status(state: State<'_, AppState>) -> Result<VaultStatus, AppError> {
    let keyring = KeyringStore;
    let ctl = ctl(&state, &keyring);
    let was_open = ctl.status().unlocked;
    if ctl.auto_unlock() && !was_open {
        opened(&state);
    }
    Ok(ctl.status())
}

#[tauri::command]
pub async fn master_create(
    state: State<'_, AppState>,
    password: String,
    remember: bool,
) -> Result<VaultStatus, AppError> {
    let password = zeroize::Zeroizing::new(password);
    let status = ctl(&state, &KeyringStore).create(&password, remember)?;
    opened(&state);
    Ok(status)
}

#[tauri::command]
pub async fn master_unlock(
    state: State<'_, AppState>,
    password: String,
) -> Result<VaultStatus, AppError> {
    let password = zeroize::Zeroizing::new(password);
    let status = ctl(&state, &KeyringStore).unlock(&password)?;
    opened(&state);
    Ok(status)
}

#[tauri::command]
pub async fn master_lock(state: State<'_, AppState>) -> Result<VaultStatus, AppError> {
    Ok(ctl(&state, &KeyringStore).lock())
}

#[tauri::command]
pub async fn master_change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), AppError> {
    let old = zeroize::Zeroizing::new(old_password);
    let new = zeroize::Zeroizing::new(new_password);
    ctl(&state, &KeyringStore).change_password(&old, &new)
}

#[tauri::command]
pub async fn master_set_remember(
    state: State<'_, AppState>,
    remember: bool,
) -> Result<VaultStatus, AppError> {
    ctl(&state, &KeyringStore).set_remember(remember)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault_store::tests::temp_db;

    #[test]
    fn the_key_file_goes_only_when_nothing_depends_on_it() {
        let (dir, db) = temp_db("retire");
        let key_file = dir.join("vault.key");
        std::fs::write(&key_file, "k".repeat(64)).unwrap();
        let id = db
            .create_vault_entry("shared", Some("root"), "vault:shared")
            .unwrap();

        // A secret still under the old seed: the file stays.
        db.set_credential_secret(id, b"old-blob").unwrap();
        retire_key_file(&db, &dir);
        assert!(key_file.exists());
        db.delete_credential_secret(id).unwrap();

        // The sync passphrase still under the old seed: the file stays.
        db.set_setting(crate::commands::sync::SYNC_PASSPHRASE_KEY, "00")
            .unwrap();
        retire_key_file(&db, &dir);
        assert!(key_file.exists());
        db.delete_setting(crate::commands::sync::SYNC_PASSPHRASE_KEY)
            .unwrap();

        retire_key_file(&db, &dir);
        assert!(!key_file.exists());
        // Nothing there: nothing to do, no error.
        retire_key_file(&db, &dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
