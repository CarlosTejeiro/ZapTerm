import { setSetting } from '$lib/bridge/commands'
import { parchment, themeAliases, themes } from './index'
import type { PylonTheme } from './index'

let active = $state<PylonTheme>(parchment)

export function getTheme(): PylonTheme {
  return active
}

/** In-memory switch only — also what `loadSettings` uses to restore the
 *  persisted choice at boot. A renamed theme maps to its replacement
 *  (`themeAliases`); an unknown name is ignored, so the app falls back to the
 *  default instead of breaking. */
export function setTheme(name: string) {
  const t = themes[themeAliases[name] ?? name]
  if (t) active = t
}

/** User-driven switch: apply and persist under `ui.theme` so the choice
 *  survives a restart. Only a valid name is ever written, and persistence is
 *  best-effort — a failed write must never undo or block the visible switch. */
export async function applyTheme(name: string): Promise<void> {
  if (!themes[name]) return
  setTheme(name)
  try {
    await setSetting('ui.theme', name)
  } catch {
    // Non-fatal: the theme is applied for this run; it just won't be remembered.
  }
}
