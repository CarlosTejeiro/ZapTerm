import { describe, expect, it } from 'vitest'
import {
  dayCounts,
  dayLabel,
  durationText,
  groupActivity,
  limitDays,
  matches,
  NO_FILTER,
  outcomeOf,
  rowsAsText,
  statsOf,
  visibleDays,
  type ActivityEntry,
} from './activityView'

let clock = 0
/** An event `minutes` after the previous one, on 24 Nov unless `day` says otherwise. */
function ev(action: string, over: Partial<ActivityEntry> = {}, minutes = 1, day = '2026-11-24') {
  clock += minutes
  const hh = String(8 + Math.floor(clock / 60)).padStart(2, '0')
  const mm = String(clock % 60).padStart(2, '0')
  return {
    time: `${day}T${hh}:${mm}:00+01:00`,
    source: 'mcp',
    client: 'Claude Desktop',
    device: 'FWF-40F',
    address: '192.168.20.254:22',
    action,
    result: action === 'run' ? 'sent' : 'ok',
    seconds: action === 'run' ? 0.4 : null,
    detail: action === 'run' ? 'get system status' : '',
    ...over,
  } as ActivityEntry
}
const reset = () => (clock = 0)

describe('outcomeOf / statsOf', () => {
  it('sorts results into sent, blocked, declined and failed', () => {
    reset()
    const rows = [
      ev('run'),
      ev('run', { result: 'approved' }),
      ev('open', { result: 'ok' }),
      ev('run', { result: 'refused' }),
      ev('run', { result: 'declined' }),
      ev('run', { result: 'timeout' }),
      ev('run', { result: 'error' }),
    ]
    expect(rows.map(outcomeOf)).toEqual([
      'sent',
      'sent',
      'sent',
      'blocked',
      'declined',
      'failed',
      'failed',
    ])
    expect(statsOf(rows)).toEqual({ all: 7, sent: 3, blocked: 1, declined: 1, failed: 2 })
  })
})

describe('groupActivity', () => {
  it('makes one visit from connect to disconnect, rows only', () => {
    reset()
    const groups = groupActivity([
      ev('connect', { device: '', address: '' }),
      ev('run'),
      ev('run', { result: 'refused', detail: 'config system global' }),
      ev('disconnect', { device: '', address: '' }),
    ])
    expect(groups).toHaveLength(1)
    expect(groups[0]!.rows.map((r) => r.detail)).toEqual([
      'get system status',
      'config system global',
    ])
    expect(groups[0]!.open).toBe(false)
    expect(groups[0]!.start.slice(11, 16)).toBe('08:01')
    expect(groups[0]!.end.slice(11, 16)).toBe('08:04')
  })

  it('starts a new visit on reconnect, another client, a long pause or a new day', () => {
    reset()
    const groups = groupActivity([
      ev('connect'),
      ev('run'),
      ev('disconnect'),
      ev('connect'), // reconnect
      ev('run'),
      ev('run', { source: 'notebook', client: 'Jupyter' }), // someone else, no connect seen
      ev('run', { source: 'notebook', client: 'Jupyter' }, 45), // long pause
      ev('run', { source: 'notebook', client: 'Jupyter' }, 1, '2026-11-25'), // next day
    ])
    expect(groups.map((g) => [g.source, g.rows.length, g.open])).toEqual([
      ['mcp', 1, false],
      ['mcp', 1, true],
      ['notebook', 1, true],
      ['notebook', 1, true],
      ['notebook', 1, true],
    ])
  })

  it('shows a refused client as a closed visit that sent nothing, with why', () => {
    reset()
    const n = { source: 'notebook', client: 'Python', device: '', address: '' } as const
    const [g] = groupActivity([
      ev('connect', {
        ...n,
        result: 'refused',
        detail: 'refused: this connection is for the other kind of client',
      }),
      ev('disconnect', n),
    ])
    expect(g!.rows).toEqual([])
    expect(g!.note).toContain('refused')
    expect(g!.open).toBe(false)
  })

  it('keeps how a script began and ended as the note', () => {
    reset()
    const s = { source: 'script', client: 'sase_reset.py' } as const
    const [g] = groupActivity([
      ev('start', { ...s, detail: 'read-only' }),
      ev('run', s),
      ev('finish', { ...s, detail: 'exit code 0' }),
    ])
    expect(g!.note).toBe('read-only · exit code 0')
    expect(g!.open).toBe(false)
  })
})

describe('matches / visibleDays', () => {
  const sample = () => {
    reset()
    return [
      ev('connect'),
      ev('run'),
      ev('run', { result: 'refused', detail: 'config system global' }),
      ev('disconnect'),
      ev('connect', { source: 'notebook', client: 'Jupyter' }),
      ev('disconnect', { source: 'notebook', client: 'Jupyter' }),
      ev('run', { device: 'core-sw', detail: 'show vlan brief' }, 1, '2026-11-25'),
    ]
  }

  it('filters by result, device and text, and can leave one filter out', () => {
    const [, sent, refused] = sample()
    expect(matches(sent!, { ...NO_FILTER, outcome: 'blocked' })).toBe(false)
    expect(matches(sent!, { ...NO_FILTER, outcome: 'blocked' }, 'outcome')).toBe(true)
    expect(matches(refused!, { ...NO_FILTER, search: 'GLOBAL' })).toBe(true)
    expect(matches(refused!, { ...NO_FILTER, search: '192.168.20' })).toBe(true)
    expect(matches(refused!, { ...NO_FILTER, device: 'core-sw' })).toBe(false)
    expect(matches(refused!, { ...NO_FILTER, day: '2026-11-25' })).toBe(false)
    expect(matches(refused!, { ...NO_FILTER, day: '2026-11-25' }, 'day')).toBe(true)
  })

  it('shows newest day and visit first, rows in the order they happened', () => {
    const days = visibleDays(groupActivity(sample()), NO_FILTER)
    expect(days.map((d) => d.day)).toEqual(['2026-11-25', '2026-11-24'])
    // On the 24th: the notebook (later, sent nothing) then the assistant.
    expect(days[1]!.groups.map((g) => [g.source, g.rows.length])).toEqual([
      ['notebook', 0],
      ['mcp', 2],
    ])
    expect(days[1]!.groups[1]!.rows.map((r) => r.result)).toEqual(['sent', 'refused'])
  })

  it('drops visits with no matching row once a filter narrows the rows', () => {
    const days = visibleDays(groupActivity(sample()), { ...NO_FILTER, outcome: 'blocked' })
    expect(days).toHaveLength(1)
    expect(days[0]!.groups).toHaveLength(1)
    expect(days[0]!.groups[0]!.rows.map((r) => r.detail)).toEqual(['config system global'])
  })

  it('copies the shown rows oldest first, with who, where and what', () => {
    const text = rowsAsText(visibleDays(groupActivity(sample()), NO_FILTER))
    const lines = text.trimEnd().split('\n')
    expect(lines).toHaveLength(3)
    expect(lines[0]).toContain('AI assistant (Claude Desktop)\tFWF-40F [192.168.20.254:22]\tSent')
    expect(lines[1]).toContain('Blocked · read-only\tconfig system global')
    expect(lines[2]).toContain('core-sw')
  })
})

describe('limitDays', () => {
  it('keeps whole visits until the row budget is used', () => {
    reset()
    const entries = []
    for (let v = 0; v < 4; v++) {
      entries.push(ev('connect'))
      for (let i = 0; i < 3; i++) entries.push(ev('run'))
      entries.push(ev('disconnect'))
    }
    const days = visibleDays(groupActivity(entries), NO_FILTER)
    const { days: kept, hidden } = limitDays(days, 4)
    expect(kept[0]!.groups).toHaveLength(2) // 3 rows, then 3 more reach the budget
    expect(hidden).toBe(6)
    expect(limitDays(days, 100).hidden).toBe(0)
  })
})

describe('dayCounts / labels', () => {
  it('counts rows per day for the days up to today, gaps as zero', () => {
    reset()
    const rows = [
      ev('run'),
      ev('run'),
      ev('run', {}, 1, '2026-11-22'),
      ev('run', {}, 1, '2026-10-01'),
    ]
    const counts = dayCounts(rows, new Date(2026, 10, 24), 4)
    expect(counts).toEqual([
      { day: '2026-11-21', count: 0 },
      { day: '2026-11-22', count: 1 },
      { day: '2026-11-23', count: 0 },
      { day: '2026-11-24', count: 2 },
    ])
  })

  it('names today and yesterday, dates the rest', () => {
    const today = new Date(2026, 10, 24)
    expect(dayLabel('2026-11-24', today)).toBe('Today')
    expect(dayLabel('2026-11-23', today)).toBe('Yesterday')
    expect(dayLabel('2026-11-20', today)).toMatch(/20 Nov 2026/)
  })

  it('writes durations a person reads', () => {
    expect(durationText(0.42)).toBe('0.4 s')
    expect(durationText(12.4)).toBe('12 s')
    expect(durationText(65)).toBe('1 min 05 s')
    expect(durationText(null)).toBe('')
  })
})
