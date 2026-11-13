// Pure helpers for the script host's `run()` — kept free of Svelte runes and
// Tauri so they are unit-testable under Vitest's node environment.

/** Strip ANSI/OSC escape sequences (colors, cursor moves, titles). */
export function stripAnsi(s: string): string {
  return s
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')
    .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '')
    .replace(/\x1b[@-Z\\-_]/g, '')
}

/** A pager prompt at the very end of the output (FortiOS / Cisco style). */
export const MORE_RE = /-+\s*More\s*-+\s*$/i

/**
 * Normalise terminal text: CRLF/CR → LF, backspace erasure applied (pagers
 * wipe their "--More--" with `\b`), pager prompts left mid-stream removed.
 */
export function cleanText(s: string): string {
  let t = s.replace(/\r\n/g, '\n')
  // Apply backspaces: "abc\b\bX" → "aX".
  while (/[^\n]\x08/.test(t)) t = t.replace(/[^\n]\x08/g, '')
  t = t.replace(/\x08/g, '')
  // A carriage return mid-line rewinds it; keep what was written last.
  t = t
    .split('\n')
    .map((line) => {
      const parts = line.split('\r')
      return parts[parts.length - 1] ?? ''
    })
    .join('\n')
  return t.replace(/[ \t]*-+\s*More\s*-+[ \t]*/gi, '')
}

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * Where the echo of `command` ends in `text` (index just past its line), or
 * -1. The echo is the command at the END of a line (after the prompt), so a
 * hostname that merely contains the command text doesn't count.
 */
export function echoEnd(text: string, command: string): number {
  const m = new RegExp(`${escapeRe(command)}[ \\t]*\\n`).exec(text)
  return m ? m.index + m[0].length : -1
}

/**
 * The output of `command` inside `raw` (everything the session printed since
 * the command was sent): the text after the echoed command line, without the
 * final prompt line that ended it. `null` while the echo hasn't arrived.
 */
export function extractRunOutput(raw: string, command: string): string | null {
  const text = cleanText(raw)
  const start = echoEnd(text, command)
  if (start < 0) return null
  const body = text.slice(start)
  // The last line is the prompt that came back (no trailing newline).
  const lastNl = body.lastIndexOf('\n')
  const out = lastNl < 0 ? '' : body.slice(0, lastNl)
  return out.replace(/\s+$/, '')
}

/** The last (partial) line of `raw`, cleaned — what prompt detection looks at. */
export function lastLine(raw: string): string {
  const text = cleanText(raw)
  return text.slice(text.lastIndexOf('\n') + 1)
}

/**
 * Whether `line` (the last, unterminated line) looks like a CLI prompt —
 * `fw1 # `, `fw1 (SASE_INET) # `, `switch> `, `user@host:~$ `, `[admin@mt] > `.
 * Used when the platform is unknown (telnet, new devices) so `run()` doesn't
 * have to wait for the full quiet period after every command.
 */
export function looksLikePrompt(line: string): boolean {
  const t = line.replace(/\s+$/, '')
  if (t.length === 0 || t.length > 120) return false
  return /^[\w.\-@:~/[\]() ]*[\w)\]~][ \t]?[#>$]$/.test(t)
}

/** Read-only mode's default allowlist: commands that only display. */
export const DEFAULT_READ_ONLY_ALLOW = ['get', 'show']

/** The allowlist as typed ("get, show" / one per line) → lowercase prefixes. */
export function parseAllowList(text: string | null | undefined): string[] {
  const items = (text ?? '')
    .split(/[,\n]/)
    .map((p) => p.trim().toLowerCase().replace(/\s+/g, ' '))
    .filter(Boolean)
  return items.length > 0 ? items : [...DEFAULT_READ_ONLY_ALLOW]
}

/**
 * Read-only mode: may `command` be typed? Only if it starts with one of
 * `allow` as whole words (`show` allows `show run`, not `shower`), and holds
 * nothing that could chain or redirect a second command on a shell-like CLI.
 * Control characters are refused outright: a CR/LF would end the checked line
 * and start another, and backspace, Ctrl-U/W, Tab or an escape sequence (an
 * arrow key recalling history) would make the device run something other
 * than what was checked. An empty line (a bare Enter) is always allowed.
 */
export function readOnlyAllows(command: string, allow: string[]): boolean {
  if (/[\u0000-\u001f\u007f-\u009f]/.test(command)) return false
  const c = command.trim().toLowerCase().replace(/ +/g, ' ')
  if (c === '') return true
  if (/[;|&`<>]|\$\(/.test(c)) return false
  return allow.some((p) => c === p || c.startsWith(p + ' '))
}
