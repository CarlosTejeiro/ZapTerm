-- 012: per-platform snippets.
--
-- A snippet can now be scoped to one platform (Cisco IOS, FortiGate, Linux,
-- ...) so the bottom button bar only surfaces relevant ones in each session.
-- NULL `platform` means the snippet is global (shown in every session).
--
-- The seed rows from migration 007 (`show interface description`,
-- `show running-config`, ...) are all Cisco-style; tag them as cisco_ios so
-- they don't pollute a Linux tab. Users who'd kept them as "general" can
-- always edit and clear the platform.

ALTER TABLE snippets ADD COLUMN platform TEXT;

UPDATE snippets SET platform = 'cisco_ios'
 WHERE name IN (
   'show interface description',
   'show ip bgp summary',
   'show running-config',
   'configure terminal',
   'write memory'
 );

CREATE INDEX IF NOT EXISTS idx_snippets_platform
  ON snippets(platform, sort_order, name);

INSERT OR IGNORE INTO schema_version(version) VALUES (12);
