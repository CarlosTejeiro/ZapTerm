// Commands that could take a device down must not be suggested by the
// built-in lists, and the ones that load it (real-time debugs, captures)
// must carry a warning that is confirmed before sending.
import { describe, expect, it } from 'vitest'
import { BUILTIN, fieldProblem, parseVendorLists } from './index'
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

  it('leaves the range out of bounded field labels (the form shows it)', () => {
    for (const [p, , i] of all()) {
      if (i.kind !== 'recipe') continue
      for (const f of i.fields)
        if (f.min !== undefined || f.max !== undefined)
          expect(f.label, `${p}: ${i.name}.${f.key}`).not.toMatch(/\(\d+\s*[–-]\s*\d+\)/)
    }
  })

  it('bounds every FortiGate real-time debug in time and every count', () => {
    for (const [p, , i] of all()) {
      if (i.kind !== 'recipe') continue
      if (p === 'fortigate' && i.lines.includes('diagnose debug enable')) {
        const at = i.lines.indexOf('diagnose debug duration {{minutes}}')
        expect(at, i.name).toBeGreaterThanOrEqual(0)
        expect(at, i.name).toBeLessThan(i.lines.indexOf('diagnose debug enable'))
        const m = i.fields.find((f) => f.key === 'minutes')!
        expect(m.max! <= 30 && m.min! >= 1, i.name).toBe(true)
      }
      for (const f of i.fields) {
        if (['count', 'seconds', 'minutes'].includes(f.key)) {
          expect(f.min, `${p}: ${i.name}.${f.key}`).toBeGreaterThanOrEqual(1)
          expect(f.max, `${p}: ${i.name}.${f.key}`).toBeDefined()
        }
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
  it('checks bounded fields', () => {
    const f = { key: 'count', label: 'Packets', min: 1, max: 1000 }
    expect(fieldProblem(f, '100')).toBeNull()
    expect(fieldProblem(f, '0')).toMatch(/between 1–1000/)
    expect(fieldProblem(f, '5000')).toMatch(/between/)
    expect(fieldProblem(f, 'all')).toMatch(/whole number/)
    expect(fieldProblem({ key: 'ip', label: 'IP' }, 'anything')).toBeNull()
    const bad = (field: object) =>
      parseVendorLists({
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
                items: [
                  { kind: 'recipe', name: 'r', desc: '', fields: [field], lines: ['x {{n}}'] },
                ],
              },
            ],
          },
        ],
      })
    expect(bad({ key: 'n', label: 'N', min: 5, max: 1 })).toMatchObject({ ok: false })
    expect(bad({ key: 'n', label: 'N', max: 10, default: '50' })).toMatchObject({ ok: false })
    expect(bad({ key: 'n', label: 'N', min: 1.5 })).toMatchObject({ ok: false })
    expect(bad({ key: 'n', label: 'N', min: 1, max: 10, default: '5' })).toMatchObject({ ok: true })
  })
})
