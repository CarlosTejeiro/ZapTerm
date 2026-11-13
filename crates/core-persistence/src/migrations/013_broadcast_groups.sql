-- Migration 013 — broadcast groups (multi-host orchestration)
--
-- A broadcast group is a named, ordered set of saved sessions that the user
-- opens together as a grid and drives with one master input / command-runner.
-- Distinct from folders (a tree for organization): a session may belong to
-- many groups, membership order matters, and the group references sessions
-- rather than containing them.

CREATE TABLE broadcast_groups (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE broadcast_group_members (
    group_id    INTEGER NOT NULL REFERENCES broadcast_groups(id) ON DELETE CASCADE,
    session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, session_id)
);

CREATE INDEX idx_bgroup_members_group ON broadcast_group_members(group_id);

INSERT INTO schema_version (version) VALUES (13);
