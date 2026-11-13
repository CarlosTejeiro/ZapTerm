import { describe, it, expect } from 'vitest'
import { InputBuffer } from './input-buffer'

function feed(buf: InputBuffer, ascii: string): string | null {
  return buf.feed(new TextEncoder().encode(ascii))
}

describe('InputBuffer', () => {
  it('accumulates printable chars', () => {
    const b = new InputBuffer()
    feed(b, 'git ')
    feed(b, 'status')
    expect(b.snapshot.text).toBe('git status')
    expect(b.snapshot.dirty).toBe(false)
  })

  it('flushes on \\r and returns the submitted command', () => {
    const b = new InputBuffer()
    feed(b, 'ls -la')
    const submitted = b.feed(new Uint8Array([0x0d]))
    expect(submitted).toBe('ls -la')
    expect(b.snapshot.text).toBe('')
  })

  it('pops on backspace', () => {
    const b = new InputBuffer()
    feed(b, 'foo')
    b.feed(new Uint8Array([0x7f]))
    expect(b.snapshot.text).toBe('fo')
  })

  it('drops last word on Ctrl-W', () => {
    const b = new InputBuffer()
    feed(b, 'git commit -m')
    b.feed(new Uint8Array([0x17]))
    expect(b.snapshot.text).toBe('git commit ')
  })

  it('resets on Ctrl-C', () => {
    const b = new InputBuffer()
    feed(b, 'rm -rf /')
    b.feed(new Uint8Array([0x03]))
    expect(b.snapshot.text).toBe('')
    expect(b.snapshot.dirty).toBe(false)
  })

  it('marks dirty on Tab', () => {
    const b = new InputBuffer()
    feed(b, 'cat /etc/h')
    b.feed(new Uint8Array([0x09]))
    expect(b.snapshot.dirty).toBe(true)
  })

  it('clears dirty on Enter', () => {
    const b = new InputBuffer()
    feed(b, 'abc')
    b.feed(new Uint8Array([0x09]))
    expect(b.snapshot.dirty).toBe(true)
    b.feed(new Uint8Array([0x0d]))
    expect(b.snapshot.dirty).toBe(false)
    expect(b.snapshot.text).toBe('')
  })

  it('marks dirty on arrow keys (CSI A)', () => {
    const b = new InputBuffer()
    feed(b, 'echo')
    // Up arrow: ESC [ A
    b.feed(new Uint8Array([0x1b, 0x5b, 0x41]))
    expect(b.snapshot.dirty).toBe(true)
  })

  it('consumes SS3 arrow keys (ESC O A) instead of leaking "OA"', () => {
    const b = new InputBuffer()
    // Application-cursor-mode up arrow: ESC O A — must NOT add "OA".
    b.feed(new Uint8Array([0x1b, 0x4f, 0x41]))
    expect(b.snapshot.text).toBe('')
    expect(b.snapshot.dirty).toBe(true)
    // Typing after the recall still accumulates, but the line is dirty.
    feed(b, ' | grep x')
    expect(b.snapshot.text).toBe(' | grep x')
  })

  it('does not record a dirty line on Enter (history recall + edit)', () => {
    const b = new InputBuffer()
    // Recall a command with up arrow (SS3), then append a pipe.
    b.feed(new Uint8Array([0x1b, 0x4f, 0x41]))
    feed(b, ' | grep smart')
    // The buffer never saw the recalled text, so the line is untrackable —
    // Enter must return null rather than a polluted/partial command.
    const submitted = b.feed(new Uint8Array([0x0d]))
    expect(submitted).toBeNull()
    expect(b.snapshot.dirty).toBe(false)
  })

  it('handles UTF-8 multi-byte chars', () => {
    const b = new InputBuffer()
    feed(b, 'echo héllo')
    expect(b.snapshot.text).toBe('echo héllo')
  })

  it('publishes snapshots to subscribers', () => {
    const b = new InputBuffer()
    const snaps: string[] = []
    const unsub = b.subscribe((s) => snaps.push(s.text))
    feed(b, 'ab')
    feed(b, 'c')
    unsub()
    feed(b, 'd')
    expect(snaps).toEqual(['', 'a', 'ab', 'abc'])
  })

  it('ignores edits while dirty (until next Enter)', () => {
    const b = new InputBuffer()
    feed(b, 'foo')
    b.feed(new Uint8Array([0x09])) // Tab -> dirty
    feed(b, 'bar')
    // chars are stored anyway when not control, but dirty flag stays true
    // until Enter resets the buffer entirely.
    expect(b.snapshot.dirty).toBe(true)
    b.feed(new Uint8Array([0x0d]))
    expect(b.snapshot.dirty).toBe(false)
  })
})
