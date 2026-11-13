import { describe, it, expect } from 'vitest'
import { parseMobaMacro, parseMobaMacros, parseMobaMacroExport } from './mobaMacro'

describe('parseMobaMacro', () => {
  it('parses the real MobaXterm macro from the editor', () => {
    // The macro from MobaXterm's "Macro edition" dialog (screenshot): build a
    // command with a BACK edit, RETURN, then timed prompts.
    const macro = [
      'ssh t769',
      'BACK',
      '0784@213.0.143.226',
      'RETURN',
      'SLEEP=1200',
      'yes',
      'RETURN',
      'SLEEP=1200',
      'RETURN',
      'SLEEP=1200',
      'a',
    ].join('\n')

    expect(parseMobaMacro(macro)).toEqual([
      // text + BACK(\x7f) + text, flushed on RETURN(\r) as a single send
      {
        kind: 'send',
        expect: '',
        is_regex: false,
        send: 'ssh t769\\x7f0784@213.0.143.226\\r',
        timeout_ms: 10000,
      },
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 1200 },
      { kind: 'send', expect: '', is_regex: false, send: 'yes\\r', timeout_ms: 10000 },
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 1200 },
      { kind: 'send', expect: '', is_regex: false, send: '\\r', timeout_ms: 10000 },
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 1200 },
      // trailing text with no RETURN — flushed at EOF (runner appends Enter)
      { kind: 'send', expect: '', is_regex: false, send: 'a', timeout_ms: 10000 },
    ])
  })

  it('maps special tokens and is case-insensitive', () => {
    const steps = parseMobaMacro('echo hi\nTab\nenter\nsleep = 500')
    expect(steps).toEqual([
      { kind: 'send', expect: '', is_regex: false, send: 'echo hi\\t\\r', timeout_ms: 10000 },
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 500 },
    ])
  })

  it('escapes literal backslashes in typed text', () => {
    const steps = parseMobaMacro('cd C:\\temp\nRETURN')
    expect(steps).toEqual([
      { kind: 'send', expect: '', is_regex: false, send: 'cd C:\\\\temp\\r', timeout_ms: 10000 },
    ])
  })

  it('returns nothing for empty input', () => {
    expect(parseMobaMacro('')).toEqual([])
    expect(parseMobaMacro('\n  \n')).toEqual([])
  })
})

describe('parseMobaMacroExport', () => {
  it('parses the real MobaXterm [Macros] export line', () => {
    const line =
      'ssh FG-900G_eq3290=0:0:0:SLEEPEQUAL2400|12:2:0:ssh t5555@222.0.143.226|258:13:1835009:RETURN|0:0:0:WAITFOREQUALpassword__DBLDOT__|12:0:0:passwordl|258:13:1835009:RETURN|0:0:0:SLEEPEQUAL1200|12:4:0:a|258:13:1835009:RETURN|'

    const macros = parseMobaMacroExport(line)
    expect(macros).toHaveLength(1)
    expect(macros[0]!.name).toBe('FG-900G_eq3290') // ssh prefix stripped
    expect(macros[0]!.steps).toEqual([
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 2400 },
      {
        kind: 'send',
        expect: '',
        is_regex: false,
        send: 'ssh t5555@222.0.143.226\\r',
        timeout_ms: 10000,
      },
      { kind: 'expect', expect: 'password:', is_regex: false, send: '', timeout_ms: 30000 },
      { kind: 'send', expect: '', is_regex: false, send: 'passwordl\\r', timeout_ms: 10000 },
      { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: 1200 },
      { kind: 'send', expect: '', is_regex: false, send: 'a\\r', timeout_ms: 10000 },
    ])
  })

  it('decodes __DBLDOT__ and other escapes in payloads', () => {
    const macros = parseMobaMacroExport('m=0:0:0:WAITFOREQUALuser__DBLDOT__|12:0:0:a__PIPE__b|')
    expect(macros[0]!.steps).toEqual([
      { kind: 'expect', expect: 'user:', is_regex: false, send: '', timeout_ms: 30000 },
      { kind: 'send', expect: '', is_regex: false, send: 'a|b', timeout_ms: 10000 },
    ])
  })

  it('tolerates malformed segments with fewer than 4 colon-parts', () => {
    // `12:0:0password` has only 3 colon-parts; the remainder (`0password`) is
    // treated as payload/text rather than throwing.
    const macros = parseMobaMacroExport('m=12:0:0password|258:13:1835009:RETURN|')
    expect(macros[0]!.steps).toEqual([
      { kind: 'send', expect: '', is_regex: false, send: '0password\\r', timeout_ms: 10000 },
    ])
  })

  it('parses multiple macros', () => {
    const text = ['[Macros]', 'ssh a=12:0:0:foo|258:13:0:RETURN|', 'telnet b=12:0:0:bar|'].join(
      '\n',
    )
    const macros = parseMobaMacroExport(text)
    expect(macros).toHaveLength(2)
    expect(macros[0]!.name).toBe('a')
    expect(macros[1]!.name).toBe('b')
    expect(macros[1]!.steps).toEqual([
      { kind: 'send', expect: '', is_regex: false, send: 'bar', timeout_ms: 10000 },
    ])
  })
})

describe('parseMobaMacros auto-detect', () => {
  it('uses the export parser for export-format input', () => {
    const macros = parseMobaMacros('ssh host=0:0:0:SLEEPEQUAL500|12:0:0:x|258:13:0:RETURN|')
    expect(macros).toHaveLength(1)
    expect(macros[0]!.name).toBe('host')
    expect(macros[0]!.steps[0]).toEqual({
      kind: 'wait',
      expect: '',
      is_regex: false,
      send: '',
      timeout_ms: 500,
    })
  })

  it('falls back to the editor parser for the line-list format', () => {
    const macros = parseMobaMacros('echo hi\nRETURN')
    expect(macros).toHaveLength(1)
    expect(macros[0]!.name).toBe('')
    expect(macros[0]!.steps).toEqual([
      { kind: 'send', expect: '', is_regex: false, send: 'echo hi\\r', timeout_ms: 10000 },
    ])
  })
})
