//! The vault's lock state: creating it, opening it, closing it.
//!
//! The vault key (see `core_vault::master`) lives in memory only while the
//! vault is open. The database holds it wrapped under the master password,
//! plus a small check value that proves a candidate key is the right one.
//!
//! Two ways to open it:
//!
//! * **Master password** — always works.
//! * **Remembered on this device** — the key is kept in one OS keyring entry
//!   and read at start, so an installed copy opens without asking. Never on a
//!   portable install: it runs on other people's computers and must leave
//!   nothing behind in their keyring.
//!
//! Nothing here reads or writes secrets yet; that is the next step.

use std::sync::{Arc, Mutex};

use core_persistence::Database;
use core_vault::{validate_master_password, VaultKey, MIN_MASTER_PASSWORD_CHARS};
use serde::Serialize;

use crate::commands::backup::SecretStore;
use crate::error::AppError;

/// The vault key wrapped under the master password (hex).
pub(crate) const WRAPPED_KEY: &str = "vault.wrapped_key";
/// Random id of this vault; names its keyring entry, so two data folders on
/// one computer don't share (or overwrite) a remembered key.
pub(crate) const VAULT_ID: &str = "vault.id";
/// The vault id encrypted with the vault key (hex): decrypting it proves a
/// key read from the keyring belongs to this vault.
pub(crate) const CHECK: &str = "vault.check";
/// "1" when the key is kept in the OS keyring.
pub(crate) const REMEMBER: &str = "vault.remember";

const CHECK_CONTEXT: &str = "vault:check";

fn internal(msg: impl Into<String>) -> AppError {
    AppError::Internal(msg.into())
}

/// The open vault's key, shared by everything that encrypts or decrypts.
#[derive(Default)]
pub struct MasterVault {
    key: Mutex<Option<Arc<VaultKey>>>,
}

impl MasterVault {
    /// The key while the vault is open.
    pub(crate) fn key(&self) -> Option<Arc<VaultKey>> {
        self.key.lock().ok()?.clone()
    }

    fn set(&self, key: Option<VaultKey>) {
        if let Ok(mut slot) = self.key.lock() {
            *slot = key.map(Arc::new);
        }
    }

    /// Test helper: an open vault without going through a master password.
    #[cfg(test)]
    pub(crate) fn open_with(&self, key: VaultKey) {
        self.set(Some(key));
    }

    #[cfg(test)]
    pub(crate) fn close(&self) {
        self.set(None);
    }
}

/// What the UI needs to decide between set-up, unlock and normal use.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VaultStatus {
    /// A master password has been set.
    pub exists: bool,
    pub unlocked: bool,
    /// The key is kept in the OS keyring (always false on portable).
    pub remember: bool,
    /// Portable install: "remember" is not offered.
    pub portable: bool,
    pub min_password_chars: usize,
}

/// The vault operations, decoupled from Tauri state so tests run against a
/// temp database and an in-memory keyring.
pub(crate) struct Ctl<'a> {
    pub db: &'a Database,
    pub keyring: &'a dyn SecretStore,
    pub portable: bool,
    pub vault: &'a MasterVault,
}

impl Ctl<'_> {
    fn setting(&self, key: &str) -> Option<String> {
        self.db.get_setting(key).ok().flatten()
    }

    fn put_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.db
            .set_setting(key, value)
            .map_err(|e| internal(e.to_string()))
    }

    fn remember(&self) -> bool {
        !self.portable && self.setting(REMEMBER).as_deref() == Some("1")
    }

    fn keyring_entry(&self) -> Option<String> {
        self.setting(VAULT_ID).map(|id| format!("vault-key:{id}"))
    }

    pub(crate) fn status(&self) -> VaultStatus {
        VaultStatus {
            exists: self.setting(WRAPPED_KEY).is_some(),
            unlocked: self.vault.key().is_some(),
            remember: self.remember(),
            portable: self.portable,
            min_password_chars: MIN_MASTER_PASSWORD_CHARS,
        }
    }

    /// Whether `key` is this vault's key.
    fn is_ours(&self, key: &VaultKey) -> bool {
        let (Some(id), Some(check)) = (self.setting(VAULT_ID), self.setting(CHECK)) else {
            return false;
        };
        from_hex(&check)
            .and_then(|blob| key.decrypt(CHECK_CONTEXT, &blob).ok())
            .is_some_and(|plain| plain == id)
    }

    /// Keep or drop the keyring copy according to `remember`. Err when asked
    /// to remember and the keyring refuses.
    fn apply_remember(&self, key: &VaultKey, remember: bool) -> Result<(), AppError> {
        let Some(entry) = self.keyring_entry() else {
            return Err(internal("the vault has no id"));
        };
        if remember {
            if self.portable {
                return Err(internal(
                    "a portable install never stores the vault key on the computer it runs on",
                ));
            }
            self.keyring.put(&entry, &key.to_hex()).map_err(|e| {
                internal(format!(
                    "this computer's keyring could not store the vault key ({e}); the vault will ask for the master password instead"
                ))
            })?;
            self.put_setting(REMEMBER, "1")
        } else {
            self.keyring.delete(&entry);
            self.put_setting(REMEMBER, "0")
        }
    }

    /// Set the master password for the first time. Leaves the vault open.
    pub(crate) fn create(&self, password: &str, remember: bool) -> Result<VaultStatus, AppError> {
        if self.setting(WRAPPED_KEY).is_some() {
            return Err(internal("a master password is already set"));
        }
        validate_master_password(password).map_err(|e| internal(e.to_string()))?;

        let key = VaultKey::generate();
        let id = uuid::Uuid::new_v4().to_string();
        let wrapped = key.wrap(password).map_err(|e| internal(e.to_string()))?;
        let check = key
            .encrypt(CHECK_CONTEXT, &id)
            .map_err(|e| internal(e.to_string()))?;

        // The wrapped key goes last: until it is written the vault does not
        // exist, so a failure half-way leaves nothing that looks usable.
        self.put_setting(VAULT_ID, &id)?;
        self.put_setting(CHECK, &to_hex(&check))?;
        self.put_setting(WRAPPED_KEY, &to_hex(&wrapped))?;

        // The keyring refusing must not undo a vault that now exists: the
        // master password still opens it.
        let remembered = remember && !self.portable && self.apply_remember(&key, true).is_ok();
        if !remembered {
            self.put_setting(REMEMBER, "0")?;
        }
        self.vault.set(Some(key));
        Ok(self.status())
    }

    /// Open with the master password.
    pub(crate) fn unlock(&self, password: &str) -> Result<VaultStatus, AppError> {
        let wrapped = self
            .setting(WRAPPED_KEY)
            .ok_or_else(|| internal("no master password has been set yet"))?;
        let wrapped =
            from_hex(&wrapped).ok_or_else(|| internal("the stored vault key is damaged"))?;
        let key =
            VaultKey::unwrap(password, &wrapped).map_err(|_| internal("wrong master password"))?;
        // A remembered key the keyring lost (new OS profile, keyring reset)
        // is put back, so the next start opens by itself again.
        if self.remember() {
            let _ = self.apply_remember(&key, true);
        }
        self.vault.set(Some(key));
        Ok(self.status())
    }

    /// Open from the keyring, at start. False when it isn't remembered, the
    /// keyring has nothing, or what it has isn't this vault's key.
    pub(crate) fn auto_unlock(&self) -> bool {
        if self.vault.key().is_some() {
            return true;
        }
        if !self.remember() {
            return false;
        }
        let Some(key) = self
            .keyring_entry()
            .and_then(|entry| self.keyring.get(&entry))
            .and_then(|hex| VaultKey::from_hex(&hex).ok())
        else {
            return false;
        };
        if !self.is_ours(&key) {
            return false;
        }
        self.vault.set(Some(key));
        true
    }

    /// Forget the key in memory. With "remember" on, the next start (or
    /// `auto_unlock`) opens it again; that is what "remember" means.
    pub(crate) fn lock(&self) -> VaultStatus {
        self.vault.set(None);
        self.status()
    }

    /// Change the master password. The key stays the same, so no secret is
    /// re-encrypted and a remembered key stays valid.
    pub(crate) fn change_password(&self, old: &str, new: &str) -> Result<(), AppError> {
        let wrapped = self
            .setting(WRAPPED_KEY)
            .and_then(|hex| from_hex(&hex))
            .ok_or_else(|| internal("no master password has been set yet"))?;
        let key = VaultKey::unwrap(old, &wrapped).map_err(|_| internal("wrong master password"))?;
        validate_master_password(new).map_err(|e| internal(e.to_string()))?;
        let rewrapped = key.wrap(new).map_err(|e| internal(e.to_string()))?;
        self.put_setting(WRAPPED_KEY, &to_hex(&rewrapped))
    }

    /// Turn "remember on this device" on or off. Needs the vault open.
    pub(crate) fn set_remember(&self, remember: bool) -> Result<VaultStatus, AppError> {
        let key = self
            .vault
            .key()
            .ok_or_else(|| internal("unlock the vault first"))?;
        self.apply_remember(&key, remember)?;
        Ok(self.status())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const PW: &str = "correct horse battery";

    /// In-memory keyring; `refuse` makes every write fail, like a Linux box
    /// with no Secret Service.
    #[derive(Default)]
    struct MemKeyring {
        map: Mutex<HashMap<String, String>>,
        refuse: bool,
    }

    impl SecretStore for MemKeyring {
        fn get(&self, key: &str) -> Option<String> {
            self.map.lock().unwrap().get(key).cloned()
        }
        fn put(&self, key: &str, secret: &str) -> Result<(), AppError> {
            if self.refuse {
                return Err(internal("no keyring"));
            }
            self.map.lock().unwrap().insert(key.into(), secret.into());
            Ok(())
        }
        fn delete(&self, key: &str) {
            self.map.lock().unwrap().remove(key);
        }
    }

    struct Fixture {
        dir: std::path::PathBuf,
        db: Database,
        keyring: MemKeyring,
        vault: MasterVault,
    }

    impl Fixture {
        fn new() -> Self {
            // No tempfile crate here: a unique folder under the OS temp dir,
            // removed on drop (same as the backup tests).
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "zapterm-master-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let db = Database::open(&dir.join("zapterm.db")).unwrap();
            Self {
                dir,
                db,
                keyring: MemKeyring::default(),
                vault: MasterVault::default(),
            }
        }

        fn ctl(&self, portable: bool) -> Ctl<'_> {
            Ctl {
                db: &self.db,
                keyring: &self.keyring,
                portable,
                vault: &self.vault,
            }
        }

        /// The same database and keyring as seen by a freshly started app.
        fn restarted(&self, portable: bool) -> (MasterVault, bool) {
            let vault = MasterVault::default();
            let opened = Ctl {
                db: &self.db,
                keyring: &self.keyring,
                portable,
                vault: &vault,
            }
            .auto_unlock();
            (vault, opened)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn a_new_install_has_no_vault() {
        let f = Fixture::new();
        let s = f.ctl(false).status();
        assert!(!s.exists && !s.unlocked && !s.remember);
        assert_eq!(s.min_password_chars, 14);
        assert!(f.ctl(false).unlock(PW).is_err());
        assert!(!f.ctl(false).auto_unlock());
    }

    #[test]
    fn create_opens_the_vault_and_refuses_a_short_password() {
        let f = Fixture::new();
        assert!(f.ctl(false).create("too short", false).is_err());
        assert!(!f.ctl(false).status().exists);

        let s = f.ctl(false).create(PW, false).unwrap();
        assert!(s.exists && s.unlocked && !s.remember);
        // A second create must not replace the key every secret depends on.
        assert!(f.ctl(false).create("another long password", false).is_err());
    }

    #[test]
    fn lock_then_unlock_gives_back_the_same_key() {
        let f = Fixture::new();
        f.ctl(false).create(PW, false).unwrap();
        let blob = f
            .vault
            .key()
            .unwrap()
            .encrypt("session:1", "hunter2")
            .unwrap();

        assert!(!f.ctl(false).lock().unlocked);
        assert!(f.vault.key().is_none());
        assert!(f.ctl(false).unlock("wrong master password").is_err());
        assert!(f.vault.key().is_none());

        assert!(f.ctl(false).unlock(PW).unwrap().unlocked);
        let key = f.vault.key().unwrap();
        assert_eq!(key.decrypt("session:1", &blob).unwrap(), "hunter2");
    }

    #[test]
    fn without_remember_a_restart_stays_locked_and_the_keyring_is_empty() {
        let f = Fixture::new();
        f.ctl(false).create(PW, false).unwrap();
        assert!(f.keyring.map.lock().unwrap().is_empty());
        let (vault, opened) = f.restarted(false);
        assert!(!opened && vault.key().is_none());
    }

    #[test]
    fn remember_opens_at_restart_with_the_same_key() {
        let f = Fixture::new();
        assert!(f.ctl(false).create(PW, true).unwrap().remember);
        let blob = f
            .vault
            .key()
            .unwrap()
            .encrypt("session:1", "hunter2")
            .unwrap();

        let (vault, opened) = f.restarted(false);
        assert!(opened);
        let key = vault.key().unwrap();
        assert_eq!(key.decrypt("session:1", &blob).unwrap(), "hunter2");
    }

    #[test]
    fn portable_never_touches_the_keyring() {
        let f = Fixture::new();
        // Asked to remember, a portable install doesn't.
        let s = f.ctl(true).create(PW, true).unwrap();
        assert!(s.portable && !s.remember && s.unlocked);
        assert!(f.keyring.map.lock().unwrap().is_empty());
        assert!(f.ctl(true).set_remember(true).is_err());
        assert!(f.keyring.map.lock().unwrap().is_empty());

        let (_, opened) = f.restarted(true);
        assert!(!opened);
    }

    #[test]
    fn a_data_folder_made_remembered_is_not_auto_opened_as_portable() {
        // An installed copy with "remember" on, later run in portable mode
        // (or its folder copied to a USB stick): the keyring is ignored.
        let f = Fixture::new();
        f.ctl(false).create(PW, true).unwrap();
        let (_, opened) = f.restarted(true);
        assert!(!opened);
        assert!(!f.ctl(true).status().remember);
    }

    #[test]
    fn a_keyring_that_refuses_leaves_a_working_vault() {
        let mut f = Fixture::new();
        f.keyring.refuse = true;
        let s = f.ctl(false).create(PW, true).unwrap();
        assert!(s.exists && s.unlocked && !s.remember);
        assert!(f.ctl(false).set_remember(true).is_err());
        f.ctl(false).lock();
        assert!(f.ctl(false).unlock(PW).unwrap().unlocked);
    }

    #[test]
    fn a_wrong_key_in_the_keyring_is_ignored() {
        let f = Fixture::new();
        f.ctl(false).create(PW, true).unwrap();
        let entry = f.ctl(false).keyring_entry().unwrap();

        f.keyring
            .put(&entry, &VaultKey::generate().to_hex())
            .unwrap();
        assert!(!f.restarted(false).1);
        f.keyring.put(&entry, "not a key").unwrap();
        assert!(!f.restarted(false).1);

        // The master password still opens it, and repairs the keyring copy.
        f.ctl(false).lock();
        f.ctl(false).unlock(PW).unwrap();
        assert!(f.restarted(false).1);
    }

    #[test]
    fn turning_remember_off_removes_the_keyring_copy() {
        let f = Fixture::new();
        f.ctl(false).create(PW, true).unwrap();
        assert_eq!(f.keyring.map.lock().unwrap().len(), 1);

        assert!(!f.ctl(false).set_remember(false).unwrap().remember);
        assert!(f.keyring.map.lock().unwrap().is_empty());
        assert!(!f.restarted(false).1);

        assert!(f.ctl(false).set_remember(true).unwrap().remember);
        assert!(f.restarted(false).1);
    }

    #[test]
    fn set_remember_needs_an_open_vault() {
        let f = Fixture::new();
        f.ctl(false).create(PW, false).unwrap();
        f.ctl(false).lock();
        assert!(f.ctl(false).set_remember(true).is_err());
        assert!(f.keyring.map.lock().unwrap().is_empty());
    }

    #[test]
    fn changing_the_password_keeps_secrets_and_the_remembered_key() {
        let f = Fixture::new();
        f.ctl(false).create(PW, true).unwrap();
        let blob = f
            .vault
            .key()
            .unwrap()
            .encrypt("credential:7", "s3cret")
            .unwrap();
        let new_pw = "a different long one";

        assert!(f
            .ctl(false)
            .change_password("not the old one!!", new_pw)
            .is_err());
        assert!(f.ctl(false).change_password(PW, "short").is_err());
        // A refused change leaves the old password working.
        f.ctl(false).lock();
        assert!(f.ctl(false).unlock(PW).is_ok());

        f.ctl(false).change_password(PW, new_pw).unwrap();
        f.ctl(false).lock();
        assert!(f.ctl(false).unlock(PW).is_err());
        assert!(f.ctl(false).unlock(new_pw).is_ok());
        let key = f.vault.key().unwrap();
        assert_eq!(key.decrypt("credential:7", &blob).unwrap(), "s3cret");

        let (vault, opened) = f.restarted(false);
        assert!(opened);
        assert_eq!(
            vault.key().unwrap().decrypt("credential:7", &blob).unwrap(),
            "s3cret"
        );
    }

    #[test]
    fn two_vaults_on_one_computer_keep_separate_keyring_entries() {
        let a = Fixture::new();
        a.ctl(false).create(PW, true).unwrap();
        let entry_a = a.ctl(false).keyring_entry().unwrap();

        let b = Fixture::new();
        b.ctl(false).create(PW, true).unwrap();
        assert_ne!(entry_a, b.ctl(false).keyring_entry().unwrap());

        // Vault B's key planted under vault A's entry does not open A.
        let key_b = b.vault.key().unwrap().to_hex();
        a.keyring.put(&entry_a, &key_b).unwrap();
        assert!(!a.restarted(false).1);
    }

    #[test]
    fn hex_helpers_round_trip_and_reject_garbage() {
        assert_eq!(
            from_hex(&to_hex(&[0, 1, 254, 255])).unwrap(),
            [0, 1, 254, 255]
        );
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
        assert!(from_hex("ññ").is_none());
    }
}
