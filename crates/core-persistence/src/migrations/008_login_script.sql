-- 008: optional automated login script per saved session.
--
-- Stored as a JSON array of `{ "expect": ..., "send": ..., "is_regex": false,
-- "timeout_ms": 10000 }` objects. NULL means no automation. The runner is
-- protocol-agnostic (works for SSH, Telnet and Serial).
ALTER TABLE sessions ADD COLUMN login_script_json TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (8);
