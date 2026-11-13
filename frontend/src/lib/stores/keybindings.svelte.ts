// Editable keyboard shortcuts.
//
// Each action has a stable id, a human label, and a default combo. The user
// can override the combo from `SettingsModal` and it is persisted to the
// `settings` SQLite table under the key `shortcut.<action-id>`.
//
// The pure combo/matching logic lives in `shortcuts.ts` (unit-testable under
// Vitest's node environment); this module holds the reactive `bindings` state
// and the `matchAction(e)` dispatch helper that `App.svelte` /
// `TerminalTab.svelte` call from their keydown handlers.

import { getSetting, setSetting } from '$lib/bridge/commands'
import {
  SHORTCUT_ACTIONS,
  eventToCombo,
  findAction,
  type ShortcutAction,
} from '$lib/stores/shortcuts'

export { SHORTCUT_ACTIONS, eventToCombo }
export type { ShortcutAction }

/// Reactive bindings: action id → combo string (e.g. `"Ctrl+Shift+M"`).
export const bindings = $state<Record<string, string>>(
  Object.fromEntries(SHORTCUT_ACTIONS.map((a) => [a.id, a.default])),
)

/// Pull each action's override from the settings table at startup.
export async function loadBindings(): Promise<void> {
  for (const action of SHORTCUT_ACTIONS) {
    try {
      const stored = await getSetting(`shortcut.${action.id}`)
      if (stored && stored.trim()) {
        bindings[action.id] = stored
      }
    } catch {
      /* settings key absent → keep default */
    }
  }
}

/// Persist a single binding (also updates the reactive store).
export async function saveBinding(actionId: ShortcutAction, combo: string): Promise<void> {
  bindings[actionId] = combo
  await setSetting(`shortcut.${actionId}`, combo)
}

/// Reset an action to its compiled-in default and clear the override in DB.
export async function resetBinding(actionId: ShortcutAction): Promise<void> {
  const def = SHORTCUT_ACTIONS.find((a) => a.id === actionId)?.default ?? ''
  bindings[actionId] = def
  await setSetting(`shortcut.${actionId}`, '')
}

/// Find the action whose combo matches this event, or null.
export function matchAction(e: KeyboardEvent): ShortcutAction | null {
  return findAction(eventToCombo(e), bindings)
}
