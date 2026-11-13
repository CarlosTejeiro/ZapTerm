//! Encrypted backup bundles (`.zapxb`).
//!
//! A bundle is the *whole* ZapTerm environment — sessions, folders, groups,
//! snippets and macros, highlight rules, settings, trusted host keys **and
//! credentials** — serialised as JSON and sealed with [`core_vault::seal`]
//! (Argon2id + AES-256-GCM) under a passphrase the user chooses. Because the
//! only key is that passphrase, the file can sit in any cloud-synced folder
//! (Dropbox, Drive, iCloud, OneDrive, a NAS…) without trusting the provider,
//! and a fresh install on another device gets everything back by opening it.
//!
//! This module is phase 1: manual export / inspect / restore. The automatic
//! sync folder (phase 2) reuses the same bundle format.
//!
//! Restore semantics are a superset of the plain sessions import
//! (`transfer::apply_import`, idempotent by identity) plus:
//! - vault entries are matched **by name**: new names are created, existing
//!   ones get the bundle's username and secret;
//! - session credentials are re-attached only to sessions that have **no**
//!   credential here yet (freshly imported, or local ones never given one),
//!   so a restore never overwrites a password the user set on this device;
//! - settings from the bundle overwrite local values, except the `backup.*`
//!   keys that describe this particular install;
//! - `known_hosts` lines are merged, never replaced.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use zeroize::Zeroize;

use core_persistence::{Credential, Database, SavedSession};

use super::transfer::{self, ExportFile, ImportSummary};
use crate::error::AppError;
use crate::state::{AppState, PasswordCache};

/// Bundle schema version — bump on breaking changes; restore rejects newer.
pub const BUNDLE_VERSION: u32 = 1;
/// File extension the UI filters on.
pub const BUNDLE_EXTENSION: &str = "zapxb";
/// Passphrases shorter than this are refused at export time. Argon2id makes
/// guessing slow, not impossible — a 3-character passphrase is still 3
/// characters.
pub const MIN_PASSPHRASE_CHARS: usize = 8;
/// Settings key holding this install's stable device id (created lazily).
pub const DEVICE_ID_KEY: &str = "backup.device_id";
/// Settings that describe *this* install and must neither travel in a bundle
/// nor be overwritten by one.
const LOCAL_SETTING_PREFIXES: &[&str] = &["backup."];
/// Settings that belong to this machine, not to the environment: a Python
/// path (`C:\Python312\python.exe` means nothing on a Mac) and window
/// layout (a sidebar drag would otherwise count as a change to sync and
/// overwrite the other devices' layout).
const LOCAL_SETTING_KEYS: &[&str] = &["scripts.python", "ui.sidebar_width"];

// ── bundle format ───────────────────────────────────────────────────────────

// Debug for the secret-carrying parts prints everything but the secret, so a
// `{:?}` in a log line or a test message can't leak passwords.
impl std::fmt::Debug for BundleVaultEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleVaultEntry")
            .field("name", &self.name)
            .field("username", &self.username)
            .field("secret", &"<redacted>")
            .finish()
    }
}

impl std::fmt::Debug for BundleSessionCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleSessionCredential")
            .field("session_id", &self.session_id)
            .field("vault_name", &self.vault_name)
            .field("kind", &self.kind)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub zapx_bundle: u32,
    pub app_version: String,
    pub created_at: String,
    pub device: DeviceInfo,
    /// The plain environment export (credentials stripped, ids file-local).
    pub environment: ExportFile,
    #[serde(default)]
    pub settings: HashMap<String, String>,
    #[serde(default)]
    pub vault_entries: Vec<BundleVaultEntry>,
    #[serde(default)]
    pub session_credentials: Vec<BundleSessionCredential>,
    /// Verbatim `~/.ssh/known_hosts`, if present on the exporting device.
    #[serde(default)]
    pub known_hosts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Random UUID minted once per install (`backup.device_id`).
    pub id: String,
    /// Hostname at export time — display only.
    pub name: String,
}

/// A reusable vault entry with its secret. Matched by `name` on restore.
#[derive(Serialize, Deserialize)]
pub struct BundleVaultEntry {
    pub name: String,
    #[serde(default)]
    pub username: Option<String>,
    pub secret: String,
}

impl Drop for BundleVaultEntry {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// The credential of one session, referenced by its file-local id. Either a
/// link to a vault entry (`vault_name`) or a private secret (`kind` +
/// `secret`).
#[derive(Serialize, Deserialize)]
pub struct BundleSessionCredential {
    pub session_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_name: Option<String>,
    /// `ssh_password` (a password) or `ssh_key` (a key-file passphrase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

impl Drop for BundleSessionCredential {
    fn drop(&mut self) {
        if let Some(s) = self.secret.as_mut() {
            s.zeroize();
        }
    }
}

/// What the UI shows before restoring — counts only, never secrets.
#[derive(Debug, Clone, Serialize)]
pub struct BundleInfo {
    pub app_version: String,
    pub created_at: String,
    pub device_name: String,
    pub sessions: usize,
    pub folders: usize,
    pub groups: usize,
    pub snippets: usize,
    pub rules: usize,
    pub vault_entries: usize,
    pub session_credentials: usize,
    pub settings: usize,
    pub known_hosts_lines: usize,
}

impl Bundle {
    pub fn info(&self) -> BundleInfo {
        BundleInfo {
            app_version: self.app_version.clone(),
            created_at: self.created_at.clone(),
            device_name: self.device.name.clone(),
            sessions: self.environment.sessions.len(),
            folders: self.environment.folders.len(),
            groups: self.environment.broadcast_groups.len(),
            snippets: self.environment.snippets.len(),
            rules: self.environment.highlight_rules.len(),
            vault_entries: self.vault_entries.len(),
            session_credentials: self.session_credentials.len(),
            settings: self.settings.len(),
            known_hosts_lines: self
                .known_hosts
                .as_deref()
                .map(|t| {
                    t.lines()
                        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                        .count()
                })
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BackupExportSummary {
    pub path: String,
    pub info: BundleInfo,
    /// Items that could not be included (e.g. a secret the keyring refused).
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct RestoreSummary {
    pub environment: ImportSummary,
    pub vault_added: usize,
    pub vault_updated: usize,
    pub credentials_linked: usize,
    pub settings_applied: usize,
    pub known_hosts_added: usize,
    pub warnings: Vec<String>,
}

// ── secret backends ─────────────────────────────────────────────────────────

/// Where per-credential secrets live. Production is the OS keyring; tests use
/// an in-memory map so they never touch a real keychain.
pub(crate) trait SecretStore {
    fn get(&self, key: &str) -> Option<String>;
    fn put(&self, key: &str, secret: &str) -> Result<(), AppError>;
    fn delete(&self, key: &str);
}

/// OS keyring, with the same portable-mode tolerance as session creation.
pub(crate) struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Option<String> {
        core_vault::Vault::retrieve(key).ok()
    }
    fn put(&self, key: &str, secret: &str) -> Result<(), AppError> {
        core_vault::Vault::store(key, secret).map_err(|e| AppError::Internal(e.to_string()))
    }
    fn delete(&self, key: &str) {
        core_vault::Vault::delete(key).ok();
    }
}

/// Everything build/apply need, decoupled from Tauri state so the round-trip
/// can be unit-tested against temp databases.
pub(crate) struct BackupCtx<'a> {
    pub db: &'a Database,
    pub secrets: &'a dyn SecretStore,
    pub vault_seed: &'a str,
    pub password_cache: Option<&'a PasswordCache>,
    pub portable: bool,
    /// `known_hosts` to read from / merge into. `None` = the user's real file.
    pub known_hosts_path: Option<PathBuf>,
}

impl<'a> BackupCtx<'a> {
    pub(crate) fn from_state(state: &'a AppState, secrets: &'a dyn SecretStore) -> Self {
        Self {
            db: &state.db,
            secrets,
            vault_seed: &state.vault_seed,
            password_cache: Some(&state.password_cache),
            portable: state.data_dir.is_portable(),
            known_hosts_path: None,
        }
    }

    fn known_hosts_file(&self) -> Option<PathBuf> {
        self.known_hosts_path
            .clone()
            .or_else(core_transport::known_hosts_path)
    }

    /// Resolve a credential's secret: keyring first, then the encrypted DB
    /// fallback that vault entries carry.
    fn credential_secret(&self, cred: &Credential) -> Option<String> {
        self.secrets.get(&cred.keyring_key).or_else(|| {
            self.db
                .get_credential_secret(cred.id)
                .ok()
                .flatten()
                .and_then(|blob| core_vault::decrypt_with_seed(self.vault_seed, &blob).ok())
        })
    }

    /// Resolve a session's private secret through the same chain the SSH
    /// connect path uses: keyring → in-memory cache → encrypted DB fallback.
    fn session_secret(&self, session: &SavedSession, cred: &Credential) -> Option<String> {
        self.secrets
            .get(&cred.keyring_key)
            .or_else(|| {
                self.password_cache
                    .and_then(|c| c.lock().unwrap().get(&session.id).cloned())
            })
            .or_else(|| {
                self.db
                    .get_session_secret(session.id)
                    .ok()
                    .flatten()
                    .and_then(|blob| core_vault::decrypt_with_seed(self.vault_seed, &blob).ok())
            })
            .or_else(|| self.credential_secret(cred))
    }

    /// After `secrets.put` (`in_keyring` = it succeeded): keep the encrypted
    /// database copy only where the policy in `crate::secrets` wants it —
    /// the same rule as creating a session or vault entry.
    fn settle_secret(
        &self,
        target: crate::secrets::Fallback,
        secret: &str,
        in_keyring: bool,
    ) -> Result<(), String> {
        crate::secrets::settle_fallback(
            self.db,
            self.vault_seed,
            target,
            secret,
            in_keyring,
            self.portable,
        )
    }
}

fn is_local_setting(key: &str) -> bool {
    LOCAL_SETTING_PREFIXES.iter().any(|p| key.starts_with(p)) || LOCAL_SETTING_KEYS.contains(&key)
}

fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}

/// This install's device id, minted on first use, plus the hostname.
pub(crate) fn device_info(db: &Database) -> DeviceInfo {
    let id = match db.get_setting(DEVICE_ID_KEY) {
        Ok(Some(v)) if !v.trim().is_empty() => v,
        // A read error (database busy…) must not replace this install's
        // identity: use a throwaway id for this bundle only.
        Err(e) => {
            tracing::warn!("could not read device id: {e}");
            Uuid::new_v4().to_string()
        }
        Ok(_) => {
            let v = Uuid::new_v4().to_string();
            if let Err(e) = db.set_setting(DEVICE_ID_KEY, &v) {
                tracing::warn!("could not persist device id: {e}");
            }
            v
        }
    };
    let name = gethostname::gethostname().to_string_lossy().into_owned();
    DeviceInfo { id, name }
}

// ── build ───────────────────────────────────────────────────────────────────

/// Snapshot the environment + credentials + settings + known_hosts. Secrets
/// that can't be resolved on this device are left out and reported in
/// `warnings` rather than failing the whole backup.
pub(crate) fn build_bundle(
    ctx: &BackupCtx<'_>,
    app_version: &str,
    warnings: &mut Vec<String>,
) -> Result<Bundle, AppError> {
    let db = ctx.db;
    let environment = transfer::build_export(db, app_version)?;

    let settings: HashMap<String, String> = db
        .get_all_settings()
        .map_err(internal)?
        .into_iter()
        .filter(|(k, _)| !is_local_setting(k))
        .collect();

    let mut vault_entries = Vec::new();
    for cred in db.list_vault_entries().map_err(internal)? {
        match ctx.credential_secret(&cred) {
            Some(secret) => vault_entries.push(BundleVaultEntry {
                name: cred.name,
                username: cred.username,
                secret,
            }),
            None => warnings.push(format!(
                "vault entry «{}»: secret unavailable on this device — not included",
                cred.name
            )),
        }
    }

    // Session credentials come from the DB rows (which still carry
    // `credential_id`); `environment.sessions` has it stripped but keeps the
    // same ids, so `session_id` below is file-local by construction.
    let mut session_credentials = Vec::new();
    for session in db.list_sessions().map_err(internal)? {
        let Some(cred_id) = session.credential_id else {
            continue;
        };
        let Ok(cred) = db.get_credential(cred_id) else {
            continue;
        };
        if cred.reusable {
            session_credentials.push(BundleSessionCredential {
                session_id: session.id,
                vault_name: Some(cred.name),
                kind: None,
                secret: None,
            });
            continue;
        }
        match ctx.session_secret(&session, &cred) {
            Some(secret) => session_credentials.push(BundleSessionCredential {
                session_id: session.id,
                vault_name: None,
                kind: Some(cred.kind),
                secret: Some(secret),
            }),
            None => warnings.push(format!(
                "session «{}»: password unavailable on this device — not included",
                session.name
            )),
        }
    }

    let known_hosts = ctx
        .known_hosts_file()
        .and_then(|p| std::fs::read_to_string(p).ok());

    Ok(Bundle {
        zapx_bundle: BUNDLE_VERSION,
        app_version: app_version.to_owned(),
        created_at: chrono::Utc::now().to_rfc3339(),
        device: device_info(db),
        environment,
        settings,
        vault_entries,
        session_credentials,
        known_hosts,
    })
}

// ── apply ───────────────────────────────────────────────────────────────────

pub(crate) fn apply_bundle(
    ctx: &BackupCtx<'_>,
    bundle: &Bundle,
) -> Result<RestoreSummary, AppError> {
    apply_bundle_with(ctx, bundle, false)
}

/// [`apply_bundle`]; with `update_existing` the bundle also wins on items
/// both sides have (fields, vault passwords, session passwords). Only for a
/// sync pull onto a device with no changes since the last sync — there every
/// difference is an edit made on another device.
pub(crate) fn apply_bundle_with(
    ctx: &BackupCtx<'_>,
    bundle: &Bundle,
    update_existing: bool,
) -> Result<RestoreSummary, AppError> {
    if bundle.zapx_bundle > BUNDLE_VERSION {
        return Err(AppError::Internal(format!(
            "the backup was made by a newer ZapTerm (bundle v{}, this build reads v{BUNDLE_VERSION})",
            bundle.zapx_bundle
        )));
    }
    let db = ctx.db;
    let mut summary = RestoreSummary {
        environment: transfer::apply_import_with(db, &bundle.environment, update_existing)?,
        ..Default::default()
    };

    // 1. Vault entries by name — before sessions so links resolve. An entry
    // this device already has keeps its own password (the bundle's may be
    // older — a sync pull must not undo a rotation made here); only a
    // missing secret is filled in. In update mode the bundle's is newer.
    let existing = db.list_vault_entries().map_err(internal)?;
    for entry in &bundle.vault_entries {
        if let Some(ex) = existing.iter().find(|e| e.name == entry.name) {
            if update_existing && ex.username != entry.username {
                db.update_credential_meta(ex.id, &ex.name, entry.username.as_deref())
                    .map_err(internal)?;
                summary.vault_updated += 1;
            }
            match ctx.credential_secret(ex) {
                Some(local) if local == entry.secret => {}
                Some(_) if !update_existing => summary.warnings.push(format!(
                    "vault entry «{}»: kept this device's password (the backup has a different one)",
                    entry.name
                )),
                _ => {
                    let in_keyring = ctx.secrets.put(&ex.keyring_key, &entry.secret).is_ok();
                    match ctx.settle_secret(
                        crate::secrets::Fallback::Credential(ex.id),
                        &entry.secret,
                        in_keyring,
                    ) {
                        Ok(()) => summary.vault_updated += 1,
                        Err(e) => summary
                            .warnings
                            .push(format!("vault entry «{}»: {e}", entry.name)),
                    }
                }
            }
        } else {
            let key = format!("vault:{}", Uuid::new_v4());
            let in_keyring = ctx.secrets.put(&key, &entry.secret).is_ok();
            let id = match db.create_vault_entry(&entry.name, entry.username.as_deref(), &key) {
                Ok(id) => id,
                Err(e) => {
                    ctx.secrets.delete(&key);
                    summary
                        .warnings
                        .push(format!("vault entry «{}»: {e}", entry.name));
                    continue;
                }
            };
            if let Err(e) = ctx.settle_secret(
                crate::secrets::Fallback::Credential(id),
                &entry.secret,
                in_keyring,
            ) {
                db.delete_credential(id).ok();
                summary
                    .warnings
                    .push(format!("vault entry «{}»: {e}", entry.name));
                continue;
            }
            summary.vault_added += 1;
        }
    }

    // 2. Session credentials — only onto sessions that have none here.
    for sc in &bundle.session_credentials {
        let Some(&new_id) = summary.environment.session_map.get(&sc.session_id) else {
            summary.warnings.push(format!(
                "credential for session id {}: session not found in the backup",
                sc.session_id
            ));
            continue;
        };
        let session = db.get_session(new_id).map_err(internal)?;
        if let Some(cred_id) = session.credential_id {
            // Keep whatever this device already has — unless the bundle is
            // known to be newer, then follow its change.
            if update_existing {
                update_session_credential(ctx, &session, cred_id, sc, &mut summary)?;
            }
            continue;
        }
        if let Some(name) = &sc.vault_name {
            match db.get_credential_by_name(name).map_err(internal)? {
                Some(cred) => {
                    db.set_session_auth(new_id, Some(cred.id), "password")
                        .map_err(internal)?;
                    summary.credentials_linked += 1;
                }
                None => summary.warnings.push(format!(
                    "session «{}»: vault entry «{name}» not available — left without credential",
                    session.name
                )),
            }
            continue;
        }
        let Some(secret) = sc.secret.as_deref() else {
            continue;
        };
        let kind = sc.kind.as_deref().unwrap_or("ssh_password");
        let key = format!("ssh:{}", Uuid::new_v4());
        let in_keyring = ctx.secrets.put(&key, secret).is_ok();
        let cred_id = db
            .create_credential(&session.name, kind, session.username.as_deref(), &key)
            .map_err(internal)?;
        let method = session.auth_method.clone().unwrap_or_else(|| {
            if kind == "ssh_key" {
                "key".into()
            } else {
                "password".into()
            }
        });
        db.set_session_auth(new_id, Some(cred_id), &method)
            .map_err(internal)?;
        if let Some(cache) = ctx.password_cache {
            cache.lock().unwrap().insert(new_id, secret.to_owned());
        }
        if let Err(e) = ctx.settle_secret(
            crate::secrets::Fallback::Session(new_id),
            secret,
            in_keyring,
        ) {
            summary
                .warnings
                .push(format!("session «{}»: {e}", session.name));
            continue;
        }
        summary.credentials_linked += 1;
    }

    // 3. Settings — bundle wins, except install-local keys.
    for (k, v) in &bundle.settings {
        if is_local_setting(k) {
            continue;
        }
        db.set_setting(k, v).map_err(internal)?;
        summary.settings_applied += 1;
    }

    // 4. known_hosts — merge, never replace.
    if let Some(text) = bundle.known_hosts.as_deref() {
        match ctx.known_hosts_file() {
            Some(path) => match core_transport::merge_known_hosts_text(text, &path) {
                Ok(n) => summary.known_hosts_added = n,
                Err(e) => summary
                    .warnings
                    .push(format!("known_hosts not merged: {e}")),
            },
            None => summary
                .warnings
                .push("known_hosts not merged: no home directory".into()),
        }
    }

    Ok(summary)
}

/// Update mode: bring an existing session credential in line with the
/// bundle — re-link to another vault entry, or take a rotated private
/// password. Other shapes (private ↔ vault) are left as they are.
fn update_session_credential(
    ctx: &BackupCtx<'_>,
    session: &SavedSession,
    cred_id: i64,
    sc: &BundleSessionCredential,
    summary: &mut RestoreSummary,
) -> Result<(), AppError> {
    let db = ctx.db;
    let Ok(cred) = db.get_credential(cred_id) else {
        return Ok(());
    };
    match (&sc.vault_name, sc.secret.as_deref(), cred.reusable) {
        (Some(name), _, true) if *name != cred.name => {
            match db.get_credential_by_name(name).map_err(internal)? {
                Some(target) => {
                    let method = session.auth_method.as_deref().unwrap_or("password");
                    db.set_session_auth(session.id, Some(target.id), method)
                        .map_err(internal)?;
                    summary.credentials_linked += 1;
                }
                None => summary.warnings.push(format!(
                    "session «{}»: vault entry «{name}» not available — kept the current one",
                    session.name
                )),
            }
        }
        (None, Some(secret), false) => {
            if ctx.session_secret(session, &cred).as_deref() == Some(secret) {
                return Ok(());
            }
            let in_keyring = ctx.secrets.put(&cred.keyring_key, secret).is_ok();
            if let Some(cache) = ctx.password_cache {
                cache.lock().unwrap().insert(session.id, secret.to_owned());
            }
            match ctx.settle_secret(
                crate::secrets::Fallback::Session(session.id),
                secret,
                in_keyring,
            ) {
                Ok(()) => summary.credentials_linked += 1,
                Err(e) => summary
                    .warnings
                    .push(format!("session «{}»: {e}", session.name)),
            }
        }
        _ => {}
    }
    Ok(())
}

// ── file I/O ────────────────────────────────────────────────────────────────

pub(crate) fn validate_passphrase(passphrase: &str) -> Result<(), AppError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(AppError::Internal(format!(
            "the passphrase must be at least {MIN_PASSPHRASE_CHARS} characters"
        )));
    }
    Ok(())
}

/// Serialise + seal + write atomically (temp file in the same directory, then
/// rename) so a synced folder never sees a half-written bundle.
pub(crate) fn write_bundle(path: &Path, passphrase: &str, bundle: &Bundle) -> Result<(), AppError> {
    let mut json = serde_json::to_vec(bundle).map_err(internal)?;
    let sealed = core_vault::seal(passphrase, &json);
    json.zeroize();
    let sealed = sealed.map_err(internal)?;
    // Temp name unique to this device+process (a shared sync folder may see
    // two devices push at once), contents synced to disk before the rename
    // (a power cut must not leave a zero-length bundle in place of the old).
    let tmp = path.with_extension(format!(
        "{BUNDLE_EXTENSION}.{}.{}.tmp",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let written = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&sealed)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(internal(format!("{}: {e}", tmp.display())));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(internal(format!("{}: {e}", path.display())));
    }
    Ok(())
}

/// Read + open + parse. Wrong passphrase and corruption both come back as a
/// single clear error (the AEAD can't tell them apart, by design).
pub(crate) fn read_bundle(path: &Path, passphrase: &str) -> Result<Bundle, AppError> {
    let sealed = std::fs::read(path).map_err(|e| internal(format!("{}: {e}", path.display())))?;
    let mut json = match core_vault::open(passphrase, &sealed) {
        Ok(j) => j,
        Err(core_vault::Error::BadCiphertext) => {
            return Err(internal("not a ZapTerm backup file"));
        }
        Err(_) => return Err(internal("wrong passphrase or damaged backup")),
    };
    let parsed = serde_json::from_slice::<Bundle>(&json)
        .map_err(|e| internal(format!("backup contents unreadable: {e}")));
    json.zeroize();
    parsed
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn backup_export(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> Result<BackupExportSummary, AppError> {
    validate_passphrase(&passphrase)?;
    let store = KeyringStore;
    let ctx = BackupCtx::from_state(&state, &store);
    let version = app.package_info().version.to_string();
    let mut warnings = Vec::new();
    let bundle = build_bundle(&ctx, &version, &mut warnings)?;
    write_bundle(Path::new(&path), &passphrase, &bundle)?;
    Ok(BackupExportSummary {
        path,
        info: bundle.info(),
        warnings,
    })
}

/// Decrypt a bundle and describe what it holds, without touching the DB.
#[tauri::command]
pub async fn backup_inspect(path: String, passphrase: String) -> Result<BundleInfo, AppError> {
    Ok(read_bundle(Path::new(&path), &passphrase)?.info())
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> Result<RestoreSummary, AppError> {
    let bundle = read_bundle(Path::new(&path), &passphrase)?;
    let store = KeyringStore;
    let ctx = BackupCtx::from_state(&state, &store);
    // Not while the background sync is fingerprinting/pushing or a pull is
    // applying: it would publish a half-restored state, or both could create
    // the same sessions.
    let summary = {
        let _g = super::sync::SYNC_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        apply_bundle(&ctx, &bundle)?
    };
    if summary.environment.rules_added > 0 {
        super::highlight::rebuild_highlighter(&state);
    }
    super::settings::apply_persisted_keepalive(&state.db);
    Ok(summary)
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// In-memory stand-in for the OS keyring.
    #[derive(Default)]
    struct MemStore(Mutex<HashMap<String, String>>);

    impl SecretStore for MemStore {
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

    struct Fixture {
        dir: PathBuf,
        db: Database,
        store: MemStore,
        cache: PasswordCache,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "zapterm-backup-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let db = Database::open(&dir.join("zapterm.db")).expect("open temp db");
            Self {
                dir,
                db,
                store: MemStore::default(),
                cache: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn ctx(&self) -> BackupCtx<'_> {
            BackupCtx {
                db: &self.db,
                secrets: &self.store,
                vault_seed: "test-seed",
                password_cache: Some(&self.cache),
                portable: false,
                known_hosts_path: Some(self.dir.join("known_hosts")),
            }
        }

        fn add_ssh(&self, name: &str, auth: &str, cred: Option<i64>) -> i64 {
            self.db
                .create_session_full(
                    None,
                    name,
                    "ssh",
                    Some(&format!("{name}.example")),
                    Some(22),
                    Some("admin"),
                    cred,
                    "{}",
                    Some(auth),
                    None,
                )
                .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// The full path: build on one "device", serialise, seal, open on another,
    /// apply — and every credential, setting and host key comes back attached
    /// to the right session. A second apply is a no-op except for the
    /// by-name vault refresh.
    #[test]
    fn roundtrip_restores_everything() {
        let src = Fixture::new("src");
        // Vault entry with its secret in the "keyring".
        src.store.put("vault:1", "vault-secret").unwrap();
        let vault_id = src
            .db
            .create_vault_entry("prod-root", Some("root"), "vault:1")
            .unwrap();
        // A: private password (keyring). B: linked to the vault entry.
        // C: key auth with a private passphrase, resolvable only through the
        // in-memory cache (simulates a keyring-denied dev build). D: agent.
        src.store.put("ssh:a", "pwA").unwrap();
        let cred_a = src
            .db
            .create_credential("A", "ssh_password", Some("admin"), "ssh:a")
            .unwrap();
        let a = src.add_ssh("A", "password", Some(cred_a));
        let _b = src.add_ssh("B", "password", Some(vault_id));
        let cred_c = src
            .db
            .create_credential("C", "ssh_key", Some("admin"), "ssh:c")
            .unwrap();
        let c = src.add_ssh("C", "key", Some(cred_c));
        src.cache.lock().unwrap().insert(c, "ppC".into());
        let _d = src.add_ssh("D", "agent", None);
        src.db.set_setting("ui.theme", "fjord").unwrap();
        src.db.set_setting("backup.device_id", "dev-src").unwrap();
        std::fs::write(
            src.dir.join("known_hosts"),
            "A.example ssh-ed25519 AAAA\nC.example ssh-ed25519 CCCC\n",
        )
        .unwrap();

        let mut warnings = Vec::new();
        let bundle = build_bundle(&src.ctx(), "9.9.9", &mut warnings).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(bundle.vault_entries.len(), 1);
        assert_eq!(bundle.vault_entries[0].secret, "vault-secret");
        assert_eq!(bundle.session_credentials.len(), 3);
        assert!(bundle.session_credentials.iter().any(|s| s.session_id == a
            && s.kind.as_deref() == Some("ssh_password")
            && s.secret.as_deref() == Some("pwA")));
        assert!(bundle
            .session_credentials
            .iter()
            .any(|s| s.vault_name.as_deref() == Some("prod-root")));
        assert!(bundle.session_credentials.iter().any(|s| s.session_id == c
            && s.kind.as_deref() == Some("ssh_key")
            && s.secret.as_deref() == Some("ppC")));
        assert_eq!(
            bundle.settings.get("ui.theme").map(String::as_str),
            Some("fjord")
        );
        assert!(!bundle.settings.contains_key("backup.device_id"));
        assert_eq!(bundle.info().known_hosts_lines, 2);
        assert_eq!(bundle.device.id, "dev-src");

        // Seal to disk and open again, as the commands do.
        let file = src.dir.join("backup.zapxb");
        write_bundle(&file, "correct horse battery", &bundle).unwrap();
        assert!(!src.dir.join("backup.zapxb.tmp").exists());
        assert!(read_bundle(&file, "wrong passphrase!").is_err());
        let opened = read_bundle(&file, "correct horse battery").unwrap();
        assert_eq!(opened.info().sessions, 4);

        // Restore on a fresh device.
        let dst = Fixture::new("dst");
        dst.db.set_setting("backup.device_id", "dev-dst").unwrap();
        let s = apply_bundle(&dst.ctx(), &opened).unwrap();
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        assert_eq!(s.environment.sessions_added, 4);
        assert_eq!(s.vault_added, 1);
        assert_eq!(s.vault_updated, 0);
        assert_eq!(s.credentials_linked, 3);
        // Every bundled setting is applied (the DB seeds defaults on open, so
        // the count is "ui.theme plus whatever the schema ships").
        assert_eq!(s.settings_applied, opened.settings.len());
        assert_eq!(s.known_hosts_added, 2);

        let sessions = dst.db.list_sessions().unwrap();
        let find = |n: &str| sessions.iter().find(|s| s.name == n).unwrap().clone();
        let na = find("A");
        let ca = dst.db.get_credential(na.credential_id.unwrap()).unwrap();
        assert!(!ca.reusable);
        assert_eq!(ca.kind, "ssh_password");
        assert_eq!(dst.store.get(&ca.keyring_key).as_deref(), Some("pwA"));
        assert_eq!(
            dst.cache.lock().unwrap().get(&na.id).map(String::as_str),
            Some("pwA")
        );
        let nb = find("B");
        let cb = dst.db.get_credential(nb.credential_id.unwrap()).unwrap();
        assert!(cb.reusable);
        assert_eq!(cb.name, "prod-root");
        assert_eq!(cb.username.as_deref(), Some("root"));
        assert_eq!(
            dst.store.get(&cb.keyring_key).as_deref(),
            Some("vault-secret")
        );
        let nc = find("C");
        assert_eq!(nc.auth_method.as_deref(), Some("key"));
        let cc = dst.db.get_credential(nc.credential_id.unwrap()).unwrap();
        assert_eq!(cc.kind, "ssh_key");
        assert_eq!(dst.store.get(&cc.keyring_key).as_deref(), Some("ppC"));
        assert!(find("D").credential_id.is_none());
        assert_eq!(
            dst.db.get_setting("ui.theme").unwrap().as_deref(),
            Some("fjord")
        );
        // The restoring device keeps its own identity.
        assert_eq!(
            dst.db.get_setting("backup.device_id").unwrap().as_deref(),
            Some("dev-dst")
        );
        assert_eq!(
            std::fs::read_to_string(dst.dir.join("known_hosts")).unwrap(),
            "A.example ssh-ed25519 AAAA\nC.example ssh-ed25519 CCCC\n"
        );

        // Second apply: nothing duplicated, existing credentials untouched.
        let s2 = apply_bundle(&dst.ctx(), &opened).unwrap();
        assert_eq!(s2.environment.sessions_added, 0);
        assert_eq!(s2.environment.sessions_skipped, 4);
        assert_eq!(s2.vault_added, 0);
        // Same password here already: nothing to update.
        assert_eq!(s2.vault_updated, 0);
        assert!(s2.warnings.is_empty(), "{:?}", s2.warnings);
        assert_eq!(s2.credentials_linked, 0);
        assert_eq!(s2.known_hosts_added, 0);
        assert_eq!(dst.db.list_vault_entries().unwrap().len(), 1);

        // A vault password changed on this device since the backup is kept
        // (a restore or sync pull must not undo a rotation), with a warning.
        let entry = dst.db.list_vault_entries().unwrap().remove(0);
        dst.store.put(&entry.keyring_key, "rotated-here").unwrap();
        let s3 = apply_bundle(&dst.ctx(), &opened).unwrap();
        assert_eq!(s3.vault_updated, 0);
        assert_eq!(
            dst.store.get(&entry.keyring_key).as_deref(),
            Some("rotated-here")
        );
        assert!(
            s3.warnings
                .iter()
                .any(|w| w.contains("kept this device's password")),
            "{:?}",
            s3.warnings
        );
    }

    /// A secret the device can't produce is a warning, not a failure — the
    /// rest of the backup is still written.
    #[test]
    fn unavailable_secret_is_a_warning() {
        let f = Fixture::new("warn");
        let cred =
            f.db.create_credential("gone", "ssh_password", None, "ssh:missing")
                .unwrap();
        f.add_ssh("gone", "password", Some(cred));
        let mut warnings = Vec::new();
        let bundle = build_bundle(&f.ctx(), "1", &mut warnings).unwrap();
        assert!(bundle.session_credentials.is_empty());
        assert_eq!(bundle.environment.sessions.len(), 1);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("gone"));
    }

    /// Restoring onto a session that already has a credential here leaves it
    /// alone, and a link to a vault entry missing from the bundle is reported.
    #[test]
    fn restore_never_overwrites_local_credentials() {
        let src = Fixture::new("src2");
        src.store.put("ssh:x", "from-backup").unwrap();
        let cred = src
            .db
            .create_credential("X", "ssh_password", None, "ssh:x")
            .unwrap();
        src.add_ssh("X", "password", Some(cred));
        let mut w = Vec::new();
        let mut bundle = build_bundle(&src.ctx(), "1", &mut w).unwrap();
        // Also a dangling vault link the destination can't resolve.
        bundle.session_credentials.push(BundleSessionCredential {
            session_id: bundle.environment.sessions[0].id,
            vault_name: Some("nope".into()),
            kind: None,
            secret: None,
        });

        let dst = Fixture::new("dst2");
        dst.store.put("ssh:local", "local-pw").unwrap();
        let local = dst
            .db
            .create_credential("X", "ssh_password", None, "ssh:local")
            .unwrap();
        // Same identity as the bundle's session → import skips it.
        dst.add_ssh("X", "password", Some(local));
        let s = apply_bundle(&dst.ctx(), &bundle).unwrap();
        assert_eq!(s.environment.sessions_skipped, 1);
        assert_eq!(s.credentials_linked, 0);
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        let x = dst.db.list_sessions().unwrap().remove(0);
        assert_eq!(x.credential_id, Some(local));
        assert_eq!(dst.store.get("ssh:local").as_deref(), Some("local-pw"));

        // On a fresh device the session's own credential comes first, so the
        // second (dangling) entry for the same session is simply skipped.
        let dst2 = Fixture::new("dst3");
        let s = apply_bundle(&dst2.ctx(), &bundle).unwrap();
        assert_eq!(s.credentials_linked, 1);
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);

        // With only the dangling vault link, the missing entry is reported.
        bundle
            .session_credentials
            .retain(|c| c.vault_name.is_some());
        let dst3 = Fixture::new("dst4");
        let s = apply_bundle(&dst3.ctx(), &bundle).unwrap();
        assert_eq!(s.credentials_linked, 0);
        assert_eq!(s.warnings.len(), 1, "{:?}", s.warnings);
        assert!(
            s.warnings[0].contains("«nope» not available"),
            "{:?}",
            s.warnings
        );
    }

    #[test]
    fn rejects_newer_bundle_and_short_passphrase() {
        let f = Fixture::new("ver");
        let mut w = Vec::new();
        let mut bundle = build_bundle(&f.ctx(), "1", &mut w).unwrap();
        bundle.zapx_bundle = BUNDLE_VERSION + 1;
        assert!(apply_bundle(&f.ctx(), &bundle).is_err());
        assert!(validate_passphrase("short").is_err());
        assert!(validate_passphrase("long enough").is_ok());
        assert!(matches!(
            read_bundle(&f.dir.join("nope.zapxb"), "whatever!"),
            Err(AppError::Internal(_))
        ));
        std::fs::write(f.dir.join("junk.zapxb"), b"not a bundle at all").unwrap();
        let err = read_bundle(&f.dir.join("junk.zapxb"), "whatever!").unwrap_err();
        assert!(err.to_string().contains("not a ZapTerm backup"));
    }
}
