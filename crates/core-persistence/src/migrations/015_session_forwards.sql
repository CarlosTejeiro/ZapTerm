-- 015: per-session port-forwards.
--
-- Forwards (-L local, -D dynamic SOCKS5, -R remote) saved on an SSH session
-- so they auto-start when it connects, matching SecureCRT/MobaXterm "Port
-- Forwarding" config. `target_*` are NULL for dynamic (SOCKS5) forwards.

CREATE TABLE session_forwards (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,         -- 'local' | 'dynamic' | 'remote'
    bind_addr   TEXT    NOT NULL,
    bind_port   INTEGER NOT NULL,
    target_host TEXT,                     -- NULL for dynamic
    target_port INTEGER,                  -- NULL for dynamic
    sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_session_forwards_session
  ON session_forwards(session_id, sort_order);

INSERT OR IGNORE INTO schema_version(version) VALUES (15);
