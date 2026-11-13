import { describe, it, expect } from 'vitest'
import {
  cleanText,
  echoEnd,
  extractRunOutput,
  lastLine,
  looksLikePrompt,
  MORE_RE,
  parseAllowList,
  readOnlyAllows,
  stripAnsi,
} from './output'

const FGT = [
  'fw1 # get vpn ipsec tunnel summary\r\n',
  "'SASE_INET' 203.0.113.1:0  selectors(total,up): 1/0  rx(pkt,err): 0/0  tx(pkt,err): 0/0\r\n",
  "'SASE_INET2' 203.0.113.2:0  selectors(total,up): 1/1  rx(pkt,err): 10/0  tx(pkt,err): 9/0\r\n",
  '\r\n',
  'fw1 # ',
].join('')

describe('extractRunOutput', () => {
  it('returns the lines between the echo and the returning prompt', () => {
    expect(extractRunOutput(FGT, 'get vpn ipsec tunnel summary')).toBe(
      [
        "'SASE_INET' 203.0.113.1:0  selectors(total,up): 1/0  rx(pkt,err): 0/0  tx(pkt,err): 0/0",
        "'SASE_INET2' 203.0.113.2:0  selectors(total,up): 1/1  rx(pkt,err): 10/0  tx(pkt,err): 9/0",
      ].join('\n'),
    )
  })

  it('ignores a leftover prompt printed before the command', () => {
    const raw = 'fw1 # \r\nfw1 # get system status\r\nVersion: FortiGate-60F v7.4.4\r\nfw1 # '
    expect(extractRunOutput(raw, 'get system status')).toBe('Version: FortiGate-60F v7.4.4')
  })

  it('is null until the echo arrives, and empty for a command with no output', () => {
    expect(extractRunOutput('fw1 # ', 'end')).toBeNull()
    expect(extractRunOutput('fw1 (interface) # end\r\nfw1 # ', 'end')).toBe('')
  })

  it('drops pager prompts erased with backspaces', () => {
    const raw =
      'sw # show run\r\nline 1\r\n --More-- \x08\x08\x08\x08\x08\x08\x08\x08\x08\x08          \x08\x08\x08\x08\x08\x08\x08\x08\x08\x08line 2\r\nsw # '
    expect(extractRunOutput(raw, 'show run')).toBe('line 1\nline 2')
  })
})

describe('helpers', () => {
  it('strips ANSI color and title sequences', () => {
    expect(stripAnsi('\x1b[1;32mOK\x1b[0m\x1b]0;title\x07!')).toBe('OK!')
  })

  it('keeps the last write of a carriage-return-rewritten line', () => {
    expect(cleanText('progress 10%\rprogress 100%\n')).toBe('progress 100%\n')
  })

  it('finds the echo end and the last line', () => {
    expect(echoEnd('a\nfw # ls\nout\n', 'ls')).toBe(10)
    // A hostname containing the command is not its echo.
    expect(echoEnd('als-fw # \nals-fw # ls\nx\n', 'ls')).toBe(22)
    // Regex metacharacters in the command are literal.
    expect(echoEnd('fw # show | grep a.b\nx\n', 'show | grep a.b')).toBe(21)
    expect(echoEnd('fw # ls', 'ls')).toBe(-1)
    expect(lastLine('out\r\nfw1 (vdom) # ')).toBe('fw1 (vdom) # ')
  })

  it('recognises a pager waiting at the end of the output', () => {
    expect(MORE_RE.test('line\n--More-- ')).toBe(true)
    expect(MORE_RE.test(' --More--')).toBe(true)
    expect(MORE_RE.test('more text')).toBe(false)
  })
})

describe('looksLikePrompt', () => {
  it('recognises common CLI prompts', () => {
    for (const p of [
      'fw1 # ',
      'fw1 (SASE_INET) # ',
      'FGT-60F (vdom-a) # ',
      'switch>',
      'core-sw1#',
      'user@host:~$ ',
      '[admin@MikroTik] > ',
      'root@vm:~# ',
    ]) {
      expect(looksLikePrompt(p), p).toBe(true)
    }
  })

  it('rejects ordinary output and pager/disclaimer lines', () => {
    for (const p of [
      '',
      "'SASE_INET' 203.0.113.1:0  selectors(total,up): 1/0",
      'Version: FortiGate-60F v7.4.4',
      ' --More-- ',
      "(Press 'a' to accept):",
      'progress 100%',
      '#',
    ]) {
      expect(looksLikePrompt(p), p).toBe(false)
    }
  })
})

describe('read-only allowlist', () => {
  const allow = parseAllowList('get, show')

  it('allows display commands as whole words', () => {
    expect(readOnlyAllows('get system status', allow)).toBe(true)
    expect(readOnlyAllows('  SHOW  running-config ', allow)).toBe(true)
    expect(readOnlyAllows('show', allow)).toBe(true)
    expect(readOnlyAllows('', allow)).toBe(true)
  })

  it('blocks everything else', () => {
    expect(readOnlyAllows('config system interface', allow)).toBe(false)
    expect(readOnlyAllows('diagnose vpn tunnel flush SASE_INET', allow)).toBe(false)
    expect(readOnlyAllows('execute reboot', allow)).toBe(false)
    expect(readOnlyAllows('shower', allow)).toBe(false)
  })

  it('blocks chaining and redirection even after an allowed prefix', () => {
    expect(readOnlyAllows('show ip; reload', allow)).toBe(false)
    expect(readOnlyAllows('show run | tee x', allow)).toBe(false)
    expect(readOnlyAllows('show a && rm -rf /', allow)).toBe(false)
    expect(readOnlyAllows('show $(id)', allow)).toBe(false)
    expect(readOnlyAllows('show > /tmp/x', allow)).toBe(false)
  })

  it('blocks control characters that would change what the device runs', () => {
    expect(readOnlyAllows('show version\rexecute reboot', allow)).toBe(false)
    expect(readOnlyAllows('show version\nexecute reboot', allow)).toBe(false)
    expect(readOnlyAllows('show \b\b\b\b\bexecute reboot', allow)).toBe(false)
    expect(readOnlyAllows('show x\x15config system global', allow)).toBe(false)
    expect(readOnlyAllows('show \x1b[A', allow)).toBe(false)
    expect(readOnlyAllows('show\tint', allow)).toBe(false)
    expect(readOnlyAllows('get\x7f', allow)).toBe(false)
  })

  it('parses lists and falls back to the defaults', () => {
    expect(parseAllowList('get\nshow\n diagnose  sys ')).toEqual(['get', 'show', 'diagnose sys'])
    expect(parseAllowList('')).toEqual(['get', 'show'])
    expect(readOnlyAllows('diagnose sys top', parseAllowList('diagnose sys'))).toBe(true)
  })
})
