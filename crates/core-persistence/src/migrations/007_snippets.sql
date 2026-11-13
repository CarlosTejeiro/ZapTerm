-- 007: command snippets.
--
-- Snippets are short, named text blocks the user can send to the focused
-- terminal with one click. Useful for network engineers who repeat common
-- commands across many devices (show interface, write memory, ...).
CREATE TABLE snippets (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    content    TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_snippets_sort ON snippets(sort_order, name);

-- A handful of useful seeds for network ops; the user can delete or rename.
INSERT INTO snippets (name, content, sort_order) VALUES
    ('show interface description', 'show interface description' || char(10), 1),
    ('show ip bgp summary',        'show ip bgp summary' || char(10),        2),
    ('show running-config',        'show running-config' || char(10),        3),
    ('configure terminal',         'configure terminal' || char(10),         4),
    ('write memory',               'write memory' || char(10),               5);

INSERT OR IGNORE INTO schema_version (version) VALUES (7);
