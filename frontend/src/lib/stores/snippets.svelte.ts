// Shared state for the bottom bar's context and the macro library.
//
//   • `snippets`   — every row of the backend's `snippets` table. Today these
//                    are macros (rows with `steps_json`, shown in the sidebar
//                    Macros section); plain snippets were retired in 1.5.0
//                    and the user's own were moved to the ★ Custom list.
//   • `recents`    — the bar's "Recent" menu: commands recently typed in the
//                    focused session, newest first. Pulled from history.
//   • `barContext` — the focused session's platform and saved id; setting it
//                    (setBarContext) refreshes `recents`.

import { listSnippets, listRecentCommandSnippets } from '$lib/bridge/commands'
import type { Snippet, RecentCommand } from '$lib/bridge/types'
import { eventToCombo, findSnippetForCombo } from '$lib/stores/shortcuts'

/** Maximum commands listed in the bar's "Recent" menu (newest first). */
export const RECENTS_BAR_LIMIT = 15

export const snippets = $state<Snippet[]>([])
export const recents = $state<RecentCommand[]>([])
/// `null` = no focused session yet (initial). `""` / `"generic"` = a session
/// with no recognised platform.
export const barContext = $state<{ platform: string | null; savedSessionId: number | null }>({
  platform: null,
  savedSessionId: null,
})

/** The macro whose own shortcut matches this key press in the focused
 *  session (globals, or scoped to its platform), or null. Plain snippets
 *  left over from older versions never fire. */
export function matchSnippetShortcut(e: KeyboardEvent): Snippet | null {
  const macros = snippets.filter((s) => s.steps_json)
  return findSnippetForCombo(eventToCombo(e), macros, barContext.platform)
}

/** Refresh the full list (the macro library reads it). */
export async function loadSnippets(): Promise<void> {
  try {
    const fresh = await listSnippets()
    snippets.length = 0
    snippets.push(...fresh)
  } catch {
    /* non-fatal */
  }
}

/** Refresh the "recents" zone. Returns the dataset the bar should mount. */
export async function loadRecents(): Promise<void> {
  try {
    const fresh = await listRecentCommandSnippets(barContext.savedSessionId, RECENTS_BAR_LIMIT)
    recents.length = 0
    recents.push(...fresh)
  } catch {
    /* non-fatal */
  }
}

/// Update the focused-session context and refresh the Recent menu. Safe to call
/// with the same values — it short-circuits when nothing changed.
export async function setBarContext(
  platform: string | null,
  savedSessionId: number | null,
): Promise<void> {
  if (barContext.platform === platform && barContext.savedSessionId === savedSessionId) return
  barContext.platform = platform
  barContext.savedSessionId = savedSessionId
  await loadRecents()
}
