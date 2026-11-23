import { describe, expect, it } from 'vitest'
import {
  collapseSame,
  compareOutputs,
  displayNames,
  endsAtPager,
  joinNames,
  normalizeOutput,
  paneTarget,
  sameDeviceSets,
  type CompareHost,
} from './compare'
import { liveDelta } from '$lib/pylon/liveInput'

function host(position: number, buffer: string, opts: Partial<CompareHost> = {}): CompareHost {
  return {
    sessionId: `s${position}`,
    label: `fw${position}`,
    position,
    target: `10.0.0.${position}:22`,
    state: 'done',
    buffer,
    ...opts,
  }
}

const CMD = 'get system status'

describe('normalizeOutput', () => {
  it('drops the echoed command, the prompt and trailing spaces', () => {
    const raw = 'FW # get system status\r\nVersion: v7.4.5  \r\nSerial: X1\r\nFW # '
    expect(normalizeOutput(raw, CMD)).toBe('Version: v7.4.5\nSerial: X1')
  })

  it('drops pager prompts the device erased when paging on', () => {
    const want = 'line 1\nline 2\nline 3'
    // Cisco IOS: " --More-- ", then backspaces, spaces, backspaces.
    const bs = '\b'.repeat(10)
    const ios = `sw# show run\r\nline 1\r\n --More-- ${bs}          ${bs}line 2\r\nline 3\r\nsw# `
    expect(normalizeOutput(ios, 'show run')).toBe(want)
    // Carriage return and spaces over the prompt (Junos style).
    const junos = `show run\r\nline 1\r\n---(more 50%)---\r                \rline 2\r\nline 3\r\nr1> `
    expect(normalizeOutput(junos, 'show run')).toBe(want)
  })

  it('drops pager prompts left at the start of a line', () => {
    const raw = 'show run\nline 1\n--More-- line 2\n<--- More --->\nline 3\nfw# '
    expect(normalizeOutput(raw, 'show run')).toBe('line 1\nline 2\nline 3')
  })

  it('keeps a line ended by a carriage return', () => {
    expect(normalizeOutput('show ver\r\r\nVersion 1\r\r\nfw# ', 'show ver')).toBe('Version 1')
  })

  it('drops the pager prompt a capture stopped at, keeping the last output line', () => {
    expect(normalizeOutput('show run\nline 1\nline 2\n --More-- ', 'show run')).toBe(
      'line 1\nline 2',
    )
  })
})

describe('endsAtPager', () => {
  it('spots the usual pager prompts on the last line only', () => {
    expect(endsAtPager('line 1\nline 2\n--More-- ')).toBe(true)
    expect(endsAtPager('line 1\n --More-- ')).toBe(true)
    expect(endsAtPager('a\n---(more)---')).toBe(true)
    expect(endsAtPager('a\n---(more 45%)---')).toBe(true)
    expect(endsAtPager('a\n<--- More --->')).toBe(true)
    expect(endsAtPager('--More--\nFW # ')).toBe(false)
    expect(endsAtPager('Learn more about it\nFW # ')).toBe(false)
    expect(endsAtPager('')).toBe(false)
  })
  it('does not see a pager prompt the device has already erased', () => {
    expect(endsAtPager('a\n --More-- ' + '\b'.repeat(10) + ' '.repeat(10))).toBe(false)
    expect(endsAtPager('a\n---(more)---\r              \r')).toBe(false)
  })
})

describe('compareOutputs', () => {
  it('groups identical outputs, most common first, lettered A, B…', () => {
    const out = (body: string) => `${CMD}\n${body}\nFW # `
    const hosts = [host(1, out('x')), host(2, out('y')), host(3, out('y'))]
    const { variants, noReply } = compareOutputs(hosts, CMD)
    expect(variants.map((v) => [v.letter, v.text, v.hosts.map((h) => h.position)])).toEqual([
      ['A', 'y', [2, 3]],
      ['B', 'x', [1]],
    ])
    expect(noReply).toEqual([])
  })

  it('breaks ties by the left-most pane', () => {
    const hosts = [host(1, `${CMD}\nb\n#`), host(2, `${CMD}\na\n#`)]
    expect(compareOutputs(hosts, CMD).variants.map((v) => v.text)).toEqual(['b', 'a'])
  })

  it('keeps a silent timeout apart, but compares one that sent output', () => {
    const hosts = [
      host(1, `${CMD}\nok\n#`),
      host(2, '', { state: 'timeout' }),
      host(3, `${CMD}\nok\npartial`, { state: 'timeout' }),
    ]
    const { variants, noReply } = compareOutputs(hosts, CMD)
    expect(noReply.map((h) => h.position)).toEqual([2])
    expect(variants.flatMap((v) => v.hosts.map((h) => h.position)).sort()).toEqual([1, 3])
  })
})

describe('displayNames', () => {
  it('adds the pane number only where labels repeat', () => {
    const hosts = [
      host(1, '', { label: 'FWF-40F' }),
      host(2, '', { label: 'FWF-40F' }),
      host(3, '', { label: 'core-sw' }),
    ]
    const names = displayNames(hosts)
    expect([...names.values()]).toEqual(['FWF-40F (pane 1)', 'FWF-40F (pane 2)', 'core-sw'])
  })
})

describe('sameDeviceSets', () => {
  it('finds panes on the same host:port, ignoring local shells', () => {
    const hosts = [
      host(1, '', { target: 'fw.lab:22' }),
      host(2, '', { target: 'FW.lab:22' }),
      host(3, '', { target: 'fw.lab:2222' }),
      host(4, '', { target: null }),
      host(5, '', { target: null }),
    ]
    const sets = sameDeviceSets(hosts)
    expect(sets.map((s) => s.hosts.map((h) => h.position))).toEqual([[1, 2]])
  })
})

describe('joinNames', () => {
  it('reads naturally', () => {
    expect(joinNames(['a'])).toBe('a')
    expect(joinNames(['a', 'b'])).toBe('a and b')
    expect(joinNames(['a', 'b', 'c'])).toBe('a, b and c')
  })
})

describe('liveDelta', () => {
  it('sends only what was added at the end', () => {
    expect(liveDelta('show ver', 'show version')).toBe('sion')
  })
  it('sends Backspace for what was removed at the end', () => {
    expect(liveDelta('show version', 'show ver')).toBe('\x7f\x7f\x7f\x7f')
  })
  it('erases back to the first change and retypes the rest', () => {
    expect(liveDelta('show run', 'shaw run')).toBe('\x7f'.repeat(6) + 'aw run')
  })
  it('counts characters, not UTF-16 units', () => {
    expect(liveDelta('ab😀', 'ab')).toBe('\x7f')
  })
})

describe('collapseSame', () => {
  it('keeps changes with context and folds long identical runs', () => {
    const same = (t: string) => ({ kind: 'same' as const, text: t })
    const lines = [
      ...Array.from({ length: 10 }, (_, i) => same(`l${i}`)),
      { kind: 'a' as const, text: 'old' },
      { kind: 'b' as const, text: 'new' },
      ...Array.from({ length: 10 }, (_, i) => same(`m${i}`)),
    ]
    const rows = collapseSame(lines, 2)
    expect(rows).toEqual([
      { kind: 'gap', count: 8 },
      same('l8'),
      same('l9'),
      { kind: 'a', text: 'old' },
      { kind: 'b', text: 'new' },
      same('m0'),
      same('m1'),
      { kind: 'gap', count: 8 },
    ])
  })
  it('folds everything when nothing changed', () => {
    expect(collapseSame([{ kind: 'same', text: 'x' }])).toEqual([{ kind: 'gap', count: 1 }])
  })
})

describe('paneTarget', () => {
  it('uses the live params, else the saved session with its default port', () => {
    expect(paneTarget({ ssh: { host: 'fw', port: 2222 } })).toBe('fw:2222')
    expect(paneTarget({ telnet: { host: 'sw', port: 23 } })).toBe('sw:23')
    expect(paneTarget({ savedSession: { protocol: 'ssh', host: 'fw', port: null } })).toBe('fw:22')
    expect(paneTarget({ savedSession: { protocol: 'telnet', host: 'sw', port: 2503 } })).toBe(
      'sw:2503',
    )
    expect(paneTarget({ savedSession: { protocol: 'serial', host: 'COM3', port: null } })).toBe(
      null,
    )
    expect(paneTarget({})).toBe(null)
  })
})
