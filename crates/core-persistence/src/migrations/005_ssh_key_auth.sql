-- 005: per-session SSH auth method.
--
-- NULL/'password' = password from vault (legacy behaviour).
-- 'key'   = public-key auth; key_path lives in options_json, passphrase in vault.
-- 'agent' = SSH agent; no secret stored.
ALTER TABLE sessions ADD COLUMN auth_method TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (5);
