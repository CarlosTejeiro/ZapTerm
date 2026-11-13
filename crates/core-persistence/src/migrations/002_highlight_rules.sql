-- Migration 002 — keyword highlight rules
CREATE TABLE highlight_rules (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    pattern    TEXT NOT NULL,
    is_regex   INTEGER NOT NULL DEFAULT 1,
    fg_color   TEXT,
    bg_color   TEXT,
    bold       INTEGER NOT NULL DEFAULT 0,
    underline  INTEGER NOT NULL DEFAULT 0,
    enabled    INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_highlight_sort ON highlight_rules(sort_order);

-- 10 built-in rules useful for network engineers
INSERT INTO highlight_rules (name, pattern, is_regex, fg_color, bold, sort_order) VALUES
    ('Cisco syslog error',  '%[A-Z_]+-[0-9]-[A-Z_]+', 1, '#ef4444', 1, 0),
    ('Error keyword',       '\b[Ee]rror\b',             1, '#ef4444', 1, 1),
    ('Warning keyword',     '\b[Ww]arning\b',           1, '#eab308', 1, 2),
    ('Interface down',      '\bdown\b',                 1, '#ef4444', 0, 3),
    ('Interface up',        '\bup\b',                   1, '#22c55e', 0, 4),
    ('IPv4 address',        '\b(?:\d{1,3}\.){3}\d{1,3}\b', 1, '#60a5fa', 0, 5),
    ('IPv6 address',        '\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{0,4}\b', 1, '#38bdf8', 0, 6),
    ('MAC (Cisco dotted)',  '\b[0-9a-fA-F]{4}\.[0-9a-fA-F]{4}\.[0-9a-fA-F]{4}\b', 1, '#c084fc', 0, 7),
    ('MAC (colon)',         '\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b',           1, '#c084fc', 0, 8),
    ('VLAN number',         '\b[Vv][Ll][Aa][Nn]\s+\d+\b', 1, '#fbbf24', 0, 9);

INSERT INTO schema_version (version) VALUES (2);
