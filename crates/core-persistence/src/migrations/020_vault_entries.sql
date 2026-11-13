-- Reusable credential entries (the "credential vault"). Extends the existing
-- `credentials` table rather than introducing a new one: a reusable entry is
-- just a credential row flagged `reusable=1`, so it can be referenced by many
-- sessions and survives any single session's deletion.
ALTER TABLE credentials ADD COLUMN reusable INTEGER NOT NULL DEFAULT 0;

INSERT OR IGNORE INTO schema_version(version) VALUES (20);
