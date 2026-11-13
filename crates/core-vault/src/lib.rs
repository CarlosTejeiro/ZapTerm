#![forbid(unsafe_code)]

//! `core-vault` — credential storage: the OS keyring, the encrypted local
//! fallback, and the sealed backup format.
//!
//! Secrets (passwords, passphrases) go to Windows Credential Manager, macOS
//! Keychain or Linux Secret Service; the database holds an opaque
//! `keyring_key` reference. When the keyring can't hold a secret (no Secret
//! Service running, access denied) or on portable installs, the app keeps an
//! AES-256-GCM copy in the database instead, keyed off the per-install
//! `vault.key` (see the app's `secrets` module for the policy). Secrets never
//! go to logs.

pub mod error;
pub mod sealed;

pub use error::Error;
pub use sealed::{open, seal};

const SERVICE: &str = "zapterm";
/// The keyring service of ZAPX, ZapTerm's predecessor. Secrets of imported
/// ZAPX data are read from it once and copied to [`SERVICE`] (the ZAPX entry is
/// left in place so ZAPX keeps working). Keys are random (`ssh:<uuid>`,
/// `vault:<uuid>`), so the two apps' entries never collide.
const LEGACY_SERVICE: &str = "zapx";

/// Thin wrapper around the OS keyring.
pub struct Vault;

impl Vault {
    /// Store `secret` under `key` in the OS keyring.
    pub fn store(key: &str, secret: &str) -> Result<(), Error> {
        keyring::Entry::new(SERVICE, key)?.set_password(secret)?;
        Ok(())
    }

    /// Retrieve the secret stored under `key`. Not found under ZapTerm's own
    /// service, ZAPX's is tried (data imported from ZAPX) and a hit is copied
    /// over, so the next read finds it directly.
    pub fn retrieve(key: &str) -> Result<String, Error> {
        match keyring::Entry::new(SERVICE, key)?.get_password() {
            Ok(secret) => Ok(secret),
            Err(keyring::Error::NoEntry) => {
                let secret = keyring::Entry::new(LEGACY_SERVICE, key)?.get_password()?;
                // Best effort: if the copy fails the legacy entry still works.
                let _ = Self::store(key, &secret);
                Ok(secret)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Delete the credential stored under `key`.
    pub fn delete(key: &str) -> Result<(), Error> {
        keyring::Entry::new(SERVICE, key)?.delete_credential()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Local-fallback encryption — used when the OS keyring can't hold a secret
// (no Secret Service, access denied…) or on portable installs.
//
// The AES-256 key is derived from a high-entropy *seed*: 32 random bytes
// generated on first run and persisted to `vault.key` (mode 0600 on Unix) via
// [`load_or_create_seed`]. Earlier builds derived the key from a predictable
// value (the data-directory path, or a constant in portable mode), which made
// the key guessable — or, in portable mode, identical across every install.
// A random per-install keyfile means recovering the plaintext requires reading
// the keyfile itself, not merely knowing where the app stores its data.
// Ciphertexts are AES-256-GCM with a fresh random nonce prepended.
//
// This is still a *dev-mode convenience*, not a substitute for code-signing +
// OS keyring: an attacker who can read both `zapterm.db` and `vault.key` can
// recover the plaintext. The keyfile's randomness and 0600 permissions are
// what separate this from plain obfuscation.
// ---------------------------------------------------------------------------

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::Zeroize;

/// Load the local-fallback seed from `dir/vault.key`, generating it only when
/// the file doesn't exist. The seed is 32 random bytes, hex-encoded. On Unix
/// the file is owner-only (0600; an existing looser one is tightened).
///
/// Pass the returned seed to [`encrypt_with_seed`] / [`decrypt_with_seed`].
/// On failure (e.g. a read-only data dir) the caller is responsible for
/// choosing a degraded fallback.
pub fn load_or_create_seed(dir: &Path) -> Result<String, Error> {
    let key_path = dir.join("vault.key");
    match std::fs::read(&key_path) {
        Ok(bytes) => {
            // Only a missing file may be replaced: any key that exists is the
            // one every stored secret is encrypted with. A file that can't be
            // used is an error, never a reason to write a new key over it
            // (that would make every saved secret undecryptable for good).
            let text = std::str::from_utf8(&bytes).map_err(|_| {
                Error::Io(format!("{} is not a valid key file", key_path.display()))
            })?;
            let trimmed = text.trim();
            if trimmed.len() < 32 {
                return Err(Error::Io(format!(
                    "{} is damaged (too short); restore it from a backup or move it away to start fresh",
                    key_path.display()
                )));
            }
            #[cfg(unix)]
            tighten_permissions(&key_path);
            Ok(trimmed.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            bytes.zeroize();
            match create_new_private(&key_path, hex.as_bytes()) {
                Ok(()) => Ok(hex),
                // Another instance created it first: use theirs.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => load_or_create_seed(dir),
                Err(e) => Err(Error::Io(e.to_string())),
            }
        }
        Err(e) => Err(Error::Io(format!(
            "cannot read {}: {e}",
            key_path.display()
        ))),
    }
}

/// An existing key file readable by others (e.g. copied with 0644) is
/// restricted to the owner.
#[cfg(unix)]
fn tighten_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(md) = std::fs::metadata(path) {
        if md.permissions().mode() & 0o077 != 0 {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Create `path` with `contents`, owner-only (0600 on Unix), never replacing
/// an existing file: written to a temp file, synced, then linked into place
/// (fails with AlreadyExists if another process got there first), so a crash
/// can't leave a half-written key and two first runs can't each keep a
/// different key.
fn create_new_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
            // No hard links here (FAT/exFAT USB stick, some network shares):
            // create the file in place instead — still never replacing one.
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(path)?;
            f.write_all(contents)?;
            f.sync_all()
        }
        other => other,
    }
}

/// Encrypt `plaintext` with a key derived from `seed`. Returns
/// `nonce || ciphertext` ready to be stored as a single BLOB.
pub fn encrypt_with_seed(seed: &str, plaintext: &str) -> Result<Vec<u8>, Error> {
    let cipher = build_cipher(seed);
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| Error::Crypto(e.to_string()))?;
    // Prepend the nonce so decrypt is self-contained.
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.append(&mut ct);
    Ok(out)
}

/// Inverse of [`encrypt_with_seed`].
pub fn decrypt_with_seed(seed: &str, blob: &[u8]) -> Result<String, Error> {
    if blob.len() < 12 {
        return Err(Error::BadCiphertext);
    }
    let (nonce_bytes, ct) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = build_cipher_with_prefix(VAULT_PREFIX_CURRENT, seed);
    let Ok(mut pt) = cipher.decrypt(nonce, ct) else {
        return Err(Error::Crypto("AEAD verification failed".into()));
    };
    // Copy the secret out as UTF-8, then wipe the intermediate plaintext
    // buffer. The returned String still holds the secret — zeroizing that is
    // the caller's responsibility (see the password-cache lifecycle).
    let result = std::str::from_utf8(&pt)
        .map(str::to_owned)
        .map_err(|e| Error::Crypto(e.to_string()));
    pt.zeroize();
    result
}

const VAULT_PREFIX_CURRENT: &[u8] = b"zapx-vault-v1:";

fn build_cipher(seed: &str) -> Aes256Gcm {
    build_cipher_with_prefix(VAULT_PREFIX_CURRENT, seed)
}

fn build_cipher_with_prefix(prefix: &[u8], seed: &str) -> Aes256Gcm {
    // SHA-256(prefix || seed) → 32-byte AES-256 key.
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(seed.as_bytes());
    let key_bytes = hasher.finalize();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    Aes256Gcm::new(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let blob = encrypt_with_seed("/path/to/data", "hunter2").unwrap();
        let pt = decrypt_with_seed("/path/to/data", &blob).unwrap();
        assert_eq!(pt, "hunter2");
    }

    #[test]
    fn different_seed_fails() {
        let blob = encrypt_with_seed("seed-a", "hunter2").unwrap();
        assert!(decrypt_with_seed("seed-b", &blob).is_err());
    }

    #[test]
    fn nonce_makes_ciphertexts_unique() {
        let a = encrypt_with_seed("s", "same").unwrap();
        let b = encrypt_with_seed("s", "same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn seed_is_random_and_stable() {
        // A fresh dir yields a 64-hex-char seed; a second call returns the
        // SAME seed (read from the keyfile), and a different dir differs.
        let base = std::env::temp_dir().join(format!("zapterm-vault-{}", std::process::id()));
        let dir_a = base.join("a");
        let dir_b = base.join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        let s1 = load_or_create_seed(&dir_a).unwrap();
        assert_eq!(s1.len(), 64, "32 random bytes hex-encode to 64 chars");
        let s2 = load_or_create_seed(&dir_a).unwrap();
        assert_eq!(s1, s2, "seed must persist across calls");
        let s3 = load_or_create_seed(&dir_b).unwrap();
        assert_ne!(s1, s3, "independent dirs get independent seeds");

        // A blob written under the keyfile seed round-trips.
        let blob = encrypt_with_seed(&s1, "hunter2").unwrap();
        assert_eq!(decrypt_with_seed(&s1, &blob).unwrap(), "hunter2");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir_a.join("vault.key"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "keyfile must be owner-only");
        }

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod seed_tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zapterm-seed-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn created_once_then_reused() {
        let d = dir("reuse");
        let a = load_or_create_seed(&d).unwrap();
        assert_eq!(a.len(), 64);
        assert_eq!(load_or_create_seed(&d).unwrap(), a);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.join("vault.key"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A key file that exists but can't be used is an error — never silently
    /// replaced, which would make every stored secret undecryptable.
    #[test]
    fn a_damaged_key_file_is_not_overwritten() {
        let d = dir("damaged");
        std::fs::write(d.join("vault.key"), b"short").unwrap();
        assert!(load_or_create_seed(&d).is_err());
        assert_eq!(std::fs::read(d.join("vault.key")).unwrap(), b"short");
        std::fs::write(d.join("vault.key"), [0xffu8; 70]).unwrap();
        assert!(load_or_create_seed(&d).is_err());
        assert_eq!(
            std::fs::read(d.join("vault.key")).unwrap(),
            vec![0xffu8; 70]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_world_readable_key_file_is_tightened() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("perm");
        let p = d.join("vault.key");
        std::fs::write(&p, "a".repeat(64)).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        load_or_create_seed(&d).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
