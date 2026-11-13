-- 016: command-sequence learning.
--
-- command_history aggregates by command (freq + last_used) and loses order,
-- so it can't tell us "after A you usually run B". This table records those
-- A→B transitions, keyed by platform so the learning is shared across every
-- session of the same vendor (e.g. all Cisco IOS devices). `freq` is bumped
-- each time `next` follows `prev`; `last_used` powers recency decay.

CREATE TABLE command_transitions (
    platform   TEXT    NOT NULL,
    prev       TEXT    NOT NULL,
    next       TEXT    NOT NULL,
    freq       INTEGER NOT NULL DEFAULT 1,
    last_used  TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (platform, prev, next)
);

CREATE INDEX idx_cmd_transitions_lookup
  ON command_transitions(platform, prev, freq DESC);

INSERT OR IGNORE INTO schema_version(version) VALUES (16);
