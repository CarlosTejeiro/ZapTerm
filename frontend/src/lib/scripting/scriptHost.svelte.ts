// Frontend side of Python scripting: carries out the `zapterm` API calls a
// script makes (relayed by the backend as `script-request` events) on real
// terminal tabs, and keeps the run's log + summary for the ScriptPanel.
//
// Output is captured from the same `terminal-data` stream the terminals
// render, per session, from the moment the script first touches it. `run()`
// sends a command and completes when the echo has come back and then either
// the platform's prompt is recognised (backend prompt regexes) or the output
// goes quiet for SETTLE_MS. Pagers (`--More--`) are answered with a space.
//
// App.svelte supplies the few things only it knows (focused tab, opening a
// saved session into a tab, closing one, confirmations) through `ScriptHostApi`.
//
// The same host serves a notebook connection (Tools → Allow notebook
// connection): instead of a script ZapTerm started, a notebook kernel attaches
// with `zapterm.connect()`; its calls arrive the same way.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { onTerminalData } from '$lib/bridge/events'
import { getSetting, promptReturned, sendVaultField } from '$lib/bridge/commands'
import { paneToSession } from '$lib/stores/sessionRuntime.svelte'
import {
  DEFAULT_READ_ONLY_ALLOW,
  extractRunOutput,
  lastLine,
  looksLikePrompt,
  MORE_RE,
  parseAllowList,
  readOnlyAllows,
  stripAnsi,
} from './output'
import { errorText } from '$lib/bridge/errors'

/** Output must stay quiet this long (after the echo) to count as finished
 *  when no prompt is recognised. */
const SETTLE_MS = 2000
/** Quiet time before asking whether the tail is a prompt. */
const PROMPT_QUIET_MS = 150
/** After open(): wait for this much silence before handing the tab over. */
const OPEN_SETTLE_MS = 2000
/** Quiet time after which a prompt-shaped last line (`looksLikePrompt`) ends a
 *  command when the platform's own prompt regex isn't available. */
const PROMPT_GUESS_MS = 400

export interface SessionInfo {
  id: string
  name: string
  platform: string | null
}

export interface ScriptHostApi {
  /** The focused tab's live session, or null. */
  focusedSession(): SessionInfo | null
  /** Every open tab's live session, in tab order. */
  openTabs(): SessionInfo[]
  /** Name + platform of a live session the script uses. */
  describe(sessionId: string): SessionInfo | null
  /** Reload the stored platform behind `sessionId` (before `describe`). */
  refreshPlatform(sessionId: string): Promise<void>
  /** Saved session names, optionally only those under `folder`. */
  savedNames(folder: string | null): string[]
  /** Open the saved session called `name` in a new tab; returns its pane id. */
  openSaved(name: string): number
  /** Connection status of a pane opened with `openSaved`. */
  paneStatus(paneId: number): 'connecting' | 'connected' | 'error' | 'lost' | 'closed'
  /** Close the tab holding `sessionId`. */
  closeSession(sessionId: string): void
  /** Ask the user a yes/no question. */
  confirm(message: string): Promise<boolean>
}

/** Script: running → done / failed / stopped. Notebook connection:
 *  waiting ⇄ connected → off. */
export type RunStatus = 'running' | 'done' | 'failed' | 'stopped' | 'waiting' | 'connected' | 'off'

export interface LogLine {
  stream: 'stdout' | 'stderr' | 'zapterm'
  line: string
}

export interface ReportRow {
  target: string
  status: string
  detail: string
}

/** Reactive state of the current (or last) run, read by ScriptPanel. */
export const scriptRun = $state({
  mode: 'script' as 'script' | 'notebook',
  runId: null as string | null,
  path: '',
  dryRun: false,
  /** Read-only: only commands starting with one of `allow` are typed; the
   *  rest are logged as "would run". Toggleable live in notebook mode. */
  readOnly: false,
  allow: [...DEFAULT_READ_ONLY_ALLOW] as string[],
  python: '',
  status: 'done' as RunStatus,
  exitCode: null as number | null,
  log: [] as LogLine[],
  report: [] as ReportRow[],
  visible: false,
  /** Notebook connection: where `zapterm.connect()` finds it, the folder with
   *  `zapterm.py`, and the idle limit (minutes, 0 = never). */
  connectionFile: '',
  moduleDir: '',
  idleMinutes: 0,
})

/** A script running or a notebook connection on: only one at a time. */
export function hostBusy(): boolean {
  return (
    scriptRun.status === 'running' ||
    scriptRun.status === 'waiting' ||
    scriptRun.status === 'connected'
  )
}

/** Thrown inside the host to send a `{kind, message, output}` error back. */
class ApiError extends Error {
  constructor(
    public kind: 'error' | 'timeout',
    message: string,
    public output = '',
  ) {
    super(message)
  }
}

/** Per-session capture: everything printed since the script attached. */
interface Tap {
  raw: string
  lastData: number
  decoder: TextDecoder
}

let api: ScriptHostApi | null = null
let unlisteners: UnlistenFn[] = []
const taps = new Map<string, Tap>()
const platforms = new Map<string, string>()
let aborted = false
/** Requests being carried out, and those the client gave up on (a stopped
 *  notebook cell sends `cancel`, a dropped connection cancels them all). */
const inFlight = new Set<number>()
const cancelled = new Set<number>()

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

function log(line: string, stream: LogLine['stream'] = 'zapterm') {
  scriptRun.log.push({ stream, line })
  if (scriptRun.log.length > 5000) scriptRun.log.splice(0, scriptRun.log.length - 5000)
}

function checkAborted(reqId?: number) {
  if (aborted) throw new ApiError('error', 'script stopped')
  if (reqId !== undefined && cancelled.has(reqId)) throw new ApiError('error', 'cancelled')
}

/** Install the host once (App.svelte onMount). Returns the teardown. */
export async function initScriptHost(hostApi: ScriptHostApi): Promise<() => void> {
  api = hostApi
  unlisteners.push(
    await onTerminalData((p) => {
      const tap = taps.get(p.session_id)
      if (!tap) return
      tap.raw += stripAnsi(tap.decoder.decode(new Uint8Array(p.data), { stream: true }))
      if (tap.raw.length > 256 * 1024) tap.raw = tap.raw.slice(-256 * 1024)
      tap.lastData = Date.now()
    }),
    await listen<{ session_id: string; platform: string }>('session-platform-detected', (e) => {
      platforms.set(e.payload.session_id, e.payload.platform)
    }),
    await listen<{
      run_id: string
      req_id: number
      method: string
      params: Record<string, unknown>
    }>('script-request', (e) => {
      if (e.payload.run_id !== scriptRun.runId) return
      void handle(e.payload.run_id, e.payload.req_id, e.payload.method, e.payload.params ?? {})
    }),
    await listen<{ run_id: string; stream: 'stdout' | 'stderr'; line: string }>(
      'script-output',
      (e) => {
        if (e.payload.run_id === scriptRun.runId) log(e.payload.line, e.payload.stream)
      },
    ),
    await listen<{ run_id: string; status: 'connected' | 'waiting' }>('script-status', (e) => {
      if (e.payload.run_id !== scriptRun.runId) return
      if (e.payload.status === 'connected') {
        scriptRun.status = 'connected'
        log('Notebook connected.')
      } else {
        scriptRun.status = 'waiting'
        for (const id of inFlight) cancelled.add(id)
        log('Notebook disconnected — waiting for a notebook…')
      }
    }),
    await listen<{ run_id: string; code: number | null }>('script-exit', (e) => {
      if (e.payload.run_id !== scriptRun.runId) return
      if (scriptRun.mode === 'notebook') {
        aborted = true
        scriptRun.status = 'off'
        log('Notebook connection turned off.')
        taps.clear()
        return
      }
      scriptRun.exitCode = e.payload.code
      if (scriptRun.status === 'running') {
        scriptRun.status = e.payload.code === 0 ? 'done' : 'failed'
      }
      log(
        scriptRun.status === 'stopped'
          ? 'Script stopped.'
          : `Script finished (exit code ${e.payload.code ?? '—'}).`,
      )
      taps.clear()
    }),
  )
  return () => {
    for (const u of unlisteners) u()
    unlisteners = []
    api = null
  }
}

/** Read-only gate for text about to be typed in `sessionId`: true when it may
 *  be sent; otherwise logs what would have been and returns false. */
function readOnlyLets(sessionId: string, text: string, verb: string): boolean {
  if (!scriptRun.readOnly || readOnlyAllows(text, scriptRun.allow)) return true
  log(`[read-only] ${name(sessionId)}: would ${verb} ${JSON.stringify(text)}`)
  return false
}

/** Start `path`. Opens the panel; progress arrives through events. */
export async function startScript(
  path: string,
  dryRun: boolean,
  readOnly = false,
  allow: string[] = DEFAULT_READ_ONLY_ALLOW,
): Promise<void> {
  if (hostBusy()) throw new Error('a script or a notebook connection is already active')
  if (!path) throw new Error('no script chosen')
  taps.clear()
  aborted = false
  cancelled.clear()
  Object.assign(scriptRun, {
    mode: 'script',
    runId: null,
    path,
    dryRun,
    readOnly: readOnly && !dryRun,
    allow: [...allow],
    python: '',
    status: 'running',
    exitCode: null,
    log: [],
    report: [],
    visible: true,
  })
  log(
    `Starting ${path}${
      dryRun
        ? ' (dry run — nothing will be sent)'
        : scriptRun.readOnly
          ? ` (read-only — only ${scriptRun.allow.join(', ')} commands are sent)`
          : ''
    }…`,
  )
  try {
    const started = await invoke<{ run_id: string; python: string }>('script_start', {
      path,
      dryRun,
      readOnly: scriptRun.readOnly,
    })
    scriptRun.runId = started.run_id
    scriptRun.python = started.python
  } catch (e) {
    scriptRun.status = 'failed'
    log(
      String(e instanceof Error ? e.message : e).replace(/^\{"Internal":"(.*)"\}$/, '$1'),
      'stderr',
    )
  }
}

/** Kill the running script and abandon what it was waiting on. */
export async function stopScript(): Promise<void> {
  if (!scriptRun.runId || scriptRun.status !== 'running') return
  aborted = true
  scriptRun.status = 'stopped'
  await invoke('script_stop', { runId: scriptRun.runId }).catch(() => {})
}

/** Turn on the notebook connection and show the panel with how to connect. */
/** Turn the notebook connection on with the saved choices: read-only unless
 *  the user switched it off before (`notebook.read_only`), with their
 *  allowlist (`scripts.read_only_allow`). Used by every "turn on" path. */
export async function startNotebookFromSettings(): Promise<void> {
  const [ro, allowText] = await Promise.all([
    getSetting('notebook.read_only').catch(() => null),
    getSetting('scripts.read_only_allow').catch(() => null),
  ])
  await startNotebook(ro !== '0', parseAllowList(allowText))
}

export async function startNotebook(
  readOnly = true,
  allow: string[] = DEFAULT_READ_ONLY_ALLOW,
): Promise<void> {
  if (hostBusy()) throw new Error('a script or a notebook connection is already active')
  taps.clear()
  aborted = false
  cancelled.clear()
  Object.assign(scriptRun, {
    mode: 'notebook',
    runId: null,
    path: '',
    dryRun: false,
    readOnly,
    allow: [...allow],
    python: '',
    status: 'waiting',
    exitCode: null,
    log: [],
    report: [],
    visible: true,
    connectionFile: '',
    moduleDir: '',
    idleMinutes: 0,
  })
  try {
    const started = await invoke<{
      run_id: string
      connection_file: string
      module_dir: string
      idle_minutes: number
    }>('notebook_enable')
    scriptRun.runId = started.run_id
    scriptRun.connectionFile = started.connection_file
    scriptRun.moduleDir = started.module_dir
    scriptRun.idleMinutes = started.idle_minutes
    log('Notebook connection on — waiting for a notebook…')
  } catch (e) {
    scriptRun.status = 'off'
    log(
      String(e instanceof Error ? e.message : e).replace(/^\{"Internal":"(.*)"\}$/, '$1'),
      'stderr',
    )
  }
}

/** Turn the notebook connection off (disconnects a connected notebook). */
export async function stopNotebook(): Promise<void> {
  if (scriptRun.mode !== 'notebook' || !scriptRun.runId) return
  if (scriptRun.status !== 'waiting' && scriptRun.status !== 'connected') return
  aborted = true
  await invoke('script_stop', { runId: scriptRun.runId }).catch(() => {})
}

// ── request dispatch ─────────────────────────────────────────────────────────

async function handle(
  runId: string,
  reqId: number,
  method: string,
  params: Record<string, unknown>,
) {
  let ok = true
  let result: unknown = null
  let error: unknown = null
  inFlight.add(reqId)
  try {
    checkAborted(reqId)
    result = await dispatch(method, params, reqId)
  } catch (e) {
    ok = false
    error =
      e instanceof ApiError
        ? { kind: e.kind, message: e.message, output: e.output }
        : { kind: 'error', message: errorText(e) }
  }
  inFlight.delete(reqId)
  cancelled.delete(reqId)
  await invoke('script_reply', { runId, reqId, ok, result, error }).catch(() => {})
}

const str = (v: unknown, name: string): string => {
  if (typeof v !== 'string') throw new ApiError('error', `${name} must be a string`)
  return v
}
const secs = (v: unknown, dflt: number): number =>
  typeof v === 'number' && v > 0 ? v * 1000 : dflt * 1000

async function dispatch(
  method: string,
  p: Record<string, unknown>,
  reqId: number,
): Promise<unknown> {
  if (!api) throw new ApiError('error', 'ZapTerm is not ready')
  switch (method) {
    case 'cancel':
      // A stopped notebook cell: abandon the call it was waiting on.
      if (typeof p.id === 'number' && inFlight.has(p.id)) cancelled.add(p.id)
      return null
    case 'tabs': {
      const all = api.openTabs()
      await Promise.all(all.map((t) => api?.refreshPlatform(t.id)))
      return all.map((t) => info(t.id) ?? t)
    }
    case 'tab': {
      const wanted = str(p.name, 'name')
      const all = api.openTabs()
      const t =
        all.find((x) => x.name === wanted) ??
        all.find((x) => x.name.toLowerCase() === wanted.toLowerCase())
      if (!t) {
        const names = all.map((x) => JSON.stringify(x.name)).join(', ') || 'none'
        throw new ApiError('error', `no open tab called ${JSON.stringify(wanted)} (open: ${names})`)
      }
      attach(t.id)
      await api.refreshPlatform(t.id)
      return info(t.id) ?? t
    }
    case 'session': {
      const s = api.focusedSession()
      if (!s) throw new ApiError('error', 'no focused session — click a terminal tab first')
      attach(s.id)
      await api.refreshPlatform(s.id)
      return info(s.id) ?? s
    }
    case 'mode':
      return { dry_run: scriptRun.dryRun, read_only: scriptRun.readOnly }
    case 'sessions':
      return api.savedNames(typeof p.folder === 'string' ? p.folder : null)
    case 'open':
      return openSaved(str(p.name, 'name'), secs(p.timeout, 60), reqId)
    case 'run':
      return run(str(p.session, 'session'), str(p.command, 'command'), secs(p.timeout, 30), reqId)
    case 'send': {
      const sid = str(p.session, 'session')
      const text = str(p.text, 'text')
      const enter = p.enter !== false
      if (scriptRun.dryRun) {
        log(`[dry-run] ${name(sid)}: would send ${JSON.stringify(text)}${enter ? ' + Enter' : ''}`)
        return null
      }
      if (!readOnlyLets(sid, text, 'send')) return null
      await sendText(sid, enter ? text + '\r' : text)
      return null
    }
    case 'expect':
      return expect(
        str(p.session, 'session'),
        str(p.pattern, 'pattern'),
        p.regex === true,
        secs(p.timeout, 30),
        reqId,
      )
    case 'send_secret': {
      const sid = str(p.session, 'session')
      const ref = str(p.ref, 'ref')
      const dot = ref.lastIndexOf('.')
      const field = dot > 0 ? ref.slice(dot + 1).toLowerCase() : ''
      if (field !== 'password' && field !== 'username') {
        throw new ApiError(
          'error',
          `use "Name.Password" or "Name.Username", not ${JSON.stringify(ref)}`,
        )
      }
      if (scriptRun.dryRun) {
        log(`[dry-run] ${name(sid)}: would type the vault ${field} of "${ref.slice(0, dot)}"`)
        return null
      }
      if (scriptRun.readOnly) {
        // Not a display command, and typed blind: keep it out of read-only.
        log(`[read-only] ${name(sid)}: would type the vault ${field} of "${ref.slice(0, dot)}"`)
        return null
      }
      attach(sid)
      await sendVaultField(
        sid,
        ref.slice(0, dot),
        field as 'password' | 'username',
        p.enter !== false,
      )
      return null
    }
    case 'close': {
      const sid = str(p.session, 'session')
      if (scriptRun.dryRun || sid.startsWith('dry:')) {
        log(`[dry-run] would close ${name(sid)}`)
        return null
      }
      api.closeSession(sid)
      taps.delete(sid)
      return null
    }
    case 'report': {
      const row = {
        target: String(p.target ?? ''),
        status: String(p.status ?? ''),
        detail: String(p.detail ?? ''),
      }
      scriptRun.report.push(row)
      log(`▸ ${row.target}: ${row.status}${row.detail ? ` — ${row.detail}` : ''}`)
      return null
    }
    case 'note':
      log(String(p.text ?? ''), 'stdout')
      return null
    case 'confirm':
      if (scriptRun.dryRun) return true
      return api.confirm(str(p.message, 'message'))
    default:
      throw new ApiError('error', `unknown zapterm call ${JSON.stringify(method)}`)
  }
}

// ── session operations ───────────────────────────────────────────────────────

function attach(sessionId: string): Tap {
  let tap = taps.get(sessionId)
  if (!tap) {
    tap = { raw: '', lastData: Date.now(), decoder: new TextDecoder() }
    taps.set(sessionId, tap)
  }
  return tap
}

function info(sessionId: string): SessionInfo | null {
  const base = api?.describe(sessionId) ?? null
  if (!base) return null
  return { ...base, platform: platforms.get(sessionId) ?? base.platform }
}

function name(sessionId: string): string {
  if (sessionId.startsWith('dry:')) return sessionId.slice(4)
  return info(sessionId)?.name ?? sessionId
}

async function sendText(sessionId: string, text: string) {
  const data = Array.from(new TextEncoder().encode(text))
  await invoke('send_input', { sessionId, data })
}

async function openSaved(
  sessionName: string,
  timeoutMs: number,
  reqId: number,
): Promise<SessionInfo> {
  if (!api) throw new ApiError('error', 'ZapTerm is not ready')
  const all = api.savedNames(null)
  if (!all.includes(sessionName)) {
    throw new ApiError('error', `no saved session called ${JSON.stringify(sessionName)}`)
  }
  if (scriptRun.dryRun) {
    log(`[dry-run] would open the saved session "${sessionName}"`)
    return { id: `dry:${sessionName}`, name: sessionName, platform: null }
  }
  log(`Opening "${sessionName}"…`)
  const paneId = api.openSaved(sessionName)
  const deadline = Date.now() + timeoutMs
  // 1. Wait for the tab's backend session and a connected status.
  let sid: string | undefined
  while (Date.now() < deadline) {
    checkAborted(reqId)
    sid = paneToSession.get(paneId)
    // Listen from the start: a fast device can print its banner and prompt
    // before the tab reports connected.
    if (sid) attach(sid)
    const st = api.paneStatus(paneId)
    if (st === 'error' || st === 'lost' || st === 'closed') {
      throw new ApiError('error', `could not connect to "${sessionName}"`)
    }
    if (sid && st === 'connected') break
    await sleep(100)
  }
  if (!sid) throw new ApiError('timeout', `"${sessionName}" did not connect in time`)
  const tap = attach(sid)
  // 2. Let the banner / login script finish: a recognised prompt, or silence.
  while (Date.now() < deadline) {
    checkAborted(reqId)
    const quiet = Date.now() - tap.lastData
    const platform = info(sid)?.platform ?? null
    if (quiet >= PROMPT_QUIET_MS && platform && tap.raw) {
      if (await promptReturned(platform, lastLine(tap.raw).slice(-512)).catch(() => false)) break
    }
    if (quiet >= PROMPT_GUESS_MS && looksLikePrompt(lastLine(tap.raw))) break
    if (quiet >= OPEN_SETTLE_MS) break
    await sleep(100)
  }
  tap.raw = '' // what came before belongs to the login, not to the first run()
  await api.refreshPlatform(sid)
  return info(sid) ?? { id: sid, name: sessionName, platform: null }
}

async function run(
  sessionId: string,
  command: string,
  timeoutMs: number,
  reqId: number,
): Promise<string> {
  if (scriptRun.dryRun || sessionId.startsWith('dry:')) {
    log(`[dry-run] ${name(sessionId)}: would run ${JSON.stringify(command)}`)
    return ''
  }
  if (/[\r\n]/.test(command)) {
    throw new ApiError('error', 'run() takes one command line — call it once per line')
  }
  if (!readOnlyLets(sessionId, command, 'run')) return ''
  const tap = attach(sessionId)
  tap.raw = ''
  tap.lastData = Date.now()
  await sendText(sessionId, command + '\r')
  const deadline = Date.now() + timeoutMs
  let lastPagerAt = -1
  while (Date.now() < deadline) {
    checkAborted(reqId)
    await sleep(50)
    const out = extractRunOutput(tap.raw, command)
    if (out === null) continue // echo not back yet
    const quiet = Date.now() - tap.lastData
    const tail = lastLine(tap.raw)
    // A pager waiting for a key: page on (once per new pager prompt).
    if (MORE_RE.test(tap.raw.slice(-64)) && tap.raw.length !== lastPagerAt) {
      lastPagerAt = tap.raw.length
      await sendText(sessionId, ' ')
      continue
    }
    if (quiet < PROMPT_QUIET_MS) continue
    const platform = info(sessionId)?.platform ?? null
    if (platform && (await promptReturned(platform, tail.slice(-512)).catch(() => false))) {
      tap.raw = ''
      return out
    }
    if (quiet >= PROMPT_GUESS_MS && looksLikePrompt(tail)) {
      tap.raw = ''
      return out
    }
    if (quiet >= SETTLE_MS) {
      tap.raw = ''
      return out
    }
  }
  const partial = extractRunOutput(tap.raw, command) ?? tap.raw
  throw new ApiError('timeout', `no prompt after ${JSON.stringify(command)}`, partial)
}

async function expect(
  sessionId: string,
  pattern: string,
  regex: boolean,
  timeoutMs: number,
  reqId: number,
): Promise<string> {
  if (scriptRun.dryRun || sessionId.startsWith('dry:')) {
    log(`[dry-run] ${name(sessionId)}: would wait for ${JSON.stringify(pattern)}`)
    return ''
  }
  const tap = attach(sessionId)
  let re: RegExp | null = null
  if (regex) {
    try {
      re = new RegExp(pattern)
    } catch (e) {
      throw new ApiError('error', `bad regex: ${e instanceof Error ? e.message : e}`)
    }
  }
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    checkAborted(reqId)
    let end = -1
    if (re) {
      const m = re.exec(tap.raw)
      if (m) end = m.index + m[0].length
    } else {
      const at = tap.raw.indexOf(pattern)
      if (at >= 0) end = at + pattern.length
    }
    if (end >= 0) {
      const got = tap.raw.slice(0, end)
      tap.raw = tap.raw.slice(end) // consume: a later expect won't re-match it
      return got
    }
    await sleep(50)
  }
  throw new ApiError('timeout', `${JSON.stringify(pattern)} did not appear`, tap.raw)
}
