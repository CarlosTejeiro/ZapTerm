// The user's own command list, the same on every platform. It's stored like
// any user list file (`<data dir>/cmdlists/custom.json`, platform "custom"),
// so it can also be edited by hand; the bar shows it as one list.
import { LISTS_FORMAT, LISTS_VERSION } from './index'
import type { CommandItem, CommandList, VendorLists } from './types'

/** File/platform key of the custom list. */
export const CUSTOM_PLATFORM = 'custom'
/** List id the bar uses for the custom panel. */
export const CUSTOM_ID = '__custom__'

/** The custom list as one list: every list and section of the file, in order
 *  (an empty list when there's no file yet). */
export function customList(userLists: Record<string, VendorLists>): CommandList {
  const file = userLists[CUSTOM_PLATFORM]
  const sections = file ? file.lists.flatMap((l) => l.sections) : []
  return {
    id: CUSTOM_ID,
    name: 'Custom',
    icon: 'system',
    color: '#8b7cf6',
    summary: 'Your commands, on every platform',
    sections: sections.length ? sections : [{ title: '', items: [] }],
  }
}

/** Where an item sits: section and position in it. */
export interface ItemAt {
  si: number
  ii: number
}

/** The list with `item` replacing the one at `at`, or added at the end of
 *  the last section when `at` is null. */
export function withItem(list: CommandList, at: ItemAt | null, item: CommandItem): CommandList {
  const sections = list.sections.map((s) => ({ ...s, items: [...s.items] }))
  if (sections.length === 0) sections.push({ title: '', items: [] })
  if (at && sections[at.si]?.items[at.ii]) {
    sections[at.si]!.items[at.ii] = item
  } else {
    sections[sections.length - 1]!.items.push(item)
  }
  return { ...list, sections }
}

/** The list without the item at `at`. */
export function withoutItem(list: CommandList, at: ItemAt): CommandList {
  const sections = list.sections.map((s, si) =>
    si === at.si ? { ...s, items: s.items.filter((_, ii) => ii !== at.ii) } : s,
  )
  return { ...list, sections }
}

/** A command for the custom list: one line, trimmed. */
export function customCommand(cmd: string, desc = ''): CommandItem | null {
  const line = cmd.replace(/[\r\n]+/g, ' ').trim()
  if (!line) return null
  return { kind: 'command', cmd: line, desc: desc.trim() }
}

/** Whether the list already holds this command. */
export function hasCommand(list: CommandList, cmd: string): boolean {
  return list.sections.some((s) => s.items.some((i) => i.kind === 'command' && i.cmd === cmd))
}

/** The file text for the custom list. */
export function customFileText(list: CommandList): string {
  const file = {
    format: LISTS_FORMAT,
    version: LISTS_VERSION,
    platform: CUSTOM_PLATFORM,
    label: 'Custom',
    stopDebug: [],
    lists: [
      {
        id: 'custom',
        name: 'Custom',
        icon: 'system',
        color: list.color,
        summary: list.summary,
        sections: list.sections,
      },
    ],
  }
  return JSON.stringify(file, null, 2) + '\n'
}
