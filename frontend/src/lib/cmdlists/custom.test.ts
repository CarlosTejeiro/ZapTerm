import { describe, expect, it } from 'vitest'
import { parseVendorLists } from './index'
import {
  customCommand,
  customFileText,
  customList,
  hasCommand,
  withItem,
  withoutItem,
} from './custom'

describe('custom list', () => {
  it('starts empty and grows', () => {
    let list = customList({})
    expect(list.sections).toEqual([{ title: '', items: [] }])
    list = withItem(list, null, customCommand('show version', 'Version')!)
    list = withItem(list, null, customCommand('  ping 8.8.8.8\n')!)
    expect(list.sections[0]!.items.map((i) => i.kind === 'command' && i.cmd)).toEqual([
      'show version',
      'ping 8.8.8.8',
    ])
    expect(hasCommand(list, 'ping 8.8.8.8')).toBe(true)
  })

  it('edits and removes by position', () => {
    let list = customList({})
    list = withItem(list, null, customCommand('a')!)
    list = withItem(list, null, customCommand('b')!)
    list = withItem(list, { si: 0, ii: 0 }, customCommand('a2', 'edited')!)
    list = withoutItem(list, { si: 0, ii: 1 })
    expect(list.sections[0]!.items).toEqual([{ kind: 'command', cmd: 'a2', desc: 'edited' }])
  })

  it('rejects empty commands', () => {
    expect(customCommand('   ')).toBeNull()
  })

  it('round-trips through its file', () => {
    const list = withItem(customList({}), null, customCommand('get system status', 's')!)
    const r = parseVendorLists(customFileText(list), 'custom')
    expect(r.ok).toBe(true)
    if (r.ok) expect(customList({ custom: r.value }).sections).toEqual(list.sections)
  })
})
