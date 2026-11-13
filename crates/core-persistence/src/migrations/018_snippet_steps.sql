-- 018: snippets can be macros. A non-NULL `steps_json` holds an expect/send/
-- wait step array (same shape as login_script_json); NULL means a plain-text
-- snippet (the original behaviour).
ALTER TABLE snippets ADD COLUMN steps_json TEXT;
INSERT OR IGNORE INTO schema_version(version) VALUES (18);
