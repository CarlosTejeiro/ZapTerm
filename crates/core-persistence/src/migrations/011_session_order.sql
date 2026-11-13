-- Per-session sort order so the user can drag-reorder rows inside a folder
-- (or at the root). Default to id-derived ordering so existing rows keep
-- the previous "newest first" feel until they get re-arranged.

ALTER TABLE sessions ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
UPDATE sessions SET sort_order = id WHERE sort_order = 0;

CREATE INDEX IF NOT EXISTS idx_sessions_folder_order
  ON sessions(folder_id, sort_order);

INSERT OR IGNORE INTO schema_version(version) VALUES (11);
