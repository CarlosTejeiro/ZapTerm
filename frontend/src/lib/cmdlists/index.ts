import type {
  RecipeField,
  CommandItem,
  CommandList,
  ListIcon,
  ListItem,
  ListSection,
  RecipeItem,
  VendorLists,
} from './types'
import fortigate from './builtin/fortigate.json'
import fortimanager from './builtin/fortimanager.json'
import fortianalyzer from './builtin/fortianalyzer.json'
import ciscoIos from './builtin/cisco_ios.json'
import ciscoIosXr from './builtin/cisco_iosxr.json'
import linux from './builtin/linux.json'
import macos from './builtin/macos.json'
import aristaEos from './builtin/arista_eos.json'
import juniper from './builtin/juniper.json'
import windows from './builtin/windows.json'

export type * from './types'

/** Marks a ZapTerm command-lists file. */
export const LISTS_FORMAT = 'zapterm-command-lists'
/** The file format version this build reads and writes. */
export const LISTS_VERSION = 1

const ICONS: readonly ListIcon[] = [
  'system',
  'interfaces',
  'routing',
  'sdwan',
  'bgp',
  'ospf',
  'vpn',
  'ha',
  'security',
  'users',
  'fortilink',
  'logs',
  'debug',
]

type Parsed =
  | { ok: true; platform: string; value: VendorLists; notes?: string }
  | { ok: false; error: string }

class Invalid extends Error {}

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v)

function str(o: Record<string, unknown>, key: string, at: string, optional = false): string {
  const v = o[key]
  if (v === undefined && optional) return ''
  if (typeof v !== 'string') throw new Invalid(`${at}.${key} must be text`)
  return v
}

function arr(o: Record<string, unknown>, key: string, at: string): unknown[] {
  const v = o[key]
  if (!Array.isArray(v)) throw new Invalid(`${at}.${key} must be a list`)
  return v
}

function strings(o: Record<string, unknown>, key: string, at: string): string[] {
  return arr(o, key, at).map((v, i) => {
    if (typeof v !== 'string') throw new Invalid(`${at}.${key}[${i}] must be text`)
    return v
  })
}

function item(v: unknown, at: string): ListItem {
  if (!isObj(v)) throw new Invalid(`${at} must be an object`)
  if (v.kind === 'command') {
    const c: CommandItem = {
      kind: 'command',
      cmd: str(v, 'cmd', at),
      desc: str(v, 'desc', at, true),
    }
    if (!c.cmd.trim()) throw new Invalid(`${at}.cmd is empty`)
    if (/[\r\n]/.test(c.cmd)) throw new Invalid(`${at}.cmd must be one line`)
    if (v.action !== undefined) {
      if (typeof v.action !== 'boolean') throw new Invalid(`${at}.action must be true or false`)
      if (v.action) c.action = true
    }
    if (v.keys !== undefined) c.keys = str(v, 'keys', at)
    const caution = str(v, 'caution', at, true).trim()
    if (caution) c.caution = caution
    return c
  }
  if (v.kind === 'recipe') {
    const fields = arr(v, 'fields', at).map((f, i) => {
      const fat = `${at}.fields[${i}]`
      if (!isObj(f)) throw new Invalid(`${fat} must be an object`)
      const field = {
        key: str(f, 'key', fat),
        label: str(f, 'label', fat),
      } as RecipeItem['fields'][number]
      if (f.default !== undefined) field.default = str(f, 'default', fat)
      if (f.placeholder !== undefined) field.placeholder = str(f, 'placeholder', fat)
      for (const bound of ['min', 'max'] as const) {
        const b = f[bound]
        if (b === undefined) continue
        if (typeof b !== 'number' || !Number.isInteger(b)) {
          throw new Invalid(`${fat}.${bound} must be a whole number`)
        }
        field[bound] = b
      }
      if (field.min !== undefined && field.max !== undefined && field.min > field.max) {
        throw new Invalid(`${fat}: min is greater than max`)
      }
      if (field.default !== undefined && fieldProblem(field, field.default)) {
        throw new Invalid(`${fat}.default is outside ${rangeText(field)}`)
      }
      return field
    })
    const r: RecipeItem = {
      kind: 'recipe',
      name: str(v, 'name', at),
      desc: str(v, 'desc', at, true),
      fields,
      lines: strings(v, 'lines', at),
    }
    const caution = str(v, 'caution', at, true).trim()
    if (caution) r.caution = caution
    const keys = new Set(fields.map((f) => f.key))
    if (keys.size !== fields.length) throw new Invalid(`${at}: two fields have the same key`)
    for (const line of r.lines) {
      for (const m of line.matchAll(/\{\{(\w+)\}\}/g)) {
        if (!keys.has(m[1]!)) throw new Invalid(`${at}: {{${m[1]}}} has no field`)
      }
    }
    return r
  }
  throw new Invalid(`${at}.kind must be "command" or "recipe"`)
}

function list(v: unknown, at: string): CommandList {
  if (!isObj(v)) throw new Invalid(`${at} must be an object`)
  const icon = str(v, 'icon', at, true) as ListIcon
  const l: CommandList = {
    id: str(v, 'id', at),
    name: str(v, 'name', at),
    // An icon this build doesn't know falls back to the generic one.
    icon: ICONS.includes(icon) ? icon : 'system',
    color: str(v, 'color', at, true) || '#6b7280',
    summary: str(v, 'summary', at, true),
    sections: arr(v, 'sections', at).map((s, i): ListSection => {
      const sat = `${at}.sections[${i}]`
      if (!isObj(s)) throw new Invalid(`${sat} must be an object`)
      return {
        title: str(s, 'title', sat, true),
        items: arr(s, 'items', sat).map((it, j) => item(it, `${sat}.items[${j}]`)),
      }
    }),
  }
  if (v.debug === true) l.debug = true
  return l
}

/** Check a command-lists file (text or already-parsed JSON) and turn it into
 *  lists the bar can show. The error names the first problem and where. */
export function parseVendorLists(input: unknown, expectPlatform?: string): Parsed {
  try {
    let v = input
    if (typeof input === 'string') {
      try {
        v = JSON.parse(input)
      } catch (e) {
        throw new Invalid(`not valid JSON (${(e as Error).message})`)
      }
    }
    if (!isObj(v)) throw new Invalid('the file must be a JSON object')
    if (v.format !== LISTS_FORMAT) throw new Invalid(`"format" must be "${LISTS_FORMAT}"`)
    if (typeof v.version !== 'number' || v.version > LISTS_VERSION)
      throw new Invalid(
        `"version" ${String(v.version)} is not supported (this build reads ${LISTS_VERSION})`,
      )
    const platform = str(v, 'platform', 'file')
    if (expectPlatform && platform !== expectPlatform)
      throw new Invalid(`"platform" is "${platform}" but the file is ${expectPlatform}.json`)
    const lists = arr(v, 'lists', 'file').map((l, i) => list(l, `lists[${i}]`))
    const ids = new Set<string>()
    for (const l of lists) {
      if (ids.has(l.id)) throw new Invalid(`two lists have the id "${l.id}"`)
      ids.add(l.id)
    }
    const value: VendorLists = {
      label: str(v, 'label', 'file', true) || platform,
      stopDebug: v.stopDebug === undefined ? [] : strings(v, 'stopDebug', 'file'),
      lists,
    }
    if (v.stopKeys !== undefined) value.stopKeys = str(v, 'stopKeys', 'file')
    const notes = typeof v.notes === 'string' ? v.notes : undefined
    return { ok: true, platform, value, notes }
  } catch (e) {
    if (e instanceof Invalid) return { ok: false, error: e.message }
    throw e
  }
}

/** The built-in lists, by platform (the hint detector's platform key). They
 *  ship as JSON in `builtin/` and are checked like a user file. */
export const BUILTIN: Record<string, VendorLists> = Object.fromEntries(
  [
    fortigate,
    fortimanager,
    fortianalyzer,
    ciscoIos,
    ciscoIosXr,
    linux,
    macos,
    aristaEos,
    juniper,
    windows,
  ].map((raw) => {
    const r = parseVendorLists(raw)
    if (!r.ok) throw new Error(`built-in command lists: ${r.error}`)
    return [r.platform, r.value]
  }),
)

/** The lists for a platform: the user's copy when there is one (it replaces
 *  the built-in list as a whole), else the built-in one. Platforms without
 *  lists get none. */
export function listsForPlatform(
  platform: string | null | undefined,
  userLists: Record<string, VendorLists> = {},
): VendorLists | null {
  if (!platform) return null
  return userLists[platform] ?? BUILTIN[platform] ?? null
}

/** A built-in list as the file a user copy starts from. */
export function builtinFileText(platform: string): string | null {
  const v = BUILTIN[platform]
  if (!v) return null
  return (
    JSON.stringify({ format: LISTS_FORMAT, version: LISTS_VERSION, platform, ...v }, null, 2) + '\n'
  )
}

const PLACEHOLDER = /<([^<>\s]+)>/g

/** "1–1000", "≥ 1" or "≤ 30" for a bounded field. */
export function rangeText(f: RecipeField): string {
  if (f.min !== undefined && f.max !== undefined) return `${f.min}–${f.max}`
  if (f.min !== undefined) return `≥ ${f.min}`
  return `≤ ${f.max}`
}

/** Why `value` can't go in a bounded field, or null when it can (or the
 *  field has no bounds). */
export function fieldProblem(f: RecipeField, value: string): string | null {
  if (f.min === undefined && f.max === undefined) return null
  const v = value.trim()
  if (!/^\d+$/.test(v)) return `a whole number, ${rangeText(f)}`
  const n = Number(v)
  if ((f.min !== undefined && n < f.min) || (f.max !== undefined && n > f.max)) {
    return `between ${rangeText(f)}`
  }
  return null
}

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
