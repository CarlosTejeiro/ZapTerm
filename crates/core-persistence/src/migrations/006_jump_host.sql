-- 006: optional jump-host (ProxyJump) reference.
--
-- `via_session_id` points to another saved session (the bastion) through which
-- this session is reached. NULL means a direct connection. On bastion deletion
-- the reference is cleared (SET NULL) rather than cascading the dependent sessions.
ALTER TABLE sessions ADD COLUMN via_session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL;

INSERT OR IGNORE INTO schema_version (version) VALUES (6);
