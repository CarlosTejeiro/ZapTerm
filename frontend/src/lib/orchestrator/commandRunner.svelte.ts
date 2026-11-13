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

export type HostState = 'running' | 'done' | 'timeout'

export interface HostCapture {
  sessionId: string
  label: string
  platform: string | null
  state: HostState
  /** ANSI-stripped accumulated output. */
  buffer: string
}

export interface RunnerHost {
  sessionId: string
  label: string
  platform: string | null
}

const QUIET_MS = 150
// Fallback completion: once a host has echoed the command and then gone quiet
// for this long with no recognised prompt, treat it as done anyway. This is
// what makes capture work for unknown platforms and for known platforms whose
// prompt we fail to match — without it those hosts only ever hit the global
// timeout and their (captured) output was being discarded.
const SETTLE_MS = 900
const DEFAULT_TIMEOUT_MS = 10_000

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
  #timeoutMs: number

  constructor(timeoutMs: number = DEFAULT_TIMEOUT_MS) {
    this.#timeoutMs = timeoutMs
  }

  /** Send `command` to every host and begin capturing. */
  async run(hosts: RunnerHost[], command: string): Promise<void> {
    this.reset()
    if (hosts.length === 0) return
    this.command = command
    this.running = true
    this.hosts = hosts.map((h) => ({
      sessionId: h.sessionId,
      label: h.label,
      platform: h.platform,
      state: 'running' as HostState,
      buffer: '',
    }))
    for (const h of hosts) this.#decoders.set(h.sessionId, new TextDecoder())

    // Subscribe BEFORE sending so we don't miss the first bytes.
    this.#unlisten = await onTerminalData((p) => this.#onData(p.session_id, p.data))

    const bytes = Array.from(new TextEncoder().encode(command + '\r'))
    for (const h of hosts) {
      invoke('send_input', { sessionId: h.sessionId, data: bytes }).catch(console.error)
    }

    this.#globalTimer = setTimeout(() => this.#expire(), this.#timeoutMs)
  }

  #onData(sessionId: string, data: number[]) {
    const host = this.hosts.find((h) => h.sessionId === sessionId)
    if (!host || host.state !== 'running') return
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
    if (this.command && !host.buffer.includes(this.command)) return
    host.state = 'done'
    this.#settleCheck()
  }

  async #maybeComplete(sessionId: string) {
    const host = this.hosts.find((h) => h.sessionId === sessionId)
    if (!host || host.state !== 'running') return
    // Wait until the command echo has shown up before trusting a prompt match;
    // otherwise the echoed prompt right after sending can look "done".
    if (this.command && !host.buffer.includes(this.command)) return
    if (!host.platform) return // unknown platform → time-window only

    // Check only the tail to keep the regex cheap and end-anchored.
    const tail = host.buffer.slice(-512)
    try {
      if (await promptReturned(host.platform, tail)) {
        host.state = 'done'
        this.#settleCheck()
      }
    } catch {
      /* leave running; the global timeout will catch it */
    }
  }

  #expire() {
    for (const h of this.hosts) {
      if (h.state === 'running') h.state = 'timeout'
    }
    this.#settleCheck()
  }

  #settleCheck() {
    if (this.hosts.every((h) => h.state !== 'running')) {
      this.running = false
      this.done = true
      this.#teardownListeners()
    }
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
