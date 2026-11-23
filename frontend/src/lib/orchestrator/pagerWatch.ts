// "Page through --More--" for a line sent with Enter from the SEND TO ALL bar:
// after the line goes out, each session that stops at a pager prompt
// (`--More--`, `---(more)---`, `<--- More --->`) is sent a space so the device
// prints the next page. A session stops being watched once it has sent
// nothing for IDLE_MS (its output is over, or it never answered), or after
// MAX_PAGES pages. Run & compare does its own paging (CommandRunner).

import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onTerminalData } from '$lib/bridge/events'
import { stripAnsi } from '$lib/scripting/output'
import { endsAtPager, MAX_PAGES } from './compare'

/** A pager prompt is answered once the session has been quiet this long, so
 *  it is really waiting for a key. */
const QUIET_MS = 150
/** A session that sends nothing for this long is no longer watched. */
export const IDLE_MS = 10_000
/** How much of each session's latest output is kept to spot the prompt. */
const TAIL = 2048

interface Watched {
  tail: string
  decoder: TextDecoder
  pages: number
  quiet: ReturnType<typeof setTimeout> | null
  idle: ReturnType<typeof setTimeout> | null
}

export class PagerWatch {
  #sessions = new Map<string, Watched>()
  #unlisten: UnlistenFn | null = null
  /** Bumped by every start/stop, so a start that is still subscribing when
   *  it is replaced doesn't keep a listener. */
  #generation = 0

  /** Pages the sessions currently watched have been paged through. */
  pages(sessionId: string): number {
    return this.#sessions.get(sessionId)?.pages ?? 0
  }

  get watching(): boolean {
    return this.#sessions.size > 0
  }

  /** Watch `sessionIds` for pager prompts. Call right before sending the line,
   *  so its first output isn't missed. Replaces any earlier watch. */
  async start(sessionIds: string[]): Promise<void> {
    this.stop()
    const generation = this.#generation
    for (const id of sessionIds) {
      this.#sessions.set(id, {
        tail: '',
        decoder: new TextDecoder(),
        pages: 0,
        quiet: null,
        idle: null,
      })
      this.#armIdle(id)
    }
    const unlisten = await onTerminalData((p) => this.#onData(p.session_id, p.data))
    if (generation !== this.#generation || this.#sessions.size === 0) unlisten()
    else this.#unlisten = unlisten
  }

  /** Stop watching every session. */
  stop(): void {
    this.#generation++
    for (const w of this.#sessions.values()) {
      if (w.quiet) clearTimeout(w.quiet)
      if (w.idle) clearTimeout(w.idle)
    }
    this.#sessions.clear()
    if (this.#unlisten) {
      this.#unlisten()
      this.#unlisten = null
    }
  }

  #onData(sessionId: string, data: number[]) {
    const w = this.#sessions.get(sessionId)
    if (!w) return
    w.tail = (w.tail + stripAnsi(w.decoder.decode(new Uint8Array(data), { stream: true }))).slice(
      -TAIL,
    )
    if (w.quiet) clearTimeout(w.quiet)
    w.quiet = setTimeout(() => this.#check(sessionId), QUIET_MS)
    this.#armIdle(sessionId)
  }

  /** Quiet at a pager prompt: press Space. The next check comes with the
   *  device's next output, so each prompt gets one space. */
  #check(sessionId: string) {
    const w = this.#sessions.get(sessionId)
    if (!w) return
    w.quiet = null
    if (!endsAtPager(w.tail)) return
    if (w.pages >= MAX_PAGES) {
      this.#drop(sessionId)
      return
    }
    w.pages++
    invoke('send_input', { sessionId, data: [0x20] }).catch(console.error)
  }

  #armIdle(sessionId: string) {
    const w = this.#sessions.get(sessionId)
    if (!w) return
    if (w.idle) clearTimeout(w.idle)
    w.idle = setTimeout(() => this.#drop(sessionId), IDLE_MS)
  }

  #drop(sessionId: string) {
    const w = this.#sessions.get(sessionId)
    if (!w) return
    if (w.quiet) clearTimeout(w.quiet)
    if (w.idle) clearTimeout(w.idle)
    this.#sessions.delete(sessionId)
    if (this.#sessions.size === 0 && this.#unlisten) {
      this.#unlisten()
      this.#unlisten = null
    }
  }
}
