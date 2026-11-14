// Commands that could take a device down must not be suggested by the
// built-in lists, and the ones that load it (real-time debugs, captures)
// must carry a warning that is confirmed before sending.
import { describe, expect, it } from 'vitest'
import { BUILTIN, parseVendorLists } from './index'
import type { ListItem } from './types'

const all = (): [string, string, ListItem][] =>
  Object.entries(BUILTIN).flatMap(([platform, v]) =>
    v.lists.flatMap((l) =>
      l.sections.flatMap((s) =>
        s.items.map((i) => [platform, l.id, i] as [string, string, ListItem]),
      ),
    ),
  )

const text = (i: ListItem) => (i.kind === 'command' ? i.cmd : i.lines.join('\n'))

/** Irreversible, or an outage that needs someone on site to undo. */
const NEVER = [
  /factoryreset|formatlogdisk|log delete-all|format flash:|write erase|erase startup/i,
  /request system (halt|zeroize)|execute shutdown|request shutdown system|\/system shutdown/i,
  /clear ip (bgp|route) \*/i,
  /\brm -rf\b/,
]

describe('command list safety', () => {
  it('suggests nothing irreversible', () => {
    for (const [p, , i] of all())
      for (const re of NEVER) expect(re.test(text(i)), `${p}: ${text(i)}`).toBe(false)
  })

  it('warns before every recipe of a debug list', () => {
    for (const [p, list, i] of all()) {
      if (i.kind === 'recipe' && BUILTIN[p]!.lists.find((l) => l.id === list)?.debug) {
        expect(i.caution, `${p}/${list}: ${i.name}`).toBeTruthy()
      }
    }
  })

  it('bounds captures and real-time debugs', () => {
    for (const [p, , i] of all()) {
      const t = text(i)
      if (/diagnose sniffer packet/.test(t)) expect(i.caution, `${p}: ${t}`).toBeTruthy()
      if (/^monitor capture \S+ start$/m.test(t)) expect(t, p).toMatch(/limit duration/)
      if (/monitor traffic interface/.test(t)) expect(t, p).toMatch(/count \d+/)
      if (/tcpdump .*-w /.test(t)) expect(t, p).toMatch(/-c \d+/)
      if (
        /diagnose debug application \S+ -1|diagnose ips debug enable|debug crypto|debug ip bgp/.test(
          t,
        )
      ) {
        expect(i.caution, `${p}: ${t}`).toBeTruthy()
      }
    }
  })

  it('reads the caution of a user file', () => {
    const r = parseVendorLists({
      format: 'zapterm-command-lists',
      version: 1,
      platform: 'x',
      lists: [
        {
          id: 'a',
          name: 'A',
          sections: [
            {
              title: '',
              items: [{ kind: 'command', cmd: 'debug all', desc: '', caution: ' Heavy ' }],
            },
          ],
        },
      ],
    })
    expect(r.ok && r.value.lists[0]!.sections[0]!.items[0]!.caution).toBe('Heavy')
  })
})
