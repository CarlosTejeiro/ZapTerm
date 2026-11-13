import { readUserCmdlists } from '$lib/bridge/commands'
import { parseVendorLists, type VendorLists } from './index'

/** The user's copies of the command lists (`<data dir>/cmdlists/`), by
 *  platform, and the files that couldn't be used, with the reason. A valid
 *  copy replaces the built-in list for its platform. */
export const userLists = $state<{
  lists: Record<string, VendorLists>
  errors: Record<string, string>
}>({ lists: {}, errors: {} })

/** (Re)read the user's list files. */
export async function loadUserLists(): Promise<void> {
  let files: { platform: string; json: string }[] = []
  try {
    files = await readUserCmdlists()
  } catch {
    files = []
  }
  const lists: Record<string, VendorLists> = {}
  const errors: Record<string, string> = {}
  for (const f of files) {
    const r = parseVendorLists(f.json, f.platform)
    if (r.ok) lists[f.platform] = r.value
    else errors[f.platform] = r.error
  }
  userLists.lists = lists
  userLists.errors = errors
}
