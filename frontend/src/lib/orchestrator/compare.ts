// Pure logic behind "Run & compare": what counts as a pane's output, which
// panes produced the same output, how to tell panes apart, and what may have
// made a capture incomplete. Kept free of Svelte so it can be unit-tested.

export type HostState = 'running' | 'done' | 'timeout'

/** What the comparison needs to know about one pane. */
export interface CompareHost {
  sessionId: string
  /** The pane's own name (usually the saved session's name). */
  label: string
  /** 1-based position of the pane in the tab, left-to-right / top-to-bottom. */
  position: number
  /** `host:port` of the device, or null for a local shell. */
  target: string | null
  state: HostState
  /** ANSI-stripped output captured since the command was sent. */
  buffer: string
}

/** One distinct output and the panes that produced it. */
export interface Variant {
  /** "A" for the most common output, then "B", "C"… */
  letter: string
  text: string
  hosts: CompareHost[]
}

export interface Comparison {
  variants: Variant[]
  /** Panes that timed out without sending anything back. */
  noReply: CompareHost[]
}

/**
 * The part of a capture that is compared: without the echoed command (first
 * line), the prompt that came back (last non-empty line) and spaces at line
 * ends, with line endings unified.
 */
export function normalizeOutput(buffer: string, command: string): string {
  let lines = buffer.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n')
  if (lines.length && command && lines[0]!.includes(command)) lines = lines.slice(1)
  while (lines.length && lines[lines.length - 1]!.trim() === '') lines.pop()
  if (lines.length) lines = lines.slice(0, -1) // the prompt that came back
  return lines
    .map((l) => l.replace(/\s+$/, ''))
    .join('\n')
    .trim()
}

// "--More--" (Cisco IOS, Arista, FortiOS), "---(more)---" / "---(more 45%)---"
// (Junos), "<--- More --->" (Cisco ASA). Only the last line of a capture is
// looked at.
const PAGER_RE = /(?:^|\s)(?:<?-{2,}\s*\(?\s*more\b[^\n]*?\)?\s*-{2,}>?)\s*$/i

/** True when the capture stopped at a pager prompt such as `--More--`. */
export function endsAtPager(buffer: string): boolean {
  const lines = buffer.replace(/\r/g, '\n').split('\n')
  for (let i = lines.length - 1; i >= 0; i--) {
    const l = lines[i]!.trim()
    if (l) return PAGER_RE.test(l)
  }
  return false
}

/** Group the panes by identical output, most common first. */
export function compareOutputs(hosts: CompareHost[], command: string): Comparison {
  const map = new Map<string, CompareHost[]>()
  const noReply: CompareHost[] = []
  for (const h of hosts) {
    const text = normalizeOutput(h.buffer, command)
    if (h.state === 'timeout' && text === '') {
      noReply.push(h)
      continue
    }
    const list = map.get(text)
    if (list) list.push(h)
    else map.set(text, [h])
  }
  const variants = [...map.entries()]
    .map(([text, hs]) => ({ text, hosts: hs }))
    // Most panes first; on a tie, the one holding the left-most pane.
    .sort(
      (a, b) =>
        b.hosts.length - a.hosts.length ||
        Math.min(...a.hosts.map((h) => h.position)) - Math.min(...b.hosts.map((h) => h.position)),
    )
    .map((v, i) => ({ letter: String.fromCharCode(65 + Math.min(i, 25)), ...v }))
  return { variants, noReply }
}

/**
 * A name per pane that tells it apart from the others: its label, plus
 * "(pane N)" when another pane has the same label.
 */
export function displayNames(hosts: CompareHost[]): Map<string, string> {
  const count = new Map<string, number>()
  for (const h of hosts) count.set(h.label, (count.get(h.label) ?? 0) + 1)
  const out = new Map<string, string>()
  for (const h of hosts) {
    out.set(
      h.sessionId,
      (count.get(h.label) ?? 0) > 1 ? `${h.label} (pane ${h.position})` : h.label,
    )
  }
  return out
}

/** Sets of two or more panes connected to the same `host:port`. */
export function sameDeviceSets(hosts: CompareHost[]): { target: string; hosts: CompareHost[] }[] {
  const map = new Map<string, CompareHost[]>()
  for (const h of hosts) {
    if (!h.target) continue
    const key = h.target.toLowerCase()
    const list = map.get(key)
    if (list) list.push(h)
    else map.set(key, [h])
  }
  return [...map.values()]
    .filter((hs) => hs.length > 1)
    .map((hs) => ({ target: hs[0]!.target!, hosts: hs }))
}

/** "a", "a and b", "a, b and c". */
export function joinNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? ''
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`
}

export type DiffRow = { kind: 'same' | 'a' | 'b'; text: string } | { kind: 'gap'; count: number }

/**
 * Keep the changed lines and `context` unchanged lines around each change;
 * longer runs of unchanged lines become one `gap` row ("12 identical lines").
 */
export function collapseSame(
  lines: { kind: 'same' | 'a' | 'b'; text: string }[],
  context = 3,
): DiffRow[] {
  const keep = new Array<boolean>(lines.length).fill(false)
  lines.forEach((l, i) => {
    if (l.kind === 'same') return
    for (let j = Math.max(0, i - context); j <= Math.min(lines.length - 1, i + context); j++) {
      keep[j] = true
    }
  })
  const out: DiffRow[] = []
  let gap = 0
  lines.forEach((l, i) => {
    if (keep[i]) {
      if (gap) out.push({ kind: 'gap', count: gap })
      gap = 0
      out.push(l)
    } else gap++
  })
  if (gap) out.push({ kind: 'gap', count: gap })
  return out
}

/** What `paneTarget` needs to know about a pane. */
export interface PaneEndpoint {
  savedSession?: { protocol: string; host: string | null; port: number | null }
  ssh?: { host: string; port: number }
  telnet?: { host: string; port: number }
}

const DEFAULT_PORT: Record<string, number> = { ssh: 22, telnet: 23 }

/** `host:port` of the device a pane is connected to; null for local shells
 *  and serial consoles. */
export function paneTarget(p: PaneEndpoint): string | null {
  if (p.ssh) return `${p.ssh.host}:${p.ssh.port}`
  if (p.telnet) return `${p.telnet.host}:${p.telnet.port}`
  const s = p.savedSession
  if (!s?.host) return null
  const def = DEFAULT_PORT[s.protocol]
  if (def === undefined) return null
  return `${s.host}:${s.port ?? def}`
}
