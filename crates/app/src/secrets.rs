//! One policy for where secrets (passwords, passphrases) live, used by every
//! caller (saved sessions, vault entries, backup restore).
//!
//! * **Primary: the OS keyring** — macOS Keychain, Windows Credential
//!   Manager, Linux Secret Service.
//! * **Fallback: an encrypted copy in the database** (AES-256-GCM under the
//!   per-install `vault.key`), kept only when the keyring can't hold the
//!   secret (no Secret Service running, access denied…) or on portable
//!   installs, which don't rely on the keyring by design. When the keyring
//!   works the database holds no copy, so reading `zapterm.db` + `vault.key`
//!   doesn't reveal it.
//! * **Migration:** a secret found only in the database while the keyring
//!   works is moved into it — written, read back, and only then the database
//!   copy is deleted.
//!
//! Before 0.29.3 the keyring crate was built without any OS backend, so
//! nothing ever reached the keyring and every secret lived in the database
//! fallback; the migration above moves those copies on first use.

use core_persistence::Database;

/// Which encrypted database copy belongs to a secret.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Fallback {
    /// A saved session's own password / passphrase (`session_secrets`).
    Session(i64),
    /// A credential row, e.g. a vault entry (`credential_secrets`).
    Credential(i64),
}

/// Store `secret` in the OS keyring under `key`; false (logged) when the
/// keyring can't take it.
pub(crate) fn keyring_put(key: &str, secret: &str) -> bool {
    match core_vault::Vault::store(key, secret) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("OS keyring unavailable ({e}); using the encrypted database copy");
            false
        }
    }
}

pub(crate) fn keyring_get(key: &str) -> Option<String> {
    core_vault::Vault::retrieve(key).ok()
}

pub(crate) fn read_fallback(db: &Database, seed: &str, target: Fallback) -> Option<String> {
    let blob = match target {
        Fallback::Session(id) => db.get_session_secret(id),
        Fallback::Credential(id) => db.get_credential_secret(id),
    }
    .ok()
    .flatten()?;
    core_vault::decrypt_with_seed(seed, &blob).ok()
}

/// Write the encrypted database copy; false (logged) on failure.
pub(crate) fn write_fallback(db: &Database, seed: &str, target: Fallback, secret: &str) -> bool {
    let blob = match core_vault::encrypt_with_seed(seed, secret) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(?target, "encrypting the secret failed: {e}");
            return false;
        }
    };
    let written = match target {
        Fallback::Session(id) => db.set_session_secret(id, &blob),
        Fallback::Credential(id) => db.set_credential_secret(id, &blob),
    };
    match written {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(?target, "saving the encrypted secret failed: {e}");
            false
        }
    }
}

pub(crate) fn delete_fallback(db: &Database, target: Fallback) {
    let _ = match target {
        Fallback::Session(id) => db.delete_session_secret(id),
        Fallback::Credential(id) => db.delete_credential_secret(id),
    };
}

/// After `keyring_put` reported `in_keyring`: keep or drop the database copy
/// by the policy above. Err only when the secret ended up stored nowhere.
pub(crate) fn settle_fallback(
    db: &Database,
    seed: &str,
    target: Fallback,
    secret: &str,
    in_keyring: bool,
    portable: bool,
) -> Result<(), String> {
    if in_keyring && !portable {
        delete_fallback(db, target);
        return Ok(());
    }
    if write_fallback(db, seed, target, secret) || in_keyring {
        Ok(())
    } else {
        Err("could not store the secret: the OS keyring is unavailable and saving the encrypted copy failed".into())
    }
}

/// Save `secret` under `keyring_key` (keyring first, database copy when
/// needed). Err only when it could be stored nowhere.
pub(crate) fn persist(
    db: &Database,
    seed: &str,
    keyring_key: &str,
    target: Fallback,
    secret: &str,
    portable: bool,
) -> Result<(), String> {
    let in_keyring = keyring_put(keyring_key, secret);
    settle_fallback(db, seed, target, secret, in_keyring, portable)
}

/// Read a secret: keyring first, then the database copy — moving that copy
/// into the keyring when it works (non-portable installs).
pub(crate) fn resolve(
    db: &Database,
    seed: &str,
    keyring_key: &str,
    target: Fallback,
    portable: bool,
) -> Option<String> {
    if let Some(s) = keyring_get(keyring_key) {
        return Some(s);
    }
    let secret = read_fallback(db, seed, target)?;
    if !portable {
        migrate_to_keyring(db, keyring_key, target, &secret);
    }
    Some(secret)
}

/// Move a database copy into the keyring: written, read back, and only then
/// the database copy deleted. Quietly does nothing if the keyring is
/// unavailable (the copy stays and keeps working).
pub(crate) fn migrate_to_keyring(db: &Database, keyring_key: &str, target: Fallback, secret: &str) {
    if core_vault::Vault::store(keyring_key, secret).is_err() {
        return;
    }
    if keyring_get(keyring_key).as_deref() == Some(secret) {
        delete_fallback(db, target);
        tracing::info!(?target, "moved a stored secret into the OS keyring");
    }
}
