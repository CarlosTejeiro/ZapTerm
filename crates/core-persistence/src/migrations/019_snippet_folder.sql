-- 019: optional folder for snippets/macros.
--
-- Lets the sidebar Macros library group entries under a named folder. NULL =
-- ungrouped (shown at the section root). Free-text, single level.
ALTER TABLE snippets ADD COLUMN folder TEXT;

INSERT OR IGNORE INTO schema_version (version) VALUES (19);
