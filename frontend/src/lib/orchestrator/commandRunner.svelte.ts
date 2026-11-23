// Multi-host command runner (Phase B of orchestration).
//
// Sends ONE command to every host in a grid and captures each host's output
// between the send and the moment its prompt returns, so the results can be
// grouped/diffed ("are all my switches configured the same?").
//
// Capture lives entirely in the frontend: we already receive every
// `terminal-data` event, so we just buffer per host. Completion is detected
// two ways, in priority order:
//   1. Prompt return — `prompt_returned(platform, tail)` (reuses the Rust
//      prompt regexes). Only attempted once the command echo has been seen
//      and after a short quiet period, so we don't match a prompt-shaped line
//      mid-stream.
//   2. Time window — a global timeout flips any still-running host to
//      `timeout`. This is also the ONLY mechanism for hosts whose platform is
//      unknown (prompt_returned always returns false there).

import { invoke } from '@tauri-apps/api/core'
import { onTerminalData } from '$lib/bridge/events'
import { promptReturned } from '$lib/bridge/commands'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { CompareHost, HostState } from './compare'

export type { HostState }

export interface HostCapture extends CompareHost {
  platform: string | null
  /** How a finished host was judged finished: its prompt came back, or it
   *  went quiet (`settle`). A settled host that sends more output while the
   *  run is still open goes back to `running`, so a pause mid-output doesn't
   *  cut the capture short. */
  doneBy: 'prompt' | 'settle' | null
}

export interface RunnerHost {
  sessionId: string
  label: string
  platform: string | null
  /** 1-based position of the pane in its tab. */
  position: number
  /** `host:port`, or null for a local shell. */
  target: string | null
}

export interface RunOptions {
  /** The command is already on the devices' command line (the master bar's
   *  "live" mode streamed it while it was typed): only press Enter, and don't
   *  wait for an echo that arrived before the run started. */
  typed?: boolean
}

const QUIET_MS = 150
// Fallback completion: once a host has echoed the command and then gone quiet
// for this long with no recognised prompt, treat it as done anyway. This is
// what makes capture work for unknown platforms and for known platforms whose
// prompt we fail to match — without it those hosts only ever hit the global
// timeout and their (captured) output was being discarded.
const SETTLE_MS = 900
const DEFAULT_TIMEOUT_MS = 10_000
// When every host is finished but some only because they went quiet, keep
// listening this much longer before closing the run: a device that pauses
// mid-output then still gets its full output compared.
const GRACE_MS = 1_500

function stripAnsi(s: string): string {
  return (
    s
      // OSC … BEL / ST
      .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')
      // CSI sequences (colors, cursor moves, etc.)
      .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '')
      // lone two-char escapes
      .replace(/\x1b[@-Z\\-_]/g, '')
  )
}

export class CommandRunner {
  /** Per-host capture, keyed by live session UUID. Reactive for the panel. */
  hosts = $state<HostCapture[]>([])
  command = $state('')
  running = $state(false)
  /** True once every host has settled (done or timeout). */
  done = $state(false)

  #unlisten: UnlistenFn | null = null
  #decoders = new Map<string, TextDecoder>()
  #quietTimers = new Map<string, ReturnType<typeof setTimeout>>()
  #settleTimers = new Map<string, ReturnType<typeof setTimeout>>()
  #globalTimer: ReturnType<typeof setTimeout> | null = null
  #graceTimer: ReturnType<typeof setTimeout> | null = null
  #timeoutMs: number
  /** Wait for the command's echo before trusting completion. */
  #needEcho = true

  constructor(timeoutMs: number = DEFAULT_TIMEOUT_MS) {
    this.#timeoutMs = timeoutMs
  }

  /** How long a host may take before it counts as "no reply". */
  get timeoutMs(): number {
    return this.#timeoutMs
  }

  /** Send `command` to every host and begin capturing. */
  async run(hosts: RunnerHost[], command: string, opts: RunOptions = {}): Promise<void> {
    this.reset()
    if (hosts.length === 0) return
    this.command = command
    this.running = true
    this.#needEcho = !opts.typed
    this.hosts = hosts.map((h) => ({
      sessionId: h.sessionId,
      label: h.label,
      platform: h.platform,
      position: h.position,
      target: h.target,
      state: 'running' as HostState,
      doneBy: null,
      buffer: '',
    }))
    for (const h of hosts) this.#decoders.set(h.sessionId, new TextDecoder())

    // Subscribe BEFORE sending so we don't miss the first bytes.
    this.#unlisten = await onTerminalData((p) => this.#onData(p.session_id, p.data))

    const bytes = Array.from(new TextEncoder().encode(opts.typed ? '\r' : command + '\r'))
    for (const h of hosts) {
      invoke('send_input', { sessionId: h.sessionId, data: bytes }).catch(console.error)
    }

    this.#globalTimer = setTimeout(() => this.#expire(), this.#timeoutMs)
  }

  #onData(sessionId: string, data: number[]) {
    const host = this.hosts.find((h) => h.sessionId === sessionId)
    if (!host) return
    // More output from a host that was only presumed finished (it had gone
    // quiet): it wasn't, keep capturing.
    if (host.state === 'done' && host.doneBy === 'settle') {
      host.state = 'running'
      host.doneBy = null
      if (this.#graceTimer) {
        clearTimeout(this.#graceTimer)
        this.#graceTimer = null
      }
    }
    if (host.state !== 'running') return
    const decoder = this.#decoders.get(sessionId)
    if (!decoder) return
    host.buffer += stripAnsi(decoder.decode(new Uint8Array(data), { stream: true }))

    // Reset the per-host quiet timer; evaluate prompt completion when it fires.
    const existing = this.#quietTimers.get(sessionId)
    if (existing) clearTimeout(existing)
    this.#quietTimers.set(
      sessionId,
      setTimeout(() => this.#maybeComplete(sessionId), QUIET_MS),
    )

    // Reset the longer settle timer — fallback completion when no prompt is
    // recognised (unknown platform, or a prompt we don't match).
    const settle = this.#settleTimers.get(sessionId)
    if (settle) clearTimeout(settle)
    this.#settleTimers.set(
      sessionId,
      setTimeout(() => this.#settleComplete(sessionId), SETTLE_MS),
    )
  }

  #settleComplete(sessionId: string) {
    const host = this.hosts.find((h) => h.sessionId === sessionId)
    if (!host || host.state !== 'running') return
    // Only settle once the command echo has come back, so we don't complete on
    // a pre-command burst. A host that never echoes (truly silent) keeps
    // running and is caught by the global timeout = a genuine no-reply.
    if (!this.#echoSeen(host)) return
    host.state = 'done'
    host.doneBy = 'settle'
    this.#settleCheck()
  }

  #echoSeen(host: HostCapture): boolean {
    return !this.#needEcho || !this.command || host.buffer.includes(this.command)
  }

  async #maybeComplete(sessionId: string) {
    const host = this.hosts.find((h) => h.sessionId === sessionId)
    if (!host || host.state !== 'running') return
    // Wait until the command echo has shown up before trusting a prompt match;
    // otherwise the echoed prompt right after sending can look "done".
    if (!this.#echoSeen(host)) return
    if (!host.platform) return // unknown platform → time-window only

    // Check only the tail to keep the regex cheap and end-anchored.
    const tail = host.buffer.slice(-512)
    try {
      if (await promptReturned(host.platform, tail)) {
        if (host.state !== 'running') return
        host.state = 'done'
        host.doneBy = 'prompt'
        this.#settleCheck()
      }
    } catch {
      /* leave running; the global timeout will catch it */
    }
  }

  #expire() {
    // The deadline has passed: close the run now, no grace period.
    this.#globalTimer = null
    for (const h of this.hosts) {
      if (h.state === 'running') h.state = 'timeout'
    }
    this.#settleCheck()
  }

  #settleCheck() {
    if (!this.hosts.every((h) => h.state !== 'running')) return
    if (this.hosts.some((h) => h.doneBy === 'settle') && this.#globalTimer) {
      if (this.#graceTimer) clearTimeout(this.#graceTimer)
      this.#graceTimer = setTimeout(() => {
        this.#graceTimer = null
        if (this.hosts.every((h) => h.state !== 'running')) this.#finish()
      }, GRACE_MS)
      return
    }
    this.#finish()
  }

  #finish() {
    this.running = false
    this.done = true
    this.#teardownListeners()
  }

  #teardownListeners() {
    if (this.#unlisten) {
      this.#unlisten()
      this.#unlisten = null
    }
    for (const t of this.#quietTimers.values()) clearTimeout(t)
    this.#quietTimers.clear()
    for (const t of this.#settleTimers.values()) clearTimeout(t)
    this.#settleTimers.clear()
    if (this.#globalTimer) {
      clearTimeout(this.#globalTimer)
      this.#globalTimer = null
    }
    if (this.#graceTimer) {
      clearTimeout(this.#graceTimer)
      this.#graceTimer = null
    }
  }

  /** Tear everything down — call on unmount or before a new run. */
  reset() {
    this.#teardownListeners()
    this.#decoders.clear()
    this.hosts = []
    this.command = ''
    this.running = false
    this.done = false
  }
}
