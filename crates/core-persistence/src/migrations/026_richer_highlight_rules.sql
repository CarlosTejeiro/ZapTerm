-- 026: richer built-in highlight rules.
--
-- Adapted (rewritten for the Rust `regex` crate, which has no look-around or
-- back-references) from two open-source colourisers:
--   * grc by Radovan Garabík (GPL-2.0-or-later): the df usage tiers
--     0–69 % / 70–89 % / 90–97 % / 98–100 %.
--   * ChromaTerm by Saria H. (MIT): the size, date/time, URL and good/bad
--     status-word rules.
--
-- Rules are appended after the user's own (a match already taken by an
-- earlier rule is never re-coloured) and only when no rule with the same
-- pattern exists, so running this on a database that already has them, or
-- syncing with a device that does, never duplicates anything.

-- The original IPv6 rule also matched times such as 12:05:11. Replace it,
-- only where the user hasn't edited it, with one that needs either "::" or
-- all eight groups (so MAC addresses and times no longer match either).
UPDATE highlight_rules
   SET pattern = '\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b|\b(?:[0-9a-fA-F]{1,4}:){1,7}:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4}){0,6})?(?:/\d{1,3})?|::(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4}){0,6})?(?:/\d{1,3})?'
 WHERE pattern = '\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{0,4}\b'
   AND is_regex = 1;

CREATE TEMP TABLE new_rules (
    pos      INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    pattern  TEXT NOT NULL,
    fg_color TEXT NOT NULL,
    bold     INTEGER NOT NULL,
    underline INTEGER NOT NULL
);

INSERT INTO new_rules (pos, name, pattern, fg_color, bold, underline) VALUES
    (1,  'Usage ≥98%',   '\b(?:9[89]|100)(?:[.,]\d+)?%', '#ef4444', 1, 0),
    (2,  'Usage 90–97%', '\b9[0-7](?:[.,]\d+)?%',        '#f87171', 0, 0),
    (3,  'Usage 70–89%', '\b[78]\d(?:[.,]\d+)?%',        '#eab308', 0, 0),
    (4,  'Usage <70%',   '\b[1-6]?\d(?:[.,]\d+)?%',      '#22c55e', 0, 0),
    (5,  'Size',
         '\b\d+(?:[.,]\d+)?(?:[kKMGTPE]i?(?:[Bb](?:ps|/s)?)?|Bi?|bps)\b|\b\d+(?:[.,]\d+)?\s(?:[kKMGTPE]i?B|[kKMGT]bps|bytes)\b',
         '#f0abfc', 0, 0),
    (6,  'Date',
         '\b(?:\d{4}[-/](?:0[1-9]|1[0-2])[-/](?:0[1-9]|[12]\d|3[01])|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(?:[12]\d|3[01]|0?[1-9]))\b',
         '#fdba74', 0, 0),
    (7,  'Time',
         '\b(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:[.,]\d{1,6})?)?\b',
         '#fdba74', 0, 0),
    (8,  'URL',
         '\b(?:https?|ftps?|sftp|ssh|telnet)://[^\s''"<>]+',
         '#60a5fa', 0, 1),
    (9,  'Unix path',
         '(?:^|\s)~?/[\w.@+-]+(?:/[\w.@+-]+)*/?',
         '#5eead4', 0, 0),
    (10, 'Status: bad',
         '(?i)\b(?:errors|err-disabled|fail(?:s|ed|ures?)?|denied|deny|reject(?:s|ed)?|drop(?:s|ped)?|refused|time[ds]?[- ]?out|unreachable|invalid|mismatch|collisions?|critical|fatal|panic|disconnected|notconnect|offline|lost|abnormal)\b',
         '#f87171', 0, 0),
    (11, 'Status: warning',
         '(?i)\b(?:warnings|deprecated|degraded|disabled|retry(?:ing)?|retransmit(?:s|ted)?|discard(?:s|ed)?|blocked|blocking)\b',
         '#facc15', 0, 0),
    (12, 'Status: good',
         '(?i)\b(?:ok|okay|success(?:ful(?:ly)?)?|succeeded|connected|established|enabled|online|reachable|forwarding|running|active|full|accepted|permit(?:ted)?)\b',
         '#4ade80', 0, 0);

INSERT INTO highlight_rules (name, pattern, is_regex, fg_color, bold, underline, sort_order)
SELECT n.name, n.pattern, 1, n.fg_color, n.bold, n.underline,
       (SELECT COALESCE(MAX(sort_order), 0) FROM highlight_rules) + n.pos
  FROM new_rules n
 WHERE NOT EXISTS (SELECT 1 FROM highlight_rules h WHERE h.pattern = n.pattern);

DROP TABLE new_rules;

INSERT OR IGNORE INTO schema_version (version) VALUES (26);
