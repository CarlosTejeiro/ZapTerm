-- Output triggers, stored as a JSON array on the session (like login_script_json).
ALTER TABLE sessions ADD COLUMN triggers_json TEXT;
INSERT OR IGNORE INTO schema_version(version) VALUES (17);
