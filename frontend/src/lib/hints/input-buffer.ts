// Tracks the user's "current input line" locally, by inspecting the bytes
// we send to the PTY. We never parse remote prompts — those are too varied
// across shells (bash, zsh, Cisco IOS, Junos, raw serial). Instead the buffer
// is built from outgoing keystrokes and reset on Enter / Ctrl-C / cursor
// drift.
//
// `dirty` means we lost sync with the terminal (Tab completion, arrow-key
// history navigation, ESC sequence we don't understand). When dirty the
// caller should hide hints until the next Enter clears the line.

export interface BufferSnapshot {
  text: string
  dirty: boolean
}

export class InputBuffer {
  private chars: string[] = []
  private _dirty = false
  /** Bumped whenever the buffer or dirty state changes. */
  private _version = 0
  private listeners = new Set<(snap: BufferSnapshot) => void>()

  get version(): number {
    return this._version
  }

  get snapshot(): BufferSnapshot {
    return { text: this.chars.join(''), dirty: this._dirty }
  }

  subscribe(fn: (snap: BufferSnapshot) => void): () => void {
    this.listeners.add(fn)
    fn(this.snapshot)
    return () => this.listeners.delete(fn)
  }

  /**
   * Feed bytes that the user typed (before they reach the PTY). Returns the
   * command that was just submitted via Enter, or null otherwise — callers
   * use this to push history.
   */
  feed(bytes: Uint8Array): string | null {
    let submitted: string | null = null
    let i = 0
    while (i < bytes.length) {
      const b = bytes[i]

      // Enter — flush. Only report a line we tracked cleanly: a dirty line
      // (Tab completion, history recall, an ESC sequence we couldn't fully
      // parse) may be incomplete or polluted with stray bytes, so don't hand
      // it back to be recorded into history.
      if (b === 0x0d || b === 0x0a) {
        submitted = this._dirty ? null : this.chars.join('').trim()
        this.reset()
        i += 1
        continue
      }

      // Backspace / DEL.
      if (b === 0x08 || b === 0x7f) {
        if (!this._dirty && this.chars.length > 0) {
          this.chars.pop()
          this.bump()
        }
        i += 1
        continue
      }

      // Ctrl-C / Ctrl-U — discard whole line.
      if (b === 0x03 || b === 0x15) {
        this.reset()
        i += 1
        continue
      }

      // Ctrl-W — drop last word.
      if (b === 0x17) {
        if (!this._dirty) {
          this.dropLastWord()
        }
        i += 1
        continue
      }

      // Tab — remote autocomplete will redraw the line; we can't keep up.
      if (b === 0x09) {
        this.markDirty()
        i += 1
        continue
      }

      // ESC sequence — arrow keys, function keys, etc. Mark dirty and skip
      // past the whole sequence. Two forms matter:
      //   • CSI: ESC [ <params> <final 0x40-0x7e>  (normal cursor mode)
      //   • SS3: ESC O <final>                      (application cursor mode,
      //     which zsh commonly enables — arrows are ESC O A/B/C/D, Home/End
      //     ESC O H/F). If we don't consume the 'O' and its final byte they
      //     leak in as printable characters and pollute the recorded command
      //     (e.g. an up-arrow history recall stored as "OA").
      if (b === 0x1b) {
        this.markDirty()
        i += 1
        const next = bytes[i]
        if (next === 0x5b /* [ */) {
          i += 1
          while (i < bytes.length) {
            const c = bytes[i] ?? 0
            i += 1
            if (c >= 0x40 && c <= 0x7e) break
          }
        } else if (next === 0x4f /* O (SS3) */) {
          i += 2 // skip the 'O' and the single final byte
        }
        // A lone ESC (or a form we don't model) just falls through — already
        // marked dirty, and `i` is past the ESC byte.
        continue
      }

      // Other C0 control chars — be conservative and mark dirty.
      if (b !== undefined && b < 0x20) {
        this.markDirty()
        i += 1
        continue
      }

      // Printable byte (or UTF-8 continuation). We decode UTF-8 codepoints
      // one at a time so emoji / non-ASCII width is preserved.
      const seqLen = utf8SeqLen(b ?? 0)
      const end = Math.min(i + seqLen, bytes.length)
      const piece = bytesToString(bytes.subarray(i, end))
      this.chars.push(piece)
      this.bump()
      i = end
    }
    return submitted
  }

  /**
   * Called when the PTY echoes back bytes. We use the xterm cursor X
   * position (passed in from the caller) to detect when the on-screen line
   * diverges from our buffer — e.g. after a 'clear', after pasted multi-line
   * input, or after remote-side line editing we couldn't follow.
   *
   * `linePrefixLen` is the number of cells between the start of the prompt
   * and the cursor on the active terminal row. The caller computes this as
   * (cursorX − the prompt's column), but since we don't know the prompt
   * column we use a softer rule: if cursorX < buffer length by more than a
   * single char, we are out of sync.
   */
  syncFromCursor(cursorX: number, expectedAtLeast: number): void {
    if (this._dirty) return
    if (cursorX + 1 < expectedAtLeast) {
      this.markDirty()
    }
  }

  reset(): void {
    if (this.chars.length === 0 && !this._dirty) return
    this.chars = []
    this._dirty = false
    this.bump()
  }

  markDirty(): void {
    if (this._dirty) return
    this._dirty = true
    this.bump()
  }

  private dropLastWord(): void {
    // Strip trailing whitespace, then chars until whitespace.
    while (this.chars.length > 0 && /\s/.test(this.chars[this.chars.length - 1] ?? '')) {
      this.chars.pop()
    }
    while (this.chars.length > 0 && !/\s/.test(this.chars[this.chars.length - 1] ?? '')) {
      this.chars.pop()
    }
    this.bump()
  }

  private bump(): void {
    this._version += 1
    const snap = this.snapshot
    for (const fn of this.listeners) fn(snap)
  }
}

function utf8SeqLen(firstByte: number): number {
  if (firstByte < 0x80) return 1
  if ((firstByte & 0xe0) === 0xc0) return 2
  if ((firstByte & 0xf0) === 0xe0) return 3
  if ((firstByte & 0xf8) === 0xf0) return 4
  return 1
}

function bytesToString(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes)
}
