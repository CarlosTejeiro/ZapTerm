-- 029: the status-word rules also colour "enable" / "disable".
--
-- FortiOS (and others) report settings as "enable" / "disable" — as in
-- `get system status`: "Private Encryption: Disable", "FIPS-CC mode:
-- disable" — while the built-in rules only knew "enabled" / "disabled".
--
-- Each rule is changed only where it still has the pattern 026 gave it: a
-- rule the user edited, or deleted, is left alone.
UPDATE highlight_rules
   SET pattern = '(?i)\b(?:warnings|deprecated|degraded|disabled?|retry(?:ing)?|retransmit(?:s|ted)?|discard(?:s|ed)?|blocked|blocking)\b'
 WHERE pattern = '(?i)\b(?:warnings|deprecated|degraded|disabled|retry(?:ing)?|retransmit(?:s|ted)?|discard(?:s|ed)?|blocked|blocking)\b'
   AND is_regex = 1;

UPDATE highlight_rules
   SET pattern = '(?i)\b(?:ok|okay|success(?:ful(?:ly)?)?|succeeded|connected|established|enabled?|online|reachable|forwarding|running|active|full|accepted|permit(?:ted)?)\b'
 WHERE pattern = '(?i)\b(?:ok|okay|success(?:ful(?:ly)?)?|succeeded|connected|established|enabled|online|reachable|forwarding|running|active|full|accepted|permit(?:ted)?)\b'
   AND is_regex = 1;

INSERT OR IGNORE INTO schema_version (version) VALUES (29);
