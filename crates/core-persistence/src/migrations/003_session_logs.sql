-- Migration 003: session activity logs metadata
-- Log files live on disk; this table tracks them.

CREATE TABLE IF NOT EXISTS session_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    file_path   TEXT NOT NULL,
    bytes       INTEGER NOT NULL DEFAULT 0,
    format      TEXT NOT NULL DEFAULT 'raw'
);

CREATE INDEX IF NOT EXISTS idx_session_logs_session
    ON session_logs(session_id, started_at DESC);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);
