-- 025: optional name per saved port-forward ("FortiGate GUI", "RDP jump"…),
-- shown in the session dialog and the active-tunnel lists. NULL = unnamed.
ALTER TABLE session_forwards ADD COLUMN name TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (25);
