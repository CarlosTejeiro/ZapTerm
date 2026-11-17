-- Migration 027 — the encrypted vault.
--
-- Every stored secret (a session's password or key passphrase, a vault
-- entry's password, the sync passphrase) is one row here: `key` is the
-- reference the rest of the schema already holds (`credentials.keyring_key`,
-- e.g. `ssh:<uuid>` / `vault:<uuid>`, or a fixed name), `ciphertext` is
-- AES-256-GCM under the vault key with `key` as associated data, so a blob
-- copied to another row does not decrypt (see core_vault::master).
--
-- This replaces the OS keyring and the `vault.key`-seed copies in
-- `session_secrets` / `credential_secrets`; those are read once more to move
-- their content here.

CREATE TABLE IF NOT EXISTS vault_secrets (
    key        TEXT PRIMARY KEY,
    ciphertext BLOB NOT NULL
);

INSERT OR IGNORE INTO schema_version (version) VALUES (27);
