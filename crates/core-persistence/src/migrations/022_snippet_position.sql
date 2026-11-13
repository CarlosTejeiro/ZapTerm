-- Per-folder ordering for macros in the sidebar Macros library. `position` is
-- a nullable rank within the snippet's folder group (NULLs sort last, then by
-- id/name as before). Distinct from `sort_order`, which the bottom button bar
-- uses for the global visible order — macros are reordered independently of the
-- bar so their sidebar arrangement doesn't perturb the bar slots.
ALTER TABLE snippets ADD COLUMN position INTEGER;

INSERT OR IGNORE INTO schema_version(version) VALUES (22);
