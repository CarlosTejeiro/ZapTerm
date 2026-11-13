import { describe, it, expect } from 'vitest'
import {
  SHORTCUT_ACTIONS,
  eventToCombo,
  findAction,
  findSnippetForCombo,
  shortcutClash,
  snippetComboProblem,
  type ShortcutAction,
} from './shortcuts'

// Minimal KeyboardEvent stand-in — eventToCombo only reads these fields, and
// the Vitest env is plain node (no DOM `KeyboardEvent` constructor).
function key(
  k: string,
  opts: { code?: string; ctrl?: boolean; shift?: boolean; alt?: boolean; meta?: boolean } = {},
): KeyboardEvent {
  return {
    key: k,
    code: opts.code ?? '',
    ctrlKey: !!opts.ctrl,
    shiftKey: !!opts.shift,
    altKey: !!opts.alt,
    metaKey: !!opts.meta,
  } as KeyboardEvent
}

// Defaults map, as loaded at startup before any user override.
const defaults: Record<string, string> = Object.fromEntries(
  SHORTCUT_ACTIONS.map((a) => [a.id, a.default]),
)
const match = (e: KeyboardEvent): ShortcutAction | null => findAction(eventToCombo(e), defaults)

describe('eventToCombo', () => {
  it('returns null for modifier-only presses', () => {
    expect(eventToCombo(key('Control', { ctrl: true }))).toBeNull()
    expect(eventToCombo(key('Shift', { shift: true }))).toBeNull()
  })

  it('upper-cases single letters and orders modifiers deterministically', () => {
    expect(eventToCombo(key('n', { code: 'KeyN', ctrl: true }))).toBe('Ctrl+N')
    expect(eventToCombo(key('M', { code: 'KeyM', ctrl: true, shift: true }))).toBe('Ctrl+Shift+M')
  })

  it('keeps named keys verbatim', () => {
    expect(eventToCombo(key('Tab', { code: 'Tab', ctrl: true }))).toBe('Ctrl+Tab')
    expect(eventToCombo(key('Tab', { code: 'Tab', ctrl: true, shift: true }))).toBe(
      'Ctrl+Shift+Tab',
    )
  })

  // Regression: symbol keys are canonicalised by physical position so Shift /
  // keyboard layout can't scramble them.
  it('canonicalises the backslash key regardless of the produced character', () => {
    // US, no shift: the key already reports "\".
    expect(eventToCombo(key('\\', { code: 'Backslash', ctrl: true }))).toBe('Ctrl+\\')
    // US, with shift: the event reports "|" but must still resolve to "\".
    expect(eventToCombo(key('|', { code: 'Backslash', ctrl: true, shift: true }))).toBe(
      'Ctrl+Shift+\\',
    )
    // Spanish layout: the physical backslash-position key emits "ç".
    expect(eventToCombo(key('ç', { code: 'Backslash', ctrl: true }))).toBe('Ctrl+\\')
  })

  it('canonicalises comma from its physical code', () => {
    expect(eventToCombo(key(',', { code: 'Comma', ctrl: true }))).toBe('Ctrl+,')
  })
})

describe('default bindings are all reachable', () => {
  // Split defaults use letter keys (Ctrl+Shift+H / Ctrl+Shift+K) so they read
  // and fire the same on every layout — the old `\`-based defaults were
  // unreachable (Shift remaps the char; `\` needs AltGr on some layouts).
  it('split-horizontal fires on Ctrl+Shift+H', () => {
    expect(match(key('H', { code: 'KeyH', ctrl: true, shift: true }))).toBe('split-h')
  })

  it('split-vertical fires on Ctrl+Shift+K', () => {
    expect(match(key('K', { code: 'KeyK', ctrl: true, shift: true }))).toBe('split-v')
  })

  it('matches the remaining letter/named-key defaults', () => {
    expect(match(key('n', { code: 'KeyN', ctrl: true }))).toBe('new-session')
    expect(match(key('t', { code: 'KeyT', ctrl: true }))).toBe('new-tab')
    expect(match(key('w', { code: 'KeyW', ctrl: true }))).toBe('close-tab')
    expect(match(key('Tab', { code: 'Tab', ctrl: true }))).toBe('next-tab')
    expect(match(key('Tab', { code: 'Tab', ctrl: true, shift: true }))).toBe('prev-tab')
    expect(match(key(',', { code: 'Comma', ctrl: true }))).toBe('settings')
    expect(match(key('M', { code: 'KeyM', ctrl: true, shift: true }))).toBe('multi-exec')
    expect(match(key('S', { code: 'KeyS', ctrl: true, shift: true }))).toBe('snippets')
    expect(match(key('N', { code: 'KeyN', ctrl: true, shift: true }))).toBe('quick-connect')
  })

  it('does not match when an extra modifier is held', () => {
    expect(match(key('n', { code: 'KeyN', ctrl: true, alt: true }))).toBeNull()
  })
})

describe('snippetComboProblem', () => {
  it('accepts Ctrl+Shift+letter, Alt+letter and Ctrl+Alt+letter', () => {
    expect(snippetComboProblem(key('P', { code: 'KeyP', ctrl: true, shift: true }))).toBeNull()
    expect(snippetComboProblem(key('p', { code: 'KeyP', alt: true }))).toBeNull()
    expect(snippetComboProblem(key('p', { code: 'KeyP', ctrl: true, alt: true }))).toBeNull()
  })

  it('rejects modifier-only, unmodified and plain Ctrl+key presses', () => {
    expect(snippetComboProblem(key('Control', { ctrl: true }))).not.toBeNull()
    expect(snippetComboProblem(key('p', { code: 'KeyP' }))).not.toBeNull()
    expect(snippetComboProblem(key('P', { code: 'KeyP', shift: true }))).not.toBeNull()
    // Ctrl+C / Ctrl+R belong to the shell.
    expect(snippetComboProblem(key('c', { code: 'KeyC', ctrl: true }))).not.toBeNull()
    expect(snippetComboProblem(key('r', { code: 'KeyR', ctrl: true }))).not.toBeNull()
  })

  it('rejects the Ctrl+Shift+1..9 row by physical key, whatever it produces', () => {
    expect(snippetComboProblem(key('!', { code: 'Digit1', ctrl: true, shift: true }))).toMatch(
      /1…9/,
    )
    expect(snippetComboProblem(key('·', { code: 'Digit3', ctrl: true, shift: true }))).toMatch(
      /1…9/,
    )
    // Ctrl+Shift+0 is outside the reserved row.
    expect(snippetComboProblem(key('=', { code: 'Digit0', ctrl: true, shift: true }))).toBeNull()
  })
})

describe('shortcutClash', () => {
  const snips = [
    { id: 1, name: 'pw fw', platform: 'fortigate', shortcut: 'Ctrl+Alt+P' },
    { id: 2, name: 'status', platform: null, shortcut: null },
  ]

  it('reports app actions and other snippets', () => {
    expect(shortcutClash('Ctrl+Shift+M', defaults, snips, null)).toMatch(/MultiExec/)
    expect(shortcutClash('Ctrl+Alt+P', defaults, snips, 2)).toMatch(/pw fw/)
  })

  it('lets a snippet keep its own combo and frees unused ones', () => {
    expect(shortcutClash('Ctrl+Alt+P', defaults, snips, 1)).toBeNull()
    expect(shortcutClash('Ctrl+Alt+Q', defaults, snips, null)).toBeNull()
  })
})

describe('findSnippetForCombo', () => {
  const snips = [
    { id: 1, name: 'fgt', platform: 'fortigate', shortcut: 'Alt+1' },
    { id: 2, name: 'any', platform: null, shortcut: 'Alt+2' },
    { id: 3, name: 'ios', platform: 'cisco_ios', shortcut: 'Alt+3' },
  ]

  it('fires globals everywhere and scoped snippets only on their platform', () => {
    expect(findSnippetForCombo('Alt+2', snips, null)?.id).toBe(2)
    expect(findSnippetForCombo('Alt+1', snips, 'fortigate')?.id).toBe(1)
    expect(findSnippetForCombo('Alt+1', snips, 'linux')).toBeNull()
    expect(findSnippetForCombo('Alt+3', snips, 'fortigate')).toBeNull()
    expect(findSnippetForCombo(null, snips, 'fortigate')).toBeNull()
  })

  it('prefers the platform-scoped snippet when a global shares the combo', () => {
    const both = [
      { id: 1, name: 'g', platform: null, shortcut: 'Alt+X' },
      { id: 2, name: 'f', platform: 'fortigate', shortcut: 'Alt+X' },
    ]
    expect(findSnippetForCombo('Alt+X', both, 'fortigate')?.id).toBe(2)
    expect(findSnippetForCombo('Alt+X', both, 'linux')?.id).toBe(1)
  })
})
