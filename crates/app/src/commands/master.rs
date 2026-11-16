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
    let moved = crate::secrets::migrate_all(&state.secret_store(), &state.db, &state.vault_seed);
    tracing::info!(secrets = moved, "vault open");
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
