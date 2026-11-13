// Parse a MobaXterm macro into ZapTerm expect/send/wait steps.
//
// MobaXterm exposes macros in two shapes:
//
//  1. The "Macro edition" dialog line list (one keystroke/token per line) —
//     parsed by `parseMobaMacroEditor`.
//  2. The real `[Macros]` section of an exported `MobaXterm.ini`, where each
//     macro is a single line `name=step|step|…|` and each step is
//     `n1:n2:n3:payload` (payload is everything after the 3rd colon and may
//     contain encoded specials) — parsed by `parseMobaMacroExport`.
//
// Both map onto the same model:
//
//   text            → buffered as a `send` payload (typed as-is)
//   RETURN / ENTER  → append `\r` to the buffer and flush it as one `send`
//   BACK / BACKSPACE→ append a DEL (`\x7f`) to the buffer (erases a char on the
//                     remote line editor, e.g. bash readline)
//   TAB             → append `\t`
//   ESC / ESCAPE    → append `\x1b`
//   SPACE           → append a space
//   SLEEP=<ms>      → flush any pending buffer, then a `wait` step of <ms>
//
// Buffering until RETURN matters: it means a command isn't sent before it's
// fully composed (Moba builds a line with text + BACK edits, then RETURN). The
// escape sequences we emit are decoded at run time by the macro runner's
// `decodeEscapes`, so they reach the PTY as the real control bytes.

import type { LoginStep } from '$lib/bridge/types'

function sendStep(send: string): LoginStep {
  return { kind: 'send', expect: '', is_regex: false, send, timeout_ms: 10000 }
}

function waitStep(ms: number): LoginStep {
  return { kind: 'wait', expect: '', is_regex: false, send: '', timeout_ms: ms }
}

function expectStep(expect: string): LoginStep {
  return { kind: 'expect', expect, is_regex: false, send: '', timeout_ms: 30000 }
}

/** Escape backslashes in literal typed text so the runner's escape decoder
 *  doesn't reinterpret e.g. a Windows path `C:\temp` as control bytes. */
function literal(text: string): string {
  return text.replace(/\\/g, '\\\\')
}

/** A parsed macro: its display name plus the steps it produces. */
export interface ParsedMacro {
  name: string
  steps: LoginStep[]
}

/**
 * Parse the MobaXterm "Macro edition" dialog line list into ZapTerm macro steps.
 * Unrecognised non-empty lines are treated as text to type. Returns an empty
 * array for empty input.
 */
export function parseMobaMacroEditor(text: string): LoginStep[] {
  const steps: LoginStep[] = []
  let buf = ''

  const flush = () => {
    if (buf.length > 0) {
      steps.push(sendStep(buf))
      buf = ''
    }
  }

  for (const raw of text.split(/\r?\n/)) {
    const trimmed = raw.trim()
    if (trimmed === '') continue

    const sleep = /^SLEEP\s*=\s*(\d+)$/i.exec(trimmed)
    if (sleep) {
      flush()
      steps.push(waitStep(parseInt(sleep[1]!, 10)))
      continue
    }

    switch (trimmed.toUpperCase()) {
      case 'RETURN':
      case 'ENTER':
        buf += '\\r'
        flush()
        break
      case 'BACK':
      case 'BACKSPACE':
        buf += '\\x7f'
        break
      case 'TAB':
        buf += '\\t'
        break
      case 'ESC':
      case 'ESCAPE':
        buf += '\\e'
        break
      case 'SPACE':
        buf += ' '
        break
      default:
        // Literal text to type. Use the raw line (sans surrounding whitespace
        // from the editor) and escape backslashes so they survive decoding.
        buf += literal(trimmed)
    }
  }

  flush()
  return steps
}

/** Decode MobaXterm's payload escapes. Single replace table; unknown `__X__`
 *  tokens pass through untouched. `__DBLDOT__` is confirmed; the rest are
 *  best-effort. */
function decodeMobaPayload(payload: string): string {
  return payload
    .replace(/__DBLDOT__/g, ':')
    .replace(/__PIPE__/g, '|')
    .replace(/__EQUAL__/g, '=')
    .replace(/__PERCENT__/g, '%')
}

/**
 * Parse one exported macro value (`step|step|…|`) into ZapTerm steps. Each step is
 * `n1:n2:n3:payload`; we only care about the payload (4th colon-part onward).
 * Malformed segments with fewer than 4 colon-parts never throw — the remainder
 * is treated as the payload/text.
 */
function parseExportSteps(value: string): LoginStep[] {
  const steps: LoginStep[] = []
  let buf = ''

  const flush = () => {
    if (buf.length > 0) {
      steps.push(sendStep(buf))
      buf = ''
    }
  }

  // Split on `|`; the trailing `|` yields a final empty segment we skip.
  for (const segment of value.split('|')) {
    if (segment === '') continue
    // Payload is everything after the 3rd colon. With fewer parts, take what
    // remains after the colons we do have (tolerant of `12:0:0password`).
    const parts = segment.split(':')
    const payloadRaw = parts.length >= 4 ? parts.slice(3).join(':') : parts[parts.length - 1]!
    const payload = decodeMobaPayload(payloadRaw)

    const sleep = /^SLEEPEQUAL(\d+)$/.exec(payload)
    if (sleep) {
      flush()
      steps.push(waitStep(parseInt(sleep[1]!, 10)))
      continue
    }
    const wait = /^WAITFOREQUAL([\s\S]*)$/.exec(payload)
    if (wait) {
      flush()
      steps.push(expectStep(wait[1]!))
      continue
    }
    if (payload === 'RETURN') {
      buf += '\\r'
      flush()
      continue
    }
    // Anything else is literal text appended to the current send buffer.
    buf += literal(payload)
  }

  flush()
  return steps
}

/** Strip a leading transport prefix (`ssh `/`telnet `) from a macro name. */
function stripTransportPrefix(name: string): string {
  return name.replace(/^(?:ssh|telnet)\s+/i, '')
}

/**
 * Parse the exported `[Macros]` ini format. Each non-blank, non-`[section]`
 * line is `name=step|step|…|`. Returns one `ParsedMacro` per line.
 */
export function parseMobaMacroExport(text: string): ParsedMacro[] {
  const macros: ParsedMacro[] = []
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim()
    if (line === '' || /^\[[^\]]*\]$/.test(line)) continue
    const eq = line.indexOf('=')
    if (eq < 0) continue
    const rawName = line.slice(0, eq)
    const value = line.slice(eq + 1)
    macros.push({
      name: stripTransportPrefix(rawName.trim()),
      steps: parseExportSteps(value),
    })
  }
  return macros
}

/** True when `text` looks like the exported `[Macros]` ini format: at least one
 *  non-blank, non-`[section]` line of the form `name=...|`. */
function isExportFormat(text: string): boolean {
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim()
    if (line === '' || /^\[[^\]]*\]$/.test(line)) continue
    if (/^[^=\n]+=.*\|/.test(line)) return true
  }
  return false
}

/**
 * Parse MobaXterm macro text, auto-detecting the format. Exported `[Macros]`
 * lines yield one macro each (named); the editor line list yields a single
 * unnamed macro.
 */
export function parseMobaMacros(text: string): ParsedMacro[] {
  if (isExportFormat(text)) {
    return parseMobaMacroExport(text)
  }
  return [{ name: '', steps: parseMobaMacroEditor(text) }]
}

/**
 * Thin wrapper returning the first parsed macro's steps. Kept for existing
 * callers/tests that work with a single step list.
 */
export function parseMobaMacro(text: string): LoginStep[] {
  const macros = parseMobaMacros(text)
  return macros[0]?.steps ?? []
}
