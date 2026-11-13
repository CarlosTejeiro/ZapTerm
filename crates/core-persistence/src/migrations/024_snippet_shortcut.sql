-- 024: optional keyboard shortcut per snippet / macro.
--
-- A combo string in the same form as the app's editable shortcuts
-- (e.g. "Ctrl+Alt+P"). NULL = no shortcut. Uniqueness and clashes with app
-- shortcuts are validated in the editor, not here.
ALTER TABLE snippets ADD COLUMN shortcut TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (24);
