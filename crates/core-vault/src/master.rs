//! The vault key and the master password that protects it.
//!
//! Every stored secret is encrypted with one random 256-bit **vault key**.
//! That key never touches disk in the clear: the database holds it *wrapped*
//! under the user's **master password** (Argon2id + AES-256-GCM, the same
//! envelope as backup bundles — see [`crate::sealed`]). Opening the vault is
//! unwrapping the key, either by typing the master password or, on an
//! installed (non-portable) copy that chose "remember on this device", by
//! reading the key from the OS keyring.
//!
//! Consequences:
//!
//! * Copying the database (or a whole portable folder) reveals nothing
//!   without the master password.
//! * Changing the master password re-wraps the same key; no secret is
//!   re-encrypted.
//! * Each secret is bound to its owner (`context`, e.g. `session:42`) as AEAD
//!   associated data, so a blob moved to another row fails to decrypt instead
//!   of handing one session another's password.
//!
//! Secret blob format: `"ZV2"` + 12-byte nonce + AES-256-GCM ciphertext/tag.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};

use crate::Error;

/// Shortest master password accepted. Length is what resists guessing; no
/// character-class rules are imposed (NIST SP 800-63B).
pub const MIN_MASTER_PASSWORD_CHARS: usize = 14;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const BLOB_MAGIC: &[u8; 3] = b"ZV2";

/// Reject a master password that is too short. Counted in characters, not
/// bytes, so "ñ" is one.
pub fn validate_master_password(password: &str) -> Result<(), Error> {
    if password.chars().count() < MIN_MASTER_PASSWORD_CHARS {
        return Err(Error::Crypto(format!(
            "the master password must be at least {MIN_MASTER_PASSWORD_CHARS} characters"
        )));
    }
    Ok(())
}

/// The key every stored secret is encrypted with. Wiped from memory on drop;
/// never printed.
pub struct VaultKey(Zeroizing<[u8; KEY_LEN]>);

impl std::fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultKey(<redacted>)")
    }
}

impl VaultKey {
    /// A fresh random key, for a new vault.
    pub fn generate() -> Self {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::thread_rng().fill_bytes(key.as_mut());
        Self(key)
    }

    /// The key wrapped under `master_password`, to be stored in the database.
    /// The caller validates the password first ([`validate_master_password`]).
    pub fn wrap(&self, master_password: &str) -> Result<Vec<u8>, Error> {
        crate::sealed::seal(master_password, self.0.as_ref())
    }

    /// Inverse of [`wrap`](Self::wrap). A wrong password and a damaged
    /// envelope both come back as an error; nothing is returned in either.
    pub fn unwrap(master_password: &str, wrapped: &[u8]) -> Result<Self, Error> {
        let mut raw = crate::sealed::open(master_password, wrapped)?;
        let key = Self::from_bytes(&raw);
        raw.zeroize();
        key
    }

    /// Hex form, for the one OS keyring entry of "remember on this device".
    pub fn to_hex(&self) -> Zeroizing<String> {
        let mut out = Zeroizing::new(String::with_capacity(KEY_LEN * 2));
        for b in self.0.iter() {
            out.push_str(&format!("{b:02x}"));
        }
        out
    }

    /// Inverse of [`to_hex`](Self::to_hex).
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let hex = hex.trim();
        if hex.len() != KEY_LEN * 2 || !hex.is_ascii() {
            return Err(Error::BadCiphertext);
        }
        let mut raw = Zeroizing::new([0u8; KEY_LEN]);
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte =
                u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| Error::BadCiphertext)?;
        }
        Ok(Self(raw))
    }

    fn from_bytes(raw: &[u8]) -> Result<Self, Error> {
        if raw.len() != KEY_LEN {
            return Err(Error::BadCiphertext);
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(raw);
        Ok(Self(key))
    }

    /// Encrypt one secret. `context` names its owner (`session:42`,
    /// `credential:7`, `setting:backup.sync.passphrase`) and must be given
    /// again to decrypt.
    pub fn encrypt(&self, context: &str, plaintext: &str) -> Result<Vec<u8>, Error> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut ct = self
            .cipher()
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: context.as_bytes(),
                },
            )
            .map_err(|e| Error::Crypto(e.to_string()))?;
        let mut out = Vec::with_capacity(BLOB_MAGIC.len() + NONCE_LEN + ct.len());
        out.extend_from_slice(BLOB_MAGIC);
        out.extend_from_slice(&nonce);
        out.append(&mut ct);
        Ok(out)
    }

    /// Inverse of [`encrypt`](Self::encrypt). Fails for another key, another
    /// `context` or a changed byte.
    pub fn decrypt(&self, context: &str, blob: &[u8]) -> Result<String, Error> {
        if !is_vault_blob(blob) {
            return Err(Error::BadCiphertext);
        }
        let (nonce, ct) = blob[BLOB_MAGIC.len()..].split_at(NONCE_LEN);
        let mut pt = self
            .cipher()
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ct,
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| Error::Crypto("AEAD verification failed".into()))?;
        let result = std::str::from_utf8(&pt)
            .map(str::to_owned)
            .map_err(|e| Error::Crypto(e.to_string()));
        pt.zeroize();
        result
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(self.0.as_ref()))
    }
}

/// Whether `blob` is in the vault-key format (as opposed to the older
/// `vault.key`-seed format, which has no marker). Lets a reader pick the
/// right decryption while old blobs are being migrated.
pub fn is_vault_blob(blob: &[u8]) -> bool {
    blob.len() >= BLOB_MAGIC.len() + NONCE_LEN + 16 && &blob[..BLOB_MAGIC.len()] == BLOB_MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    const PW: &str = "correct horse battery";

    #[test]
    fn master_password_needs_fourteen_characters() {
        assert!(validate_master_password("thirteen chars").is_ok()); // 14
        assert!(validate_master_password("only thirteen").is_err()); // 13
                                                                     // Characters, not bytes: 14 two-byte letters pass, 13 don't.
        assert!(validate_master_password(&"ñ".repeat(14)).is_ok());
        assert!(validate_master_password(&"ñ".repeat(13)).is_err());
    }

    #[test]
    fn wrap_and_unwrap_give_the_same_key() {
        let key = VaultKey::generate();
        let blob = key.encrypt("session:1", "hunter2").unwrap();
        let wrapped = key.wrap(PW).unwrap();

        let again = VaultKey::unwrap(PW, &wrapped).unwrap();
        assert_eq!(again.decrypt("session:1", &blob).unwrap(), "hunter2");
    }

    #[test]
    fn wrong_master_password_opens_nothing() {
        let wrapped = VaultKey::generate().wrap(PW).unwrap();
        assert!(VaultKey::unwrap("correct horse batterx", &wrapped).is_err());
        assert!(VaultKey::unwrap("", &wrapped).is_err());
    }

    #[test]
    fn a_damaged_envelope_is_refused() {
        let mut wrapped = VaultKey::generate().wrap(PW).unwrap();
        let last = wrapped.len() - 1;
        wrapped[last] ^= 1;
        assert!(VaultKey::unwrap(PW, &wrapped).is_err());
        assert!(VaultKey::unwrap(PW, &wrapped[..10]).is_err());
    }

    #[test]
    fn changing_the_master_password_keeps_the_secrets() {
        let key = VaultKey::generate();
        let blob = key.encrypt("credential:7", "s3cret").unwrap();
        let old = key.wrap(PW).unwrap();

        // Change = unwrap with the old one, wrap with the new one.
        let new_pw = "a different long one";
        let rewrapped = VaultKey::unwrap(PW, &old).unwrap().wrap(new_pw).unwrap();

        let opened = VaultKey::unwrap(new_pw, &rewrapped).unwrap();
        assert_eq!(opened.decrypt("credential:7", &blob).unwrap(), "s3cret");
        assert!(VaultKey::unwrap(PW, &rewrapped).is_err());
    }

    #[test]
    fn two_wraps_of_one_key_differ() {
        // Fresh salt and nonce each time: the stored envelope doesn't reveal
        // that the password was kept.
        let key = VaultKey::generate();
        assert_ne!(key.wrap(PW).unwrap(), key.wrap(PW).unwrap());
    }

    #[test]
    fn a_secret_is_bound_to_its_owner() {
        let key = VaultKey::generate();
        let blob = key.encrypt("session:1", "hunter2").unwrap();
        assert!(key.decrypt("session:2", &blob).is_err());
        assert!(key.decrypt("credential:1", &blob).is_err());
        assert_eq!(key.decrypt("session:1", &blob).unwrap(), "hunter2");
    }

    #[test]
    fn another_key_or_a_changed_byte_fails() {
        let key = VaultKey::generate();
        let mut blob = key.encrypt("session:1", "hunter2").unwrap();
        assert!(VaultKey::generate().decrypt("session:1", &blob).is_err());
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(key.decrypt("session:1", &blob).is_err());
    }

    #[test]
    fn same_secret_encrypts_differently_each_time() {
        let key = VaultKey::generate();
        assert_ne!(
            key.encrypt("session:1", "hunter2").unwrap(),
            key.encrypt("session:1", "hunter2").unwrap()
        );
    }

    #[test]
    fn empty_and_unicode_secrets_round_trip() {
        let key = VaultKey::generate();
        for s in ["", "contraseña✓", "with\nnewline"] {
            let blob = key.encrypt("session:1", s).unwrap();
            assert_eq!(key.decrypt("session:1", &blob).unwrap(), s);
        }
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        let key = VaultKey::generate();
        let blob = key.encrypt("session:1", "hunter2").unwrap();
        let hex = key.to_hex();
        assert_eq!(hex.len(), 64);
        let again = VaultKey::from_hex(&hex).unwrap();
        assert_eq!(again.decrypt("session:1", &blob).unwrap(), "hunter2");

        assert!(VaultKey::from_hex("abcd").is_err());
        assert!(VaultKey::from_hex(&"zz".repeat(32)).is_err());
        assert!(VaultKey::from_hex(&"ñ".repeat(32)).is_err());
    }

    #[test]
    fn old_seed_blobs_are_told_apart() {
        let key = VaultKey::generate();
        assert!(is_vault_blob(&key.encrypt("session:1", "x").unwrap()));
        let old = crate::encrypt_with_seed("seed", "x").unwrap();
        // An old blob starts with a random nonce; it is the vault format only
        // if that nonce happens to start with the marker, and even then the
        // decryption fails rather than returning anything.
        if is_vault_blob(&old) {
            assert!(key.decrypt("session:1", &old).is_err());
        }
        assert!(!is_vault_blob(b"ZV2"));
    }

    #[test]
    fn debug_does_not_print_the_key() {
        let key = VaultKey::generate();
        let shown = format!("{key:?}");
        assert!(!shown.contains(key.to_hex().as_str()));
    }
}
