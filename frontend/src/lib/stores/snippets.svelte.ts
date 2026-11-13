// Shared snippet state.
//
// Three reactive surfaces:
//   • `snippets`         — the FULL list, used by the manage dialog so the
//                           user can edit / delete regardless of which
//                           session is focused.
//   • `visibleSnippets`  — what the bottom button bar currently shows: a
//                           subset of `snippets` filtered by the focused
//                           session's platform (globals + matches).
//                           Capped at 9 so the [1..9] number pills always
//                           fit on screen.
//   • `recents`          — read-only "recents" zone of the bar — top
//                           frequently-typed commands the user can re-fire
//                           with one click. Pulled from `command_history`.
//
// `currentPlatform` is the bar's view of the focused session's platform;
// setting it triggers `loadVisibleSnippets()` + `loadRecents()`.

import {
  listSnippets,
  listSnippetsForPlatform,
  listRecentCommandSnippets,
} from '$lib/bridge/commands'
import type { Snippet, RecentCommand } from '$lib/bridge/types'
import { eventToCombo, findSnippetForCombo } from '$lib/stores/shortcuts'

/** Maximum number of user snippets shown in the bottom bar at once. The
 *  number-pill keyboard shortcut Ctrl+Shift+1..9 only addresses these. */
export const SNIPPET_BAR_LIMIT = 9
/** Maximum commands listed in the bar's "Recent" menu (newest first). */
export const RECENTS_BAR_LIMIT = 15

export const snippets = $state<Snippet[]>([])
export const visibleSnippets = $state<Snippet[]>([])
export const recents = $state<RecentCommand[]>([])
/// `null` = no focused session yet (initial). `""` / `"generic"` = a session
/// with no recognised platform; visibleSnippets contains globals only.
export const barContext = $state<{ platform: string | null; savedSessionId: number | null }>({
  platform: null,
  savedSessionId: null,
})

/** The snippet or macro whose own shortcut matches this key press in the
 *  focused session (globals, or scoped to its platform), or null. Uses the
 *  FULL list — macros live in the sidebar, not the 9-slot bar. */
export function matchSnippetShortcut(e: KeyboardEvent): Snippet | null {
  return findSnippetForCombo(eventToCombo(e), snippets, barContext.platform)
}

/** Refresh the full list (for the manage dialog). */
export async function loadSnippets(): Promise<void> {
  try {
    const fresh = await listSnippets()
    snippets.length = 0
    snippets.push(...fresh)
  } catch {
    /* non-fatal */
  }
}

/** Refresh what the bar shows. Call when the focused session changes. */
export async function loadVisibleSnippets(): Promise<void> {
  const platform = barContext.platform ?? ''
  try {
    // Always platform-scoped. With a real key it returns globals + that
    // platform's snippets (and seeds its curated defaults on first use);
    // with `''` (no recognised platform) it returns GLOBALS ONLY. We must
    // NOT fall back to `listSnippets()` here — that returns every vendor's
    // snippets at once, which is why Cisco commands leaked into local and
    // Linux sessions.
    // Macros (snippets with steps) live in the sidebar Macros section, not the
    // bottom bar — filter them out before the visible-count slice.
    const fresh = (await listSnippetsForPlatform(platform)).filter((s) => !s.steps_json)
    visibleSnippets.length = 0
    visibleSnippets.push(...fresh.slice(0, SNIPPET_BAR_LIMIT))
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

/// Update the focused-session context and refresh both panels. Safe to call
/// with the same values — it short-circuits when nothing changed.
export async function setBarContext(
  platform: string | null,
  savedSessionId: number | null,
): Promise<void> {
  if (barContext.platform === platform && barContext.savedSessionId === savedSessionId) return
  barContext.platform = platform
  barContext.savedSessionId = savedSessionId
  await Promise.all([loadVisibleSnippets(), loadRecents()])
}
