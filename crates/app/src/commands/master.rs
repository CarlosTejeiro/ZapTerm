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

/// Current state. A vault remembered on this device is opened here, so the
/// first call at start tells the UI whether to ask for the master password.
#[tauri::command]
pub async fn master_status(state: State<'_, AppState>) -> Result<VaultStatus, AppError> {
    let keyring = KeyringStore;
    let ctl = ctl(&state, &keyring);
    ctl.auto_unlock();
    Ok(ctl.status())
}

#[tauri::command]
pub async fn master_create(
    state: State<'_, AppState>,
    password: String,
    remember: bool,
) -> Result<VaultStatus, AppError> {
    let password = zeroize::Zeroizing::new(password);
    ctl(&state, &KeyringStore).create(&password, remember)
}

#[tauri::command]
pub async fn master_unlock(
    state: State<'_, AppState>,
    password: String,
) -> Result<VaultStatus, AppError> {
    let password = zeroize::Zeroizing::new(password);
    ctl(&state, &KeyringStore).unlock(&password)
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
