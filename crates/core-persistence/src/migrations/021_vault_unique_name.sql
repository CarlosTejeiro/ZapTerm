-- Enforce unique names for reusable "vault" credential entries so they can be
-- referenced unambiguously by name from a macro (`{{vault:<Name>.<Field>}}`).
-- A partial index keeps the constraint scoped to vault rows; per-session
-- (non-reusable) credentials may still share names freely.
-- 0.13.0 (migration 020) allowed duplicate vault names; rename all but the
-- oldest of each group ("name (id)") so the index below can be created
-- instead of failing and blocking the upgrade.
UPDATE credentials
   SET name = name || ' (' || id || ')'
 WHERE reusable = 1
   AND id NOT IN (SELECT MIN(id) FROM credentials WHERE reusable = 1 GROUP BY name);

CREATE UNIQUE INDEX IF NOT EXISTS idx_credentials_vault_name
    ON credentials(name) WHERE reusable = 1;

INSERT OR IGNORE INTO schema_version(version) VALUES (21);
