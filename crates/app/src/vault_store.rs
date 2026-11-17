//! Where secrets live: the encrypted vault.
//!
//! [`VaultStore`] is the app's [`SecretStore`]: every secret is a row of
//! `vault_secrets`, encrypted under the vault key with its own name as
//! associated data. It takes the place the OS keyring had, so the callers'
//! logic (a credential row holds a key name; the secret is looked up by it)
//! is unchanged.
//!
//! **Moving what older versions stored.** A name the vault doesn't have yet
//! is looked up in the OS keyring, where older versions kept it. A hit is
//! written to the vault, read back, and only then removed from the keyring —
//! so a failure at any point leaves the secret where it was. This is also
//! what cleans up after older portable copies, which left their secrets in
//! the keyring of whatever computer they ran on.
//!
//! With the vault locked nothing can be read or written: `get` is `None` and
//! `put` is an error. The UI keeps the app behind the unlock screen until the
//! vault is open, so callers only see that at start-up races.

use core_persistence::Database;

use crate::commands::backup::SecretStore;
use crate::error::AppError;
use crate::master::MasterVault;

/// The OS keyring as older versions used it: one entry per secret.
pub(crate) trait LegacyKeyring {
    fn get(&self, key: &str) -> Option<String>;
    fn delete(&self, key: &str);
}

/// The real OS keyring.
pub(crate) struct OsKeyring;

impl LegacyKeyring for OsKeyring {
    fn get(&self, key: &str) -> Option<String> {
        core_vault::Vault::retrieve(key).ok()
    }
    fn delete(&self, key: &str) {
        core_vault::Vault::delete(key).ok();
    }
}

/// Names introduced with the vault (`typed:session:<id>`,
/// `typed:credential:<id>`, see `crate::secrets::Fallback`). Older versions
/// kept those secrets in the database under `vault.key`, never in the OS
/// keyring, so the keyring has nothing under them.
fn is_vault_only(key: &str) -> bool {
    key.starts_with("typed:")
}

pub(crate) struct VaultStore<'a> {
    pub db: &'a Database,
    pub vault: &'a MasterVault,
    pub legacy: &'a dyn LegacyKeyring,
}

impl VaultStore<'_> {
    fn read(&self, key: &str) -> Option<String> {
        let vault_key = self.vault.key()?;
        let blob = self.db.get_vault_secret(key).ok().flatten()?;
        match vault_key.decrypt(key, &blob) {
            Ok(secret) => Some(secret),
            Err(e) => {
                tracing::warn!(key, "a vault secret could not be decrypted: {e}");
                None
            }
        }
    }

    fn write(&self, key: &str, secret: &str) -> Result<(), AppError> {
        let vault_key = self
            .vault
            .key()
            .ok_or_else(|| AppError::Internal("the vault is locked".into()))?;
        let blob = vault_key
            .encrypt(key, secret)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        self.db
            .set_vault_secret(key, &blob)
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    /// Whether the vault itself (not the keyring) holds `key`.
    #[cfg(test)]
    pub(crate) fn holds(&self, key: &str) -> bool {
        matches!(self.db.get_vault_secret(key), Ok(Some(_)))
    }
}

impl SecretStore for VaultStore<'_> {
    fn get(&self, key: &str) -> Option<String> {
        if let Some(secret) = self.read(key) {
            return Some(secret);
        }
        // Locked: don't even look at the keyring. Handing out a secret while
        // the vault is locked would make the lock meaningless.
        self.vault.key()?;
        // Names that only ever existed in the vault were never in the OS
        // keyring: don't ask it. This is what keeps opening the vault from
        // costing one keyring lookup per saved session.
        if is_vault_only(key) {
            return None;
        }
        // A row that exists but didn't decrypt is damaged; the keyring copy,
        // if one is still there, may overwrite it below.
        let secret = self.legacy.get(key)?;
        if self.write(key, &secret).is_ok() && self.read(key).as_deref() == Some(secret.as_str()) {
            self.legacy.delete(key);
            tracing::info!(
                key,
                "moved a stored secret from the OS keyring into the vault"
            );
        }
        Some(secret)
    }

    fn put(&self, key: &str, secret: &str) -> Result<(), AppError> {
        self.write(key, secret)?;
        // An older copy in the keyring would be stale from now on.
        if !is_vault_only(key) {
            self.legacy.delete(key);
        }
        Ok(())
    }

    fn delete(&self, key: &str) {
        let _ = self.db.delete_vault_secret(key);
        if !is_vault_only(key) {
            self.legacy.delete(key);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use core_vault::VaultKey;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory stand-in for the OS keyring.
    #[derive(Default)]
    pub(crate) struct MemLegacy(pub Mutex<HashMap<String, String>>);

    impl MemLegacy {
        pub(crate) fn with(entries: &[(&str, &str)]) -> Self {
            Self(Mutex::new(
                entries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ))
        }
        pub(crate) fn has(&self, key: &str) -> bool {
            self.0.lock().unwrap().contains_key(key)
        }
    }

    impl LegacyKeyring for MemLegacy {
        fn get(&self, key: &str) -> Option<String> {
            self.0.lock().unwrap().get(key).cloned()
        }
        fn delete(&self, key: &str) {
            self.0.lock().unwrap().remove(key);
        }
    }

    /// A plain in-memory [`SecretStore`], for tests that don't care how the
    /// vault encrypts.
    #[derive(Default)]
    pub(crate) struct MemSecrets(pub Mutex<HashMap<String, String>>);

    impl SecretStore for MemSecrets {
        fn get(&self, key: &str) -> Option<String> {
            self.0.lock().unwrap().get(key).cloned()
        }
        fn put(&self, key: &str, secret: &str) -> Result<(), AppError> {
            self.0.lock().unwrap().insert(key.into(), secret.into());
            Ok(())
        }
        fn delete(&self, key: &str) {
            self.0.lock().unwrap().remove(key);
        }
    }

    pub(crate) fn temp_db(tag: &str) -> (std::path::PathBuf, Database) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "zapterm-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("zapterm.db")).unwrap();
        (dir, db)
    }

    struct Fixture {
        dir: std::path::PathBuf,
        db: Database,
        vault: MasterVault,
        legacy: MemLegacy,
    }

    impl Fixture {
        fn new(legacy: MemLegacy) -> Self {
            let (dir, db) = temp_db("vaultstore");
            let vault = MasterVault::default();
            vault.open_with(VaultKey::generate());
            Self {
                dir,
                db,
                vault,
                legacy,
            }
        }
        fn store(&self) -> VaultStore<'_> {
            VaultStore {
                db: &self.db,
                vault: &self.vault,
                legacy: &self.legacy,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn put_get_delete_round_trip_without_touching_the_keyring() {
        let f = Fixture::new(MemLegacy::default());
        let s = f.store();
        assert_eq!(s.get("ssh:a"), None);
        s.put("ssh:a", "hunter2").unwrap();
        assert_eq!(s.get("ssh:a").as_deref(), Some("hunter2"));
        assert!(f.legacy.0.lock().unwrap().is_empty());
        s.put("ssh:a", "changed").unwrap();
        assert_eq!(s.get("ssh:a").as_deref(), Some("changed"));
        s.delete("ssh:a");
        assert_eq!(s.get("ssh:a"), None);
    }

    #[test]
    fn the_database_holds_no_plaintext() {
        let f = Fixture::new(MemLegacy::default());
        f.store().put("ssh:a", "hunter2-very-recognisable").unwrap();
        let blob = f.db.get_vault_secret("ssh:a").unwrap().unwrap();
        let needle = b"hunter2-very-recognisable";
        assert!(!blob.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn a_keyring_secret_moves_into_the_vault_on_first_read() {
        let f = Fixture::new(MemLegacy::with(&[("ssh:a", "from-keyring")]));
        let s = f.store();
        assert!(!s.holds("ssh:a"));
        assert_eq!(s.get("ssh:a").as_deref(), Some("from-keyring"));
        assert!(s.holds("ssh:a"));
        assert!(
            !f.legacy.has("ssh:a"),
            "removed only after the vault has it"
        );
        // Second read comes from the vault.
        assert_eq!(s.get("ssh:a").as_deref(), Some("from-keyring"));
    }

    #[test]
    fn locked_reads_nothing_writes_nothing_and_leaves_the_keyring_alone() {
        let f = Fixture::new(MemLegacy::with(&[("ssh:a", "from-keyring")]));
        f.store().put("ssh:b", "in-vault").unwrap();
        f.vault.close();

        let s = f.store();
        assert_eq!(s.get("ssh:a"), None);
        assert_eq!(s.get("ssh:b"), None);
        assert!(s.put("ssh:c", "x").is_err());
        assert!(f.legacy.has("ssh:a"));
        assert!(!s.holds("ssh:c"));
    }

    #[test]
    fn a_secret_copied_to_another_name_does_not_decrypt() {
        let f = Fixture::new(MemLegacy::default());
        let s = f.store();
        s.put("ssh:a", "password-of-a").unwrap();
        // Someone with write access to the database points B at A's blob.
        let blob = f.db.get_vault_secret("ssh:a").unwrap().unwrap();
        f.db.set_vault_secret("ssh:b", &blob).unwrap();
        assert_eq!(s.get("ssh:b"), None);
        assert_eq!(s.get("ssh:a").as_deref(), Some("password-of-a"));
    }

    #[test]
    fn another_vault_key_reads_nothing() {
        let f = Fixture::new(MemLegacy::default());
        f.store().put("ssh:a", "hunter2").unwrap();
        f.vault.open_with(VaultKey::generate());
        assert_eq!(f.store().get("ssh:a"), None);
    }

    #[test]
    fn put_removes_a_stale_keyring_copy_and_delete_removes_both() {
        let f = Fixture::new(MemLegacy::with(&[("ssh:a", "old"), ("ssh:b", "old")]));
        let s = f.store();
        s.put("ssh:a", "new").unwrap();
        assert!(!f.legacy.has("ssh:a"));
        assert_eq!(s.get("ssh:a").as_deref(), Some("new"));

        s.delete("ssh:b");
        assert!(!f.legacy.has("ssh:b"));
        assert_eq!(s.get("ssh:b"), None);
    }
    #[test]
    fn vault_only_names_never_reach_the_keyring() {
        // Something under such a name in the keyring is not ours to read,
        // move or delete.
        let f = Fixture::new(MemLegacy::with(&[("typed:session:1", "stray")]));
        let s = f.store();
        assert_eq!(s.get("typed:session:1"), None);
        s.put("typed:session:1", "pw").unwrap();
        assert_eq!(s.get("typed:session:1").as_deref(), Some("pw"));
        s.delete("typed:session:1");
        assert_eq!(s.get("typed:session:1"), None);
        assert!(f.legacy.has("typed:session:1"));
    }
}
