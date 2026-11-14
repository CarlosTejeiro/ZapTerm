// One-time move of the user's plain snippets to the ★ Custom list (snippets
// were retired in 1.5.0). Runs at startup after the list files are loaded.
import {
  getSetting,
  listUserPlainSnippets,
  setSetting,
  writeUserCmdlist,
} from '$lib/bridge/commands'
import { parseVendorLists } from './index'
import { CUSTOM_PLATFORM, customFileText, customList } from './custom'
import { withSnippets } from './legacySnippets'
import { userLists } from './userLists.svelte'

const DONE_KEY = 'snippets.moved_to_custom'

/** Move the snippets once; returns how many were added to ★ Custom. A
 *  custom.json with an error is left alone and the move is retried at the
 *  next start, so nothing is overwritten or lost. */
export async function moveSnippetsToCustom(): Promise<number> {
  if ((await getSetting(DONE_KEY)) === '1') return 0
  if (userLists.errors[CUSTOM_PLATFORM]) return 0
  const snippets = await listUserPlainSnippets()
  const { list, added } = withSnippets(customList(userLists.lists), snippets)
  if (added > 0) {
    const text = customFileText(list)
    await writeUserCmdlist(CUSTOM_PLATFORM, text)
    const r = parseVendorLists(text, CUSTOM_PLATFORM)
    if (r.ok) userLists.lists = { ...userLists.lists, [CUSTOM_PLATFORM]: r.value }
  }
  await setSetting(DONE_KEY, '1')
  return added
}
