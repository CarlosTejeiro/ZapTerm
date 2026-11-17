-- 028: the OS version a session's device reported (e.g. "7.4.3" from
-- FortiOS `get system status`), next to its platform. Picks the command
-- lists written for that release. NULL = not seen yet.
ALTER TABLE session_platforms ADD COLUMN os_version TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (28);
