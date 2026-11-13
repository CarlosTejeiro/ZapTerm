import type { CommandItem, RecipeItem, VendorLists } from './types'
import { fortigate } from './fortigate'
import { ciscoIos } from './cisco_ios'
import { ciscoIosXr } from './cisco_iosxr'
import { linux } from './linux'
import { aristaEos } from './arista_eos'
import { juniper } from './juniper'
import { windows } from './windows'

export type * from './types'

/** Lists per detected platform (the hint detector's platform key). Vendors
 *  without lists get no list row. */
const BY_PLATFORM: Record<string, VendorLists> = {
  fortigate,
  cisco_ios: ciscoIos,
  cisco_iosxr: ciscoIosXr,
  linux,
  arista_eos: aristaEos,
  juniper,
  windows,
}

export function listsForPlatform(platform: string | null | undefined): VendorLists | null {
  return (platform && BY_PLATFORM[platform]) || null
}

const PLACEHOLDER = /<([^<>\s]+)>/g

/** Split a command into literal text and `<placeholder>` parts, in order. */
export function commandParts(cmd: string): { text: string; placeholder: boolean }[] {
  const parts: { text: string; placeholder: boolean }[] = []
  let last = 0
  for (const m of cmd.matchAll(PLACEHOLDER)) {
    const at = m.index ?? 0
    if (at > last) parts.push({ text: cmd.slice(last, at), placeholder: false })
    parts.push({ text: m[1] ?? '', placeholder: true })
    last = at + m[0].length
  }
  if (last < cmd.length) parts.push({ text: cmd.slice(last), placeholder: false })
  return parts
}

/**
 * How a command is sent:
 *  • `run`    — no placeholder: send it with Enter.
 *  • `type`   — one placeholder, at the end: type everything before it and
 *               leave the cursor there for the user to finish.
 *  • `ask`    — placeholders elsewhere: ask for their values first.
 */
export function commandMode(item: CommandItem): 'run' | 'type' | 'ask' {
  if (item.keys) return 'run'
  const parts = commandParts(item.cmd)
  const holes = parts.filter((p) => p.placeholder).length
  if (holes === 0) return 'run'
  const lastPart = parts[parts.length - 1]
  if (holes === 1 && lastPart?.placeholder) return 'type'
  return 'ask'
}

/** The text typed for a `type` command: everything before its placeholder. */
export function typedPrefix(item: CommandItem): string {
  const i = item.cmd.search(PLACEHOLDER)
  return i < 0 ? item.cmd : item.cmd.slice(0, i)
}

/** The placeholder names of a command, in order, without duplicates. */
export function placeholderNames(cmd: string): string[] {
  const names: string[] = []
  for (const p of commandParts(cmd)) {
    if (p.placeholder && !names.includes(p.text)) names.push(p.text)
  }
  return names
}

/** A command with its placeholders replaced by `values` (trimmed). */
export function fillCommand(cmd: string, values: Record<string, string>): string {
  return cmd.replace(PLACEHOLDER, (_m, name: string) => (values[name] ?? '').trim())
}

/** A recipe's lines with `{{key}}` replaced by the field values (trimmed). */
export function fillRecipe(recipe: RecipeItem, values: Record<string, string>): string[] {
  return recipe.lines.map((line) =>
    line.replace(/\{\{(\w+)\}\}/g, (_m, key: string) => (values[key] ?? '').trim()),
  )
}

/** Case-insensitive match on the command, recipe name or description. */
export function itemMatches(item: CommandItem | RecipeItem, query: string): boolean {
  const q = query.trim().toLowerCase()
  if (!q) return true
  const hay = item.kind === 'command' ? `${item.cmd} ${item.desc}` : `${item.name} ${item.desc}`
  return hay.toLowerCase().includes(q)
}
