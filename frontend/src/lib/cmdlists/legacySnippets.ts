// Plain snippets were retired in 1.5.0. The ones the user wrote are moved,
// once, to the ★ Custom list as the closest list item:
//   • one line, sent with Enter        → a command (runs on click);
//   • one line, no trailing newline    → a command ending in `<value>`, so a
//                                        click types it and waits, as before;
//   • several lines or `{{variables}}` → a recipe: a form asks for the
//                                        variables, then the lines are sent.
import { parseVendorLists } from './index'
import type { CommandItem, CommandList, ListItem, RecipeField, RecipeItem } from './types'

/** The snippet variable syntax: `{{name}}`, `{{name=default}}`, `{{name|default}}`. */
const VAR_RE = /\{\{\s*([^}|=]+?)\s*(?:[|=]\s*([^}]*?))?\s*\}\}/g
/** A `<placeholder>` as the command lists read it. */
const PLACEHOLDER = /<[^<>\s]+>/

export interface LegacySnippet {
  name: string
  content: string
}

/** The list item for a snippet, or null when it has no text. */
export function snippetToItem(snippet: LegacySnippet): ListItem | null {
  const content = snippet.content.replace(/\r\n?/g, '\n')
  const runs = content.endsWith('\n')
  const text = content.replace(/\n+$/, '')
  if (!text.trim()) return null
  const name = snippet.name.trim()
  const lines = text.split('\n')
  const hasVars = [...text.matchAll(VAR_RE)].length > 0

  // A single plain line whose own `<…>` won't be taken for a placeholder.
  if (lines.length === 1 && !hasVars && !PLACEHOLDER.test(text)) {
    const cmd = runs ? text.trim() : `${text.trimStart()}<value>`
    const item: CommandItem = { kind: 'command', cmd, desc: name === text.trim() ? '' : name }
    return item
  }

  const fields: RecipeField[] = []
  const keyOf = new Map<string, string>()
  for (const m of text.matchAll(VAR_RE)) {
    const label = (m[1] ?? '').trim()
    if (!label || keyOf.has(label)) continue
    let key = label.replace(/\W+/g, '_').replace(/^_+|_+$/g, '') || 'value'
    while (fields.some((f) => f.key === key)) key += '_'
    keyOf.set(label, key)
    const field: RecipeField = { key, label }
    const def = (m[2] ?? '').trim()
    if (def) field.default = def
    fields.push(field)
  }
  const recipe: RecipeItem = {
    kind: 'recipe',
    name: name || lines[0]!.trim(),
    desc: '',
    fields,
    lines: lines.map((line) =>
      line.replace(VAR_RE, (_m, label: string) => `{{${keyOf.get(label.trim())}}}`),
    ),
  }
  return recipe
}

function sameItem(a: ListItem, b: ListItem): boolean {
  if (a.kind === 'command' && b.kind === 'command') return a.cmd === b.cmd
  if (a.kind === 'recipe' && b.kind === 'recipe') return a.lines.join('\n') === b.lines.join('\n')
  return false
}

/** The custom list with the snippets added at the end of its last section
 *  (skipping any it already holds), and how many were added. */
export function withSnippets(
  list: CommandList,
  snippets: LegacySnippet[],
): { list: CommandList; added: number } {
  const sections = list.sections.map((s) => ({ ...s, items: [...s.items] }))
  if (sections.length === 0) sections.push({ title: '', items: [] })
  const last = sections[sections.length - 1]!
  let added = 0
  for (const s of snippets) {
    const item = snippetToItem(s)
    if (!item || sections.some((sec) => sec.items.some((i) => sameItem(i, item)))) continue
    last.items.push(item)
    added++
  }
  return { list: { ...list, sections }, added }
}

/** Whether `text` is a valid custom list file (what a save would write). */
export function isValidCustomFile(text: string): boolean {
  try {
    const parsed = parseVendorLists(JSON.parse(text), 'custom')
    return parsed.ok
  } catch {
    return false
  }
}
