import { describe, expect, it } from 'vitest'
import {
  commandMode,
  commandParts,
  fillCommand,
  fillRecipe,
  itemMatches,
  listsForPlatform,
  placeholderNames,
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
    expect(listsForPlatform('mikrotik')).toBeNull()
    expect(listsForPlatform(null)).toBeNull()
  })

  it.each(['fortigate', 'cisco_ios', 'cisco_iosxr', 'linux', 'arista_eos', 'juniper', 'windows'])(
    '%s data is well formed',
    (platform) => {
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
    },
  )
})
