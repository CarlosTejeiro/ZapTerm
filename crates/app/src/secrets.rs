//! One policy for where secrets (passwords, passphrases) live, used by every
//! caller (saved sessions, vault entries, backup restore).
//!
//! * **Every secret is in the encrypted vault** (`crate::vault_store`), under
//!   the name the schema already holds for it: a credential's `keyring_key`
//!   (`ssh:<uuid>`, `vault:<uuid>`), or `typed:session:<id>` /
//!   `typed:credential:<id>` for a secret with no credential row of its own
//!   (a password typed in a pane for an imported session).
//! * **Nothing is written to the OS keyring or under `vault.key` any more.**
//!   What older versions left there is read once and moved into the vault:
//!   written, read back, and only then removed from where it was.
//!
//! The function names still say "keyring" and "fallback" because the callers'
//! logic is unchanged: the vault took the keyring's place, and the "fallback"
//! is the `typed:` name. With the vault locked, reads find nothing and writes
//! fail; the unlock screen keeps the app from getting that far.

use core_persistence::Database;

use crate::commands::backup::SecretStore;

/// Which secret without a credential row of its own.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Fallback {
    /// A saved session's own password / passphrase.
    Session(i64),
    /// A credential row, e.g. a vault entry.
    Credential(i64),
}

impl Fallback {
    /// Its name in the vault.
    pub(crate) fn key(self) -> String {
        match self {
            Fallback::Session(id) => format!("typed:session:{id}"),
            Fallback::Credential(id) => format!("typed:credential:{id}"),
        }
    }
}

/// Store `secret` under `key`; false (logged) when the vault can't take it.
pub(crate) fn keyring_put(store: &dyn SecretStore, key: &str, secret: &str) -> bool {
    match store.put(key, secret) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("a secret could not be stored: {e}");
            false
        }
    }
}

pub(crate) fn keyring_get(store: &dyn SecretStore, key: &str) -> Option<String> {
    store.get(key)
}

/// The copy an older version kept in the database under the `vault.key` seed.
fn legacy_blob(db: &Database, seed: &str, target: Fallback) -> Option<String> {
    let blob = match target {
        Fallback::Session(id) => db.get_session_secret(id),
        Fallback::Credential(id) => db.get_credential_secret(id),
    }
    .ok()
    .flatten()?;
    core_vault::decrypt_with_seed(seed, &blob).ok()
}

fn delete_legacy_blob(db: &Database, target: Fallback) {
    let _ = match target {
        Fallback::Session(id) => db.delete_session_secret(id),
        Fallback::Credential(id) => db.delete_credential_secret(id),
    };
}

/// The secret kept for `target` itself. An old `vault.key` copy is moved into
/// the vault on the way: written, read back, then the old copy deleted.
pub(crate) fn read_fallback(
    store: &dyn SecretStore,
    db: &Database,
    seed: &str,
    target: Fallback,
) -> Option<String> {
    let key = target.key();
    if let Some(secret) = store.get(&key) {
        return Some(secret);
    }
    let secret = legacy_blob(db, seed, target)?;
    if store.put(&key, &secret).is_ok() && store.get(&key).as_deref() == Some(secret.as_str()) {
        delete_legacy_blob(db, target);
        tracing::info!(?target, "moved a stored secret into the vault");
    }
    Some(secret)
}

/// Keep `secret` for `target` itself; false (logged) on failure.
pub(crate) fn write_fallback(
    store: &dyn SecretStore,
    db: &Database,
    target: Fallback,
    secret: &str,
) -> bool {
    if !keyring_put(store, &target.key(), secret) {
        return false;
    }
    delete_legacy_blob(db, target);
    true
}

pub(crate) fn delete_fallback(store: &dyn SecretStore, db: &Database, target: Fallback) {
    store.delete(&target.key());
    delete_legacy_blob(db, target);
}

/// After `keyring_put` reported `stored`: the secret is under its
/// credential's name, so any copy kept for `target` itself is dropped. Err
/// when it was stored nowhere (the vault is locked).
pub(crate) fn settle_fallback(
    store: &dyn SecretStore,
    db: &Database,
    target: Fallback,
    stored: bool,
) -> Result<(), String> {
    if stored {
        delete_fallback(store, db, target);
        Ok(())
    } else {
        Err("could not store the secret: the vault is locked".into())
    }
}

/// Save `secret` under `keyring_key`. Err when it could not be stored.
pub(crate) fn persist(
    store: &dyn SecretStore,
    db: &Database,
    keyring_key: &str,
    target: Fallback,
    secret: &str,
) -> Result<(), String> {
    let stored = keyring_put(store, keyring_key, secret);
    settle_fallback(store, db, target, stored)
}

/// Read a secret: under its credential's name first, then the copy kept for
/// `target` itself — which is moved under the credential's name.
pub(crate) fn resolve(
    store: &dyn SecretStore,
    db: &Database,
    seed: &str,
    keyring_key: &str,
    target: Fallback,
) -> Option<String> {
    if let Some(s) = keyring_get(store, keyring_key) {
        return Some(s);
    }
    let secret = read_fallback(store, db, seed, target)?;
    migrate_to_keyring(store, db, keyring_key, target, &secret);
    Some(secret)
}

/// Move the copy kept for `target` under `keyring_key`: written, read back,
/// and only then the other copy deleted.
pub(crate) fn migrate_to_keyring(
    store: &dyn SecretStore,
    db: &Database,
    keyring_key: &str,
    target: Fallback,
    secret: &str,
) {
    if store.put(keyring_key, secret).is_err() {
        return;
    }
    if keyring_get(store, keyring_key).as_deref() == Some(secret) {
        delete_fallback(store, db, target);
    }
}

/// Move everything older versions stored into the vault, in one go: every
/// credential's keyring entry and every `vault.key` copy. Run when the vault
/// opens; reading each secret is what moves it. Returns how many secrets the
/// vault could read afterwards.
pub(crate) fn migrate_all(store: &dyn SecretStore, db: &Database, seed: &str) -> usize {
    let mut found = 0usize;
    for key in db.list_credential_keyring_keys().unwrap_or_default() {
        if store.get(&key).is_some() {
            found += 1;
        }
    }
    for session in db.list_sessions().unwrap_or_default() {
        let target = Fallback::Session(session.id);
        let Some(secret) = read_fallback(store, db, seed, target) else {
            continue;
        };
        found += 1;
        // A session with its own credential keeps the secret under that
        // credential's name, like a newly saved one.
        if let Some(cred) = session
            .credential_id
            .and_then(|id| db.get_credential(id).ok())
            .filter(|c| !c.reusable)
        {
            if store.get(&cred.keyring_key).is_none() {
                migrate_to_keyring(store, db, &cred.keyring_key, target, &secret);
            } else {
                delete_fallback(store, db, target);
            }
        }
    }
    for cred in db.list_vault_entries().unwrap_or_default() {
        let target = Fallback::Credential(cred.id);
        if let Some(secret) = read_fallback(store, db, seed, target) {
            if store.get(&cred.keyring_key).is_none() {
                migrate_to_keyring(store, db, &cred.keyring_key, target, &secret);
                found += 1;
            } else {
                delete_fallback(store, db, target);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::MasterVault;
    use crate::vault_store::tests::{temp_db, MemLegacy};
    use crate::vault_store::VaultStore;
    use core_vault::VaultKey;

    const SEED: &str = "old-vault-key-seed";

    /// A database as an older version left it, plus the "OS keyring" it used.
    struct Old {
        dir: std::path::PathBuf,
        db: Database,
        vault: MasterVault,
        legacy: MemLegacy,
    }

    impl Old {
        fn new(keyring: &[(&str, &str)]) -> Self {
            let (dir, db) = temp_db("secrets");
            let vault = MasterVault::default();
            vault.open_with(VaultKey::generate());
            Self {
                dir,
                db,
                vault,
                legacy: MemLegacy::with(keyring),
            }
        }

        fn store(&self) -> VaultStore<'_> {
            VaultStore {
                db: &self.db,
                vault: &self.vault,
                legacy: &self.legacy,
            }
        }

        fn session(&self, name: &str, credential_id: Option<i64>) -> i64 {
            self.db
                .create_session_full(
                    None,
                    name,
                    "ssh",
                    Some("host.example"),
                    Some(22),
                    Some("admin"),
                    credential_id,
                    "{}",
                    Some("password"),
                    None,
                )
                .unwrap()
        }

        fn private_credential(&self, key: &str) -> i64 {
            self.db
                .create_credential("session password", "ssh_password", Some("admin"), key)
                .unwrap()
        }

        fn seed_blob(&self, target: Fallback, secret: &str) {
            let blob = core_vault::encrypt_with_seed(SEED, secret).unwrap();
            match target {
                Fallback::Session(id) => self.db.set_session_secret(id, &blob).unwrap(),
                Fallback::Credential(id) => self.db.set_credential_secret(id, &blob).unwrap(),
            }
        }

        fn has_seed_blob(&self, target: Fallback) -> bool {
            match target {
                Fallback::Session(id) => self.db.get_session_secret(id),
                Fallback::Credential(id) => self.db.get_credential_secret(id),
            }
            .unwrap()
            .is_some()
        }
    }

    impl Drop for Old {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn a_new_secret_goes_only_to_the_vault() {
        let o = Old::new(&[]);
        let cred = o.private_credential("ssh:new");
        let id = o.session("fw", Some(cred));
        persist(
            &o.store(),
            &o.db,
            "ssh:new",
            Fallback::Session(id),
            "hunter2",
        )
        .unwrap();

        assert!(o.store().holds("ssh:new"));
        assert!(
            o.legacy.0.lock().unwrap().is_empty(),
            "nothing in the OS keyring"
        );
        assert!(
            !o.has_seed_blob(Fallback::Session(id)),
            "nothing under vault.key"
        );
        assert_eq!(
            resolve(&o.store(), &o.db, SEED, "ssh:new", Fallback::Session(id)).as_deref(),
            Some("hunter2")
        );
    }

    #[test]
    fn a_typed_password_left_under_vault_key_moves_into_the_vault() {
        // A session with no credential (e.g. a MobaXterm import) whose
        // password an older version kept under the vault.key seed.
        let o = Old::new(&[]);
        let id = o.session("imported", None);
        let target = Fallback::Session(id);
        o.seed_blob(target, "typed-pw");

        assert_eq!(
            read_fallback(&o.store(), &o.db, SEED, target).as_deref(),
            Some("typed-pw")
        );
        assert!(o.store().holds(&target.key()));
        assert!(
            !o.has_seed_blob(target),
            "old copy removed once the vault has it"
        );
        // And again, now from the vault alone.
        assert_eq!(
            read_fallback(&o.store(), &o.db, "another-seed", target).as_deref(),
            Some("typed-pw")
        );
    }

    #[test]
    fn migrate_all_moves_an_installed_copys_secrets() {
        // Installed copy: everything in the OS keyring, nothing in the DB.
        let o = Old::new(&[("ssh:a", "pw-a"), ("vault:shared", "pw-shared")]);
        let cred = o.private_credential("ssh:a");
        let a = o.session("a", Some(cred));
        o.db.create_vault_entry("shared", Some("root"), "vault:shared")
            .unwrap();

        assert_eq!(migrate_all(&o.store(), &o.db, SEED), 2);
        assert!(o.store().holds("ssh:a") && o.store().holds("vault:shared"));
        assert!(o.legacy.0.lock().unwrap().is_empty(), "keyring emptied");
        assert_eq!(
            resolve(&o.store(), &o.db, SEED, "ssh:a", Fallback::Session(a)).as_deref(),
            Some("pw-a")
        );
        // Running it again changes nothing.
        assert_eq!(migrate_all(&o.store(), &o.db, SEED), 2);
    }

    #[test]
    fn migrate_all_moves_a_portable_copys_secrets() {
        // Portable copy: the secret both in the host's keyring and under
        // vault.key in the database; a vault entry only under vault.key.
        let o = Old::new(&[("ssh:a", "pw-a")]);
        let cred = o.private_credential("ssh:a");
        let a = o.session("a", Some(cred));
        o.seed_blob(Fallback::Session(a), "pw-a");
        let shared =
            o.db.create_vault_entry("shared", Some("root"), "vault:shared")
                .unwrap();
        o.seed_blob(Fallback::Credential(shared), "pw-shared");
        let typed = o.session("imported", None);
        o.seed_blob(Fallback::Session(typed), "typed-pw");

        migrate_all(&o.store(), &o.db, SEED);

        let s = o.store();
        assert_eq!(s.get("ssh:a").as_deref(), Some("pw-a"));
        assert_eq!(s.get("vault:shared").as_deref(), Some("pw-shared"));
        assert_eq!(
            s.get(&Fallback::Session(typed).key()).as_deref(),
            Some("typed-pw")
        );
        assert!(
            o.legacy.0.lock().unwrap().is_empty(),
            "host keyring cleaned"
        );
        for t in [
            Fallback::Session(a),
            Fallback::Credential(shared),
            Fallback::Session(typed),
        ] {
            assert!(!o.has_seed_blob(t), "{t:?} still under vault.key");
        }
        // The session's own copy is not kept twice.
        assert!(!s.holds(&Fallback::Session(a).key()));
    }

    #[test]
    fn a_locked_vault_moves_nothing_and_loses_nothing() {
        let o = Old::new(&[("ssh:a", "pw-a")]);
        let cred = o.private_credential("ssh:a");
        let a = o.session("a", Some(cred));
        o.seed_blob(Fallback::Session(a), "pw-a");
        o.vault.close();

        assert_eq!(
            migrate_all(&o.store(), &o.db, SEED),
            1,
            "seed copy is still readable"
        );
        assert!(o.legacy.has("ssh:a"));
        assert!(o.has_seed_blob(Fallback::Session(a)));
        assert!(persist(&o.store(), &o.db, "ssh:a", Fallback::Session(a), "x").is_err());
    }

    #[test]
    fn a_wrong_seed_leaves_the_old_copy_in_place() {
        let o = Old::new(&[]);
        let id = o.session("imported", None);
        let target = Fallback::Session(id);
        o.seed_blob(target, "typed-pw");
        assert_eq!(
            read_fallback(&o.store(), &o.db, "not-the-seed", target),
            None
        );
        assert!(o.has_seed_blob(target));
    }
}
