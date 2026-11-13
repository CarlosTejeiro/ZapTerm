-- Migration 009 — command hints storage
-- Backs the ghost-text / IntelliSense popup feature.
--
-- Snippets are *not* declared here: they live in the table created by
-- 007_snippets.sql (managed by the dedicated snippets UI) and the hint
-- engine reads from it directly so a single source of truth feeds both
-- the snippets dialog and the popup.

-- Per-session command history. session_id NULL means "global pool"
-- (used by local PTY sessions that have no saved profile).
CREATE TABLE IF NOT EXISTS command_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
    command     TEXT NOT NULL,
    freq        INTEGER NOT NULL DEFAULT 1,
    last_used   TEXT NOT NULL DEFAULT (datetime('now')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- NULL values do not collide with each other in UNIQUE indexes, so two
-- partial uniques cover both the session-scoped and global pools.
CREATE UNIQUE INDEX IF NOT EXISTS idx_history_session_cmd
    ON command_history(session_id, command) WHERE session_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_history_global_cmd
    ON command_history(command) WHERE session_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_history_prefix
    ON command_history(session_id, command);

-- Per-session platform override. The default platform lives in the
-- settings key 'hints.default_platform'.
CREATE TABLE IF NOT EXISTS session_platforms (
    session_id   INTEGER PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    platform     TEXT NOT NULL,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('hints.ghost_enabled',     'true'),
    ('hints.popup_enabled',     'true'),
    ('hints.default_platform',  'generic');

INSERT OR IGNORE INTO schema_version (version) VALUES (9);
