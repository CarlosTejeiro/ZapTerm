import { describe, expect, it } from 'vitest'
import {
  BUILTIN,
  builtinFileText,
  commandMode,
  commandParts,
  fillCommand,
  fillRecipe,
  itemMatches,
  listsForPlatform,
  parseVendorLists,
  placeholderNames,
  type VendorLists,
  typedPrefix,
} from './index'
import type { CommandItem, RecipeItem } from './types'

const cmd = (c: string): CommandItem => ({ kind: 'command', cmd: c, desc: '' })

describe('command lists', () => {
  it('splits placeholders from literal text', () => {
    expect(commandParts('get router info bgp neighbors <peer-ip> routes')).toEqual([
      { text: 'get router info bgp neighbors ', placeholder: false },
      { text: 'peer-ip', placeholder: true },
      { text: ' routes', placeholder: false },
    ])
  })

  it('picks run / type / ask', () => {
    expect(commandMode(cmd('get system status'))).toBe('run')
    expect(commandMode(cmd('execute ping <ip>'))).toBe('type')
    expect(commandMode(cmd('get router info bgp neighbors <peer-ip> routes'))).toBe('ask')
  })

  it('sends raw keys as a plain run', () => {
    expect(commandMode({ kind: 'command', cmd: 'Ctrl+C', desc: '', keys: '\x03' })).toBe('run')
  })

  it('types the text before a trailing placeholder', () => {
    expect(typedPrefix(cmd('execute ping <ip>'))).toBe('execute ping ')
  })

  it('fills placeholders and recipe fields', () => {
    expect(placeholderNames('a <x> b <y> c <x>')).toEqual(['x', 'y'])
    expect(
      fillCommand('get router info bgp neighbors <peer-ip> routes', { 'peer-ip': ' 10.1.1.1 ' }),
    ).toBe('get router info bgp neighbors 10.1.1.1 routes')
    const r: RecipeItem = {
      kind: 'recipe',
      name: 'r',
      desc: '',
      fields: [],
      lines: [
        'diagnose debug flow filter addr {{ip}}',
        'diagnose debug flow trace start {{count}}',
      ],
    }
    expect(fillRecipe(r, { ip: '10.0.0.1', count: '50' })).toEqual([
      'diagnose debug flow filter addr 10.0.0.1',
      'diagnose debug flow trace start 50',
    ])
  })

  it('filters by command or description', () => {
    const c: CommandItem = { kind: 'command', cmd: 'get system arp', desc: 'ARP table' }
    expect(itemMatches(c, 'ARP')).toBe(true)
    expect(itemMatches(c, 'bgp')).toBe(false)
    expect(itemMatches(c, '')).toBe(true)
  })

  it('has lists per vendor and nothing for unknown platforms', () => {
    expect(listsForPlatform('fortigate')?.lists.length).toBeGreaterThanOrEqual(12)
    expect(listsForPlatform('cisco_ios')?.lists.length).toBeGreaterThanOrEqual(9)
    expect(listsForPlatform('cisco_iosxr')?.lists.length).toBeGreaterThanOrEqual(8)
    expect(listsForPlatform('linux')?.lists.length).toBeGreaterThanOrEqual(7)
    expect(listsForPlatform('arista_eos')?.lists.length).toBeGreaterThanOrEqual(8)
    expect(listsForPlatform('juniper')?.lists.length).toBeGreaterThanOrEqual(8)
    expect(listsForPlatform('windows')?.lists.length).toBeGreaterThanOrEqual(6)
    expect(listsForPlatform('fortimanager')?.lists.length).toBeGreaterThanOrEqual(6)
    expect(listsForPlatform('fortianalyzer')?.lists.length).toBeGreaterThanOrEqual(5)
    expect(listsForPlatform('mikrotik')).toBeNull()
    expect(listsForPlatform(null)).toBeNull()
  })

  it.each([
    'fortigate',
    'fortimanager',
    'fortianalyzer',
    'cisco_ios',
    'cisco_iosxr',
    'linux',
    'macos',
    'arista_eos',
    'juniper',
    'windows',
  ])('%s data is well formed', (platform) => {
    const vendor = listsForPlatform(platform)!
    const ids = new Set<string>()
    for (const list of vendor.lists) {
      expect(ids.has(list.id)).toBe(false)
      ids.add(list.id)
      for (const section of list.sections) {
        for (const item of section.items) {
          if (item.kind === 'command') {
            expect(item.cmd).not.toMatch(/\n/)
            // Nothing that resets, formats, erases or reboots the device.
            expect(item.cmd).not.toMatch(
              /factoryreset|formatlogdisk|execute reboot|disk format|fsso refresh$|reset-uptime|test authserver|^reload|^erase|^format|write erase/,
            )
          } else {
            const keys = new Set(item.fields.map((f) => f.key))
            for (const line of item.lines) {
              for (const m of line.matchAll(/\{\{(\w+)\}\}/g)) expect(keys.has(m[1]!)).toBe(true)
            }
          }
        }
      }
    }
  })

  it('a user copy replaces the built-in list for its platform only', () => {
    const mine = { label: 'Mine', stopDebug: [], lists: [] }
    expect(listsForPlatform('fortigate', { fortigate: mine })).toBe(mine)
    expect(listsForPlatform('linux', { fortigate: mine })).toBe(BUILTIN.linux)
    expect(listsForPlatform('mikrotik', { mikrotik: mine })).toBe(mine)
  })

  it.each(Object.keys(BUILTIN))('%s: a fresh copy reads back as the built-in list', (platform) => {
    const r = parseVendorLists(builtinFileText(platform)!, platform)
    expect(r.ok && r.value).toEqual(BUILTIN[platform])
  })

  it('explains what is wrong in a file', () => {
    const base = { format: 'zapterm-command-lists', version: 1, platform: 'linux', lists: [] }
    const err = (v: unknown, p?: string) => {
      const r = parseVendorLists(typeof v === 'string' ? v : JSON.stringify(v), p)
      return r.ok ? '' : r.error
    }
    expect(err('{ nope')).toMatch(/not valid JSON/)
    expect(err({ ...base, format: 'x' })).toMatch(/format/)
    expect(err({ ...base, version: 2 })).toMatch(/version/)
    expect(err(base, 'fortigate')).toMatch(/platform/)
    const withItem = (item: unknown) => ({
      ...base,
      lists: [{ id: 'a', name: 'A', sections: [{ title: 'S', items: [item] }] }],
    })
    expect(err(withItem({ kind: 'command', cmd: '' }))).toBe(
      'lists[0].sections[0].items[0].cmd is empty',
    )
    expect(err(withItem({ kind: 'command', cmd: 'a\nb' }))).toMatch(/one line/)
    expect(err(withItem({ kind: 'thing' }))).toMatch(/kind/)
    expect(err(withItem({ kind: 'recipe', name: 'r', fields: [], lines: ['x {{ip}}'] }))).toMatch(
      /\{\{ip\}\} has no field/,
    )
    expect(
      err(
        withItem({
          kind: 'recipe',
          name: 'r',
          fields: [
            { key: 'a', label: 'A' },
            { key: 'a', label: 'B' },
          ],
          lines: [],
        }),
      ),
    ).toMatch(/same key/)
    const twice = {
      ...base,
      lists: [
        { id: 'a', name: 'A', sections: [] },
        { id: 'a', name: 'B', sections: [] },
      ],
    }
    expect(err(twice)).toMatch(/two lists/)
  })

  it('accepts a minimal file and falls back on unknown icons', () => {
    const r = parseVendorLists({
      format: 'zapterm-command-lists',
      version: 1,
      platform: 'mikrotik',
      lists: [
        {
          id: 'sys',
          name: 'System',
          icon: 'rocket',
          sections: [{ title: '', items: [{ kind: 'command', cmd: '/system resource print' }] }],
        },
      ],
    })
    expect(r.ok).toBe(true)
    if (r.ok) {
      expect(r.platform).toBe('mikrotik')
      expect(r.value.label).toBe('mikrotik')
      expect(r.value.lists[0]!.icon).toBe('system')
      expect(r.value.stopDebug).toEqual([])
    }
  })
})

describe('lists per OS release', () => {
  const lists = (osVersion: string, variantOf?: string): VendorLists => ({
    label: `fgt ${osVersion}`,
    stopDebug: [],
    lists: [],
    osVersion,
    ...(variantOf ? { variantOf } : {}),
  })
  const files = {
    fortigate: lists('7.6'),
    fortigate_7_4: lists('7.4', 'fortigate'),
    fortigate_7_2: lists('7.2', 'fortigate'),
    other_7_4: lists('7.4', 'cisco_ios'),
  }
  const pick = (v?: string | null) => listsForPlatform('fortigate', files, v)?.label

  it('uses the platform file when the version is unknown', () => {
    expect(pick()).toBe('fgt 7.6')
    expect(pick(null)).toBe('fgt 7.6')
    expect(pick('garbage')).toBe('fgt 7.6')
  })

  it('uses the lists of the device release', () => {
    expect(pick('7.2.8')).toBe('fgt 7.2')
    expect(pick('7.4.3')).toBe('fgt 7.4')
    expect(pick('7.6.0')).toBe('fgt 7.6')
  })

  it('falls back to the newest older release, or the oldest there is', () => {
    expect(pick('8.0.1')).toBe('fgt 7.6')
    expect(pick('7.5.0')).toBe('fgt 7.4')
    expect(pick('7.10.0')).toBe('fgt 7.6') // 10 > 6, not "1" < "6"
    expect(pick('7.0.14')).toBe('fgt 7.2')
    expect(pick('6.4.15')).toBe('fgt 7.2')
  })

  it('a platform whose lists state no release is unaffected', () => {
    expect(listsForPlatform('linux', {}, '7.4.3')).toBe(BUILTIN.linux)
  })

  it('the built-in FortiGate lists say which release they are for', () => {
    expect(BUILTIN.fortigate?.osVersion).toBe('7.6')
    expect(listsForPlatform('fortigate', {}, '7.2.8')).toBe(BUILTIN.fortigate)
  })

  it('checks osVersion and variantOf in a file', () => {
    const file = (extra: object) => ({
      format: 'zapterm-command-lists',
      version: 1,
      platform: 'fortigate_7_4',
      lists: [],
      ...extra,
    })
    const ok = parseVendorLists(file({ osVersion: '7.4', variantOf: 'fortigate' }))
    expect(ok.ok && ok.value.variantOf).toBe('fortigate')
    expect(parseVendorLists(file({ osVersion: '7.4.3' })).ok).toBe(false)
    expect(parseVendorLists(file({ variantOf: 'fortigate' })).ok).toBe(false)
  })
})

describe('commands bound to releases', () => {
  const cmd = (name: string, extra: object = {}) => ({
    kind: 'command' as const,
    cmd: name,
    desc: 'd',
    ...extra,
  })
  const withItems = (items: object[]): VendorLists => {
    const r = parseVendorLists({
      format: 'zapterm-command-lists',
      version: 1,
      platform: 'fortigate',
      osVersion: '7.6',
      lists: [
        { id: 'a', name: 'A', sections: [{ title: 'S', items }] },
        {
          id: 'b',
          name: 'B',
          sections: [{ title: 'Only old', items: [cmd('gone', { until: '7.6.2' })] }],
        },
      ],
    })
    if (!r.ok) throw new Error(r.error)
    return r.value
  }
  const mine = {
    fortigate: withItems([
      cmd('always'),
      cmd('old way', { until: '7.6.4' }),
      cmd('new way', { since: '7.6.5' }),
      cmd('whole train', { since: '7.6' }),
    ]),
  }
  const shown = (v?: string) =>
    listsForPlatform('fortigate', mine, v)!.lists.flatMap((l) =>
      l.sections.flatMap((s) => s.items.map((i) => (i.kind === 'command' ? i.cmd : i.name))),
    )

  it('shows what exists on the device release', () => {
    expect(shown('7.6.4')).toEqual(['always', 'old way', 'whole train'])
    expect(shown('7.6.5')).toEqual(['always', 'new way', 'whole train'])
    expect(shown('7.6.8')).toEqual(['always', 'new way', 'whole train'])
    expect(shown('7.6.10')).toEqual(['always', 'new way', 'whole train'])
    expect(shown('7.6.1')).toEqual(['always', 'old way', 'whole train', 'gone'])
  })

  it('with the version unknown shows the newest form and says what it needs', () => {
    expect(shown()).toEqual(['always', 'new way', 'whole train'])
    const items = listsForPlatform('fortigate', mine)!.lists[0]!.sections[0]!.items
    expect(items.find((i) => i.kind === 'command' && i.cmd === 'new way')?.desc).toBe('d (7.6.5+)')
    expect(items.find((i) => i.kind === 'command' && i.cmd === 'always')?.desc).toBe('d')
  })

  it('drops a section left empty', () => {
    const titles = listsForPlatform('fortigate', mine, '7.6.8')!.lists.map((l) =>
      l.sections.map((s) => s.title),
    )
    expect(titles).toEqual([['S'], []])
  })

  it('rejects a bound that is not a release, or an inverted range', () => {
    expect(() => withItems([cmd('x', { since: 'soon' })])).toThrow(/since/)
    expect(() => withItems([cmd('x', { since: '7.6.5', until: '7.6.4' })])).toThrow(/later/)
  })
})
