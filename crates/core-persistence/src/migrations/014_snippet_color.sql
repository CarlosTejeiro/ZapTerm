-- 014: per-snippet color.
--
-- The bottom bar now doubles as a user-curated button bar (SecureCRT-style).
-- An optional accent color lets the user visually group buttons (e.g. green
-- for safe show-commands, red for disruptive ones). NULL = default theme tint.

ALTER TABLE snippets ADD COLUMN color TEXT;

INSERT OR IGNORE INTO schema_version(version) VALUES (14);
