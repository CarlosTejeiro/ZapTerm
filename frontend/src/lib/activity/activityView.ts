// Pure logic behind the Activity view: what an event is, how events group
// into one client's visit, what the filters keep and what the counters say.
// Free of Svelte and Tauri so it can be unit-tested.

/** `unknown`: a client that connected without saying what it is (an older
 *  copy of zapterm.py still running). */
export type ActivitySource = 'script' | 'notebook' | 'mcp' | 'unknown'

/** What the script host reports; the backend stamps the time. */
export interface NewActivity {
  source: ActivitySource
  /** Script file name, `Jupyter` / `Python`, or the MCP client's name. */
  client: string
  /** Tab name; empty for connection events. */
  device: string
  /** `host:port` of the device, when known. */
  address: string
  /** `run`, `send`, `secret`, `open`, `close` (shown as rows) or `connect`,
   *  `disconnect`, `start`, `finish` (where a visit begins and ends). */
  action: string
  /** `sent`, `approved` (asked, allowed, sent), `refused` (read-only),
   *  `declined` (asked, not allowed), `timeout`, `error`, `ok`. */
  result: string
  seconds?: number | null
  /** The command line, or what happened. */
  detail: string
}

export interface ActivityEntry extends NewActivity {
  /** Local time with its UTC offset, e.g. `2026-11-24T13:05:12+01:00`. */
  time: string
}

/** How an action ended, for the counters and the result filter. */
export type Outcome = 'sent' | 'blocked' | 'declined' | 'failed'

const BOUNDARY = new Set(['connect', 'disconnect', 'start', 'finish'])

/** True for the events shown as rows (not where a visit begins or ends). */
export function isRow(e: ActivityEntry): boolean {
  return !BOUNDARY.has(e.action)
}

export function outcomeOf(e: ActivityEntry): Outcome {
  switch (e.result) {
    case 'refused':
      return 'blocked'
    case 'declined':
      return 'declined'
    case 'timeout':
    case 'error':
      return 'failed'
    default:
      return 'sent'
  }
}

/** The local day an event was recorded on, `YYYY-MM-DD`. */
export const dayOf = (e: ActivityEntry): string => e.time.slice(0, 10)
/** `HH:MM:SS`, local to where it was recorded. */
export const clockOf = (time: string): string => time.slice(11, 19)

export interface ActivityFilter {
  source: ActivitySource | 'all'
  outcome: Outcome | 'all'
  /** Tab name, or '' for every device. */
  device: string
  /** `YYYY-MM-DD`, or '' for every day. */
  day: string
  search: string
}

export const NO_FILTER: ActivityFilter = {
  source: 'all',
  outcome: 'all',
  device: '',
  day: '',
  search: '',
}

/** Whether the row `e` passes `f`; `ignore` leaves one filter out (the
 *  counters and the day strip show what each choice would give). */
export function matches(
  e: ActivityEntry,
  f: ActivityFilter,
  ignore: 'outcome' | 'day' | null = null,
): boolean {
  if (f.source !== 'all' && e.source !== f.source) return false
  if (ignore !== 'outcome' && f.outcome !== 'all' && outcomeOf(e) !== f.outcome) return false
  if (f.device && e.device !== f.device) return false
  if (ignore !== 'day' && f.day && dayOf(e) !== f.day) return false
  const q = f.search.trim().toLowerCase()
  if (q) {
    const hay = `${e.detail}\n${e.device}\n${e.address}\n${e.client}`.toLowerCase()
    if (!hay.includes(q)) return false
  }
  return true
}

export interface ActivityStats {
  all: number
  sent: number
  blocked: number
  declined: number
  failed: number
}

export function statsOf(rows: ActivityEntry[]): ActivityStats {
  const s: ActivityStats = { all: rows.length, sent: 0, blocked: 0, declined: 0, failed: 0 }
  for (const r of rows) s[outcomeOf(r)]++
  return s
}

/** One client's visit: a script run, or a notebook / assistant from the
 *  moment it connected until it left. */
export interface ActivityGroup {
  key: string
  source: ActivitySource
  client: string
  day: string
  /** Times of its first and last event. */
  start: string
  end: string
  rows: ActivityEntry[]
  /** How it began or ended, when that says something ("dry run", "exit code 1"). */
  note: string
  /** No disconnect / finish seen: it may still be going on. */
  open: boolean
}

/** A pause this long between two events of the same client starts a new visit. */
const GAP_MS = 30 * 60 * 1000

/**
 * Split the events (oldest first) into visits. A new one starts when a client
 * connects or a script starts, when the client or the day changes, after one
 * ended, or after a long pause.
 */
export function groupActivity(entries: ActivityEntry[]): ActivityGroup[] {
  const groups: ActivityGroup[] = []
  let cur: ActivityGroup | null = null
  let lastAt = 0
  for (const e of entries) {
    const at = Date.parse(e.time)
    const starts = e.action === 'connect' || e.action === 'start'
    const fits =
      cur !== null &&
      cur.open &&
      !starts &&
      cur.source === e.source &&
      cur.client === e.client &&
      cur.day === dayOf(e) &&
      at - lastAt <= GAP_MS
    if (!fits || cur === null) {
      cur = {
        key: `${e.time}#${groups.length}`,
        source: e.source,
        client: e.client,
        day: dayOf(e),
        start: e.time,
        end: e.time,
        rows: [],
        note: '',
        open: true,
      }
      groups.push(cur)
    }
    cur.end = e.time
    lastAt = at
    if (isRow(e)) cur.rows.push(e)
    else if (e.action === 'connect' && e.result === 'refused') cur.note = e.detail
    else if (e.action === 'start') cur.note = e.detail
    else if (e.action === 'finish') {
      cur.note = [cur.note, e.detail].filter(Boolean).join(' · ')
      cur.open = false
    } else if (e.action === 'disconnect') cur.open = false
  }
  return groups
}

export interface DayGroups {
  day: string
  groups: ActivityGroup[]
}

/**
 * The visits to show under `f`, newest day and newest visit first, each with
 * only its matching rows (in the order they happened). A visit with no
 * matching row is left out, unless nothing narrows the rows (then a visit
 * that sent nothing is still worth seeing).
 */
export function visibleDays(groups: ActivityGroup[], f: ActivityFilter): DayGroups[] {
  const narrowing = f.outcome !== 'all' || f.device !== '' || f.search.trim() !== ''
  const days = new Map<string, ActivityGroup[]>()
  for (let i = groups.length - 1; i >= 0; i--) {
    const g = groups[i]!
    if (f.source !== 'all' && g.source !== f.source) continue
    if (f.day && g.day !== f.day) continue
    const rows = g.rows.filter((r) => matches(r, f))
    if (rows.length === 0 && narrowing) continue
    const list = days.get(g.day)
    const shown = { ...g, rows }
    if (list) list.push(shown)
    else days.set(g.day, [shown])
  }
  return [...days.entries()].map(([day, gs]) => ({ day, groups: gs }))
}

/**
 * Keep the newest visits until about `maxRows` rows are shown (a visit is
 * never cut in two); `hidden` is how many rows were left out.
 */
export function limitDays(
  days: DayGroups[],
  maxRows: number,
): { days: DayGroups[]; hidden: number } {
  const out: DayGroups[] = []
  let shown = 0
  let hidden = 0
  for (const d of days) {
    const kept: ActivityGroup[] = []
    for (const g of d.groups) {
      if (shown < maxRows || shown === 0) {
        kept.push(g)
        shown += g.rows.length
      } else hidden += g.rows.length
    }
    if (kept.length) out.push({ day: d.day, groups: kept })
  }
  return { days: out, hidden }
}

const pad = (n: number) => String(n).padStart(2, '0')
/** `YYYY-MM-DD` of a local date. */
export const isoDay = (d: Date): string =>
  `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`

export interface DayCount {
  day: string
  count: number
}

/** Rows per day for the `days` days ending `today` (oldest first). */
export function dayCounts(rows: ActivityEntry[], today: Date, days = 14): DayCount[] {
  const counts = new Map<string, number>()
  for (const r of rows) counts.set(dayOf(r), (counts.get(dayOf(r)) ?? 0) + 1)
  const out: DayCount[] = []
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i)
    const day = isoDay(d)
    out.push({ day, count: counts.get(day) ?? 0 })
  }
  return out
}

/** "Today", "Yesterday" or "Tue 24 Nov 2026". */
export function dayLabel(day: string, today: Date): string {
  if (day === isoDay(today)) return 'Today'
  const y = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1)
  if (day === isoDay(y)) return 'Yesterday'
  const d = new Date(`${day}T00:00:00`)
  return d.toLocaleDateString('en-GB', {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  })
}

export const SOURCE_LABEL: Record<ActivitySource, string> = {
  mcp: 'AI assistant',
  notebook: 'Notebook',
  script: 'Script',
  unknown: 'Unidentified client',
}

/** "0.4 s", "12 s", "1 min 05 s"; '' when unknown. */
export function durationText(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) return ''
  if (seconds < 10) return `${seconds.toFixed(1)} s`
  if (seconds < 60) return `${Math.round(seconds)} s`
  const s = Math.round(seconds)
  return `${Math.floor(s / 60)} min ${pad(s % 60)} s`
}

export const RESULT_LABEL: Record<string, string> = {
  sent: 'Sent',
  ok: 'Done',
  approved: 'Approved · sent',
  refused: 'Blocked · read-only',
  declined: 'Declined by you',
  timeout: 'No prompt in time',
  error: 'Error',
}

/** What a row did, as a short tag before its text ('' for a plain command). */
export function actionTag(e: ActivityEntry): string {
  switch (e.action) {
    case 'send':
      return 'send'
    case 'secret':
      return 'vault'
    case 'open':
      return 'open session'
    case 'close':
      return 'close tab'
    default:
      return ''
  }
}

/** The rows as plain text, one per line, for copying. */
export function rowsAsText(days: DayGroups[]): string {
  const lines: string[] = []
  for (const d of [...days].reverse()) {
    for (const g of [...d.groups].reverse()) {
      for (const r of g.rows) {
        const who = `${SOURCE_LABEL[r.source]}${r.client ? ` (${r.client})` : ''}`
        const tag = actionTag(r)
        lines.push(
          [
            r.time,
            who,
            r.device + (r.address ? ` [${r.address}]` : ''),
            RESULT_LABEL[r.result] ?? r.result,
            (tag ? `${tag}: ` : '') + r.detail,
          ].join('\t'),
        )
      }
    }
  }
  return lines.join('\n') + (lines.length ? '\n' : '')
}
