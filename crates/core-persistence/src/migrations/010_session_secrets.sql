-- Migration 010 — local fallback for credentials when the OS keyring is
-- unusable (typical on macOS dev builds: the unsigned binary is denied read
-- access to its own Keychain entry).
--
-- Ciphertext is AES-256-GCM, encrypted with a key derived from the app's
-- data-directory path (see core_vault::encrypt_with_seed). The keyring
-- remains the *primary* store; this table is consulted only on retrieval
-- failure and exists so the user doesn't have to re-enter the password
-- every time the dev binary is rebuilt.

CREATE TABLE IF NOT EXISTS session_secrets (
    session_id  INTEGER PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    ciphertext  BLOB    NOT NULL,
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (10);
