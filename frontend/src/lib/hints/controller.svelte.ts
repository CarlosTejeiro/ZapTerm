import type { Terminal } from '@xterm/xterm'
import type { Hint } from '$lib/bridge/types'
import { getHints, recordCommand } from '$lib/bridge/commands'
import { InputBuffer } from './input-buffer'
import { isPasswordPromptLine } from './passwordPrompt'

// xterm exposes cell dimensions via a private path; we touch it through a
// loose-typed accessor and fall back to a measured estimate if the shape
// changes in a future release.
interface XtermCellDims {
  width: number
  height: number
}

function readCellDims(term: Terminal): XtermCellDims {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const dims = (term as any)?._core?._renderService?.dimensions?.css?.cell
  if (dims && typeof dims.width === 'number' && typeof dims.height === 'number') {
    return { width: dims.width, height: dims.height }
  }
  // Fallback: measure the visible area.
  const el = term.element
  if (el) {
    const rect = el.getBoundingClientRect()
    return {
      width: rect.width / Math.max(term.cols, 1),
      height: rect.height / Math.max(term.rows, 1),
    }
  }
  return { width: 8, height: 16 }
}

export interface CursorPosition {
  /** Pixel offset from the terminal container's top-left. */
  left: number
  top: number
  cellHeight: number
}

export interface HintControllerOpts {
  /** Saved session id, or null for ephemeral / local PTY sessions. */
  savedSessionId: number | null
  /** Called when user accepts a hint — we send the missing suffix to the PTY. */
  sendBytes: (data: Uint8Array) => void
  /** Read live cursor info from xterm. */
  term: Terminal
  /** Enable/disable flags (live from settings). */
  ghostEnabled: () => boolean
  popupEnabled: () => boolean
}

/**
 * Owns the input buffer, debounces backend queries, and exposes reactive
 * state for the ghost-text overlay and the IntelliSense popup.
 */
export class HintController {
  readonly buffer = new InputBuffer()

  // Reactive UI state — Svelte 5 runes.
  ghost = $state<string>('') // suffix shown after the cursor; '' when no hint
  hints = $state<Hint[]>([]) // candidates (best first)
  selected = $state<number>(0) // index into `hints`
  cursor = $state<CursorPosition>({ left: 0, top: 0, cellHeight: 16 })
  popupOpen = $state<boolean>(false)

  private opts: HintControllerOpts
  private debounceTimer: ReturnType<typeof setTimeout> | null = null
  private lastPrefixRequested = ''
  // Last command submitted in this session — context for sequence learning.
  private lastSubmitted: string | null = null
  private cellDims: XtermCellDims = { width: 8, height: 16 }

  constructor(opts: HintControllerOpts) {
    this.opts = opts
    this.cellDims = readCellDims(opts.term)
    this.buffer.subscribe((snap) => {
      this.refreshCursor()
      if (snap.dirty || snap.text.length === 0) {
        this.clear()
        return
      }
      this.schedule(snap.text)
    })
  }

  /** Feed outgoing keystrokes; returns the just-submitted command (or null). */
  onOutgoing(data: Uint8Array): string | null {
    const submitted = this.buffer.feed(data)
    if (submitted !== null && submitted.length > 0) {
      // SECURITY (PRIMARY defense — the backend can't see a bare secret typed
      // at a prompt): if the terminal is sitting at a no-echo password prompt,
      // the typed secret isn't echoed, so the current cursor line is still the
      // prompt. Never record the secret into command history / recents, and
      // never let it become a later transition's `prev` (don't touch
      // lastSubmitted). The ghost/cursor machinery still gets `submitted`.
      // Also test the previous line joined to the current one, in case the
      // prompt wrapped and its terminator fell onto the continuation row.
      const buf = this.opts.term.buffer.active
      const line = buf.getLine(buf.baseY + buf.cursorY)?.translateToString(true) ?? ''
      const prevLine = buf.getLine(buf.baseY + buf.cursorY - 1)?.translateToString(true) ?? ''
      if (!isPasswordPromptLine(line) && !isPasswordPromptLine(prevLine + line)) {
        // Persist asynchronously; backend filters out sensitive lines. Pass the
        // previous command so the backend learns the prev→submitted transition.
        recordCommand(this.opts.savedSessionId, submitted, this.lastSubmitted).catch(console.error)
        this.lastSubmitted = submitted
      }
    }
    return submitted
  }

  /** Called after xterm writes incoming data; recomputes cursor pixel pos. */
  onIncomingFlushed(): void {
    this.refreshCursor()
  }

  /**
   * Accept the current ghost suggestion: send the remaining characters to
   * the PTY as if the user typed them.
   */
  acceptGhost(): boolean {
    if (!this.ghost) return false
    const bytes = new TextEncoder().encode(this.ghost)
    // Mirror them into our buffer so the next snapshot matches.
    this.buffer.feed(bytes)
    this.opts.sendBytes(bytes)
    this.ghost = ''
    this.popupOpen = false
    return true
  }

  /** Accept a specific candidate (used by the popup). */
  acceptIndex(i: number): boolean {
    const hint = this.hints[i]
    if (!hint) return false
    const prefix = this.buffer.snapshot.text
    if (!hint.text.toLowerCase().startsWith(prefix.toLowerCase())) {
      return false
    }
    const suffix = hint.text.slice(prefix.length)
    if (suffix.length === 0) {
      this.popupOpen = false
      return true
    }
    const bytes = new TextEncoder().encode(suffix)
    this.buffer.feed(bytes)
    this.opts.sendBytes(bytes)
    this.ghost = ''
    this.popupOpen = false
    return true
  }

  openPopup(): void {
    if (!this.opts.popupEnabled()) return
    if (this.hints.length === 0) return
    this.popupOpen = true
    this.selected = 0
  }

  closePopup(): void {
    this.popupOpen = false
  }

  movePopup(delta: number): void {
    if (this.hints.length === 0) return
    this.selected = (this.selected + delta + this.hints.length) % this.hints.length
  }

  /** Reset everything (call on session close or terminal redraw). */
  clear(): void {
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer)
      this.debounceTimer = null
    }
    this.ghost = ''
    this.hints = []
    this.selected = 0
    this.popupOpen = false
  }

  private schedule(prefix: string): void {
    if (this.debounceTimer) clearTimeout(this.debounceTimer)
    this.debounceTimer = setTimeout(() => {
      void this.fetch(prefix)
    }, 30)
  }

  private async fetch(prefix: string): Promise<void> {
    if (!this.opts.ghostEnabled() && !this.opts.popupEnabled()) return
    this.lastPrefixRequested = prefix
    try {
      const hints = await getHints(this.opts.savedSessionId, prefix, 8, this.lastSubmitted)
      // Drop stale responses if the user kept typing.
      if (this.lastPrefixRequested !== prefix) return
      // Also: the buffer may have moved on; only commit if still matching.
      const currentSnap = this.buffer.snapshot
      if (currentSnap.dirty || currentSnap.text !== prefix) return

      this.hints = hints
      this.selected = 0
      const best = hints[0]
      if (
        best &&
        this.opts.ghostEnabled() &&
        best.text.length > prefix.length &&
        best.text.toLowerCase().startsWith(prefix.toLowerCase())
      ) {
        this.ghost = best.text.slice(prefix.length)
      } else {
        this.ghost = ''
      }
    } catch (e) {
      console.error('[hints] backend error', e)
    }
  }

  private refreshCursor(): void {
    const term = this.opts.term
    this.cellDims = readCellDims(term)
    const buf = term.buffer.active
    const x = buf.cursorX
    const y = buf.cursorY
    this.cursor = {
      left: x * this.cellDims.width,
      top: y * this.cellDims.height,
      cellHeight: this.cellDims.height,
    }
  }
}
