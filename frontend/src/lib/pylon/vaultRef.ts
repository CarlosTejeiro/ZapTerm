// Vault-reference validation for the macro step editor.
//
// A `send` step may be a vault placeholder `{{vault:<inner>}}` (optionally with
// a trailing `\r`). This is an INTENTIONALLY STRICTER lint than the runtime
// parser in `macroRunner.ts` `send()` — not a mirror of it. The runner is
// permissive: any inner it doesn't recognise as `<Name>.<Field>` is treated as
// a bare name that resolves to the password. That permissiveness silently hides
// typos: `{{vault:admin.Passwd}}` resolves the vault name `admin.Passwd`
// (password) instead of `admin`/password and fails at run time. So here we
// accept the forms the runner genuinely supports (legacy id, bare name, or a
// recognised Username/Password field) but flag anything that HAS a dot whose
// suffix looks like a misspelled field as MALFORMED, so the user sees the typo
// BEFORE running.

/** A send that is exactly a vault reference, optionally ending in `\r`. */
const VAULT_REF_RE = /^\{\{vault:([^}]+)\}\}(\\r)?$/

export type VaultRefStatus = 'none' | 'valid' | 'malformed'

/**
 * Classify a `send` value:
 *  - `'none'`      — not a `{{vault:…}}` reference at all.
 *  - `'valid'`     — a form the runner resolves cleanly: an all-digits legacy
 *                    id, a bare non-empty Name (no dot → resolves to password),
 *                    or `<Name>.<Field>` with a non-empty Name and a Field
 *                    (after the last dot, lowercased) of `username`/`password`.
 *  - `'malformed'` — has a dot but the suffix is not `username`/`password`
 *                    (e.g. `admin.Pass word`, `admin.Passwd`) — a likely typo
 *                    the runner would misresolve into the wrong vault name.
 */
export function vaultRefStatus(send: string): VaultRefStatus {
  const m = VAULT_REF_RE.exec(send)
  if (!m) return 'none'
  const inner = m[1]!
  // Legacy id reference → always the password.
  if (/^\d+$/.test(inner)) return 'valid'
  const dot = inner.lastIndexOf('.')
  // Bare name, no dot → the runner's shortcut (name → password). Non-empty is
  // guaranteed by the `[^}]+` capture, but check defensively.
  if (dot < 0) return inner.length > 0 ? 'valid' : 'malformed'
  // Has a dot: only a recognised field suffix (with a non-empty Name) is valid;
  // anything else is a probable field typo.
  const name = inner.slice(0, dot)
  const suffix = inner.slice(dot + 1).toLowerCase()
  if (name.length > 0 && (suffix === 'username' || suffix === 'password')) {
    return 'valid'
  }
  return 'malformed'
}
