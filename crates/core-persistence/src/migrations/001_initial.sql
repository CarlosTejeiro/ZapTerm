-- Migration 001 — initial schema
-- Credentials: opaque references into the OS keyring (never store secrets here)
CREATE TABLE credentials (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    username    TEXT,
    keyring_key TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Folders: tree of connection groups
CREATE TABLE folders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id  INTEGER REFERENCES folders(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sessions: saved connections
CREATE TABLE sessions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id     INTEGER REFERENCES folders(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    protocol      TEXT NOT NULL CHECK (protocol IN ('ssh','telnet','serial','local')),
    host          TEXT,
    port          INTEGER,
    username      TEXT,
    credential_id INTEGER REFERENCES credentials(id) ON DELETE SET NULL,
    options_json  TEXT NOT NULL DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at  TEXT
);

CREATE INDEX idx_sessions_folder    ON sessions(folder_id);
CREATE INDEX idx_sessions_last_used ON sessions(last_used_at DESC);

-- Schema version tracking
CREATE TABLE schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO schema_version (version) VALUES (1);
