-- Migration 023 — local fallback for reusable vault credentials when the OS
-- keyring is unusable (typical on unsigned builds: the binary is denied read
-- access to its own keyring/Keychain entry). Mirrors `session_secrets`
-- (migration 010) but keyed by the credential row.
--
-- Ciphertext is AES-256-GCM, encrypted with a key derived from the app's
-- data-directory path (see core_vault::encrypt_with_seed). The keyring
-- remains the *primary* store; this table is consulted only on retrieval
-- failure so a vault entry keeps working on a keyring-denying build.

CREATE TABLE IF NOT EXISTS credential_secrets (
    credential_id INTEGER PRIMARY KEY REFERENCES credentials(id) ON DELETE CASCADE,
    ciphertext    BLOB    NOT NULL
);

INSERT OR IGNORE INTO schema_version (version) VALUES (23);
