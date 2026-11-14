import { describe, expect, it } from 'vitest'
import { commandMode, fillRecipe, typedPrefix } from './index'
import { customFileText, customList } from './custom'
import { isValidCustomFile, snippetToItem, withSnippets } from './legacySnippets'
import type { CommandItem, RecipeItem } from './types'

describe('snippetToItem', () => {
  it('turns a one-line snippet sent with Enter into a command that runs', () => {
    const item = snippetToItem({ name: 'Routes', content: 'show ip route\n' }) as CommandItem
    expect(item).toEqual({ kind: 'command', cmd: 'show ip route', desc: 'Routes' })
    expect(commandMode(item)).toBe('run')
    // A name that just repeats the command adds no description.
    expect(snippetToItem({ name: 'df -h', content: 'df -h\n' })).toEqual({
      kind: 'command',
      cmd: 'df -h',
      desc: '',
    })
  })

  it('keeps a snippet without a trailing newline typed, not run', () => {
    const item = snippetToItem({ name: 'grep', content: 'ps aux | grep ' }) as CommandItem
    expect(commandMode(item)).toBe('type')
    expect(typedPrefix(item)).toBe('ps aux | grep ')
  })

  it('turns variables into a recipe form with their defaults', () => {
    const item = snippetToItem({
      name: 'Ping',
      content: 'execute ping-options source {{src=10.0.0.1}}\nexecute ping {{target host}}\n',
    }) as RecipeItem
    expect(item.kind).toBe('recipe')
    expect(item.fields).toEqual([
      { key: 'src', label: 'src', default: '10.0.0.1' },
      { key: 'target_host', label: 'target host' },
    ])
    expect(fillRecipe(item, { src: '192.0.2.1', target_host: 'core' })).toEqual([
      'execute ping-options source 192.0.2.1',
      'execute ping core',
    ])
  })

  it('sends a line with its own <angle brackets> as is', () => {
    const item = snippetToItem({ name: 'echo', content: 'echo <b>hi</b>\n' }) as RecipeItem
    expect(item.kind).toBe('recipe')
    expect(item.fields).toEqual([])
    expect(fillRecipe(item, {})).toEqual(['echo <b>hi</b>'])
  })

  it('skips empty snippets', () => {
    expect(snippetToItem({ name: 'x', content: ' \n\n' })).toBeNull()
  })
})

describe('withSnippets', () => {
  it('adds the snippets to the custom list once, and the file stays valid', () => {
    const start = customList({})
    const snippets = [
      { name: 'Routes', content: 'show ip route\n' },
      { name: 'Ping', content: 'ping {{host}}\n' },
      { name: 'empty', content: '' },
    ]
    const first = withSnippets(start, snippets)
    expect(first.added).toBe(2)
    const text = customFileText(first.list)
    expect(isValidCustomFile(text)).toBe(true)

    const again = withSnippets(first.list, snippets)
    expect(again.added).toBe(0)
    expect(again.list.sections[0]!.items).toHaveLength(2)
  })
})
