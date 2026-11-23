// PagerWatch with the Tauri bridge mocked: which sessions get a space, and
// when it stops watching.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { TerminalDataPayload } from '$lib/bridge/events'

let feed: ((p: TerminalDataPayload) => void) | null = null
const sent: { sessionId: string; text: string }[] = []

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string, args: { sessionId: string; data: number[] }) => {
    if (cmd === 'send_input') {
      sent.push({
        sessionId: args.sessionId,
        text: new TextDecoder().decode(new Uint8Array(args.data)),
      })
    }
  },
}))
vi.mock('$lib/bridge/events', () => ({
  onTerminalData: (handler: (p: TerminalDataPayload) => void) => {
    feed = handler
    return Promise.resolve(() => {
      feed = null
    })
  },
}))

const { PagerWatch, IDLE_MS } = await import('./pagerWatch')
const { MAX_PAGES } = await import('./compare')

const out = (sessionId: string, text: string) =>
  feed?.({ session_id: sessionId, data: Array.from(new TextEncoder().encode(text)) })

beforeEach(() => {
  vi.useFakeTimers()
  feed = null
  sent.length = 0
})
afterEach(() => {
  vi.useRealTimers()
})

describe('PagerWatch', () => {
  it('presses Space only in the session waiting at a pager prompt', async () => {
    const w = new PagerWatch()
    await w.start(['s1', 's2'])
    out('s1', 'get system status\r\nVersion: 7.6\r\n--More-- ')
    out('s2', 'get system status\r\nVersion: 7.6\r\nfw2 # ')
    await vi.advanceTimersByTimeAsync(200)
    expect(sent).toEqual([{ sessionId: 's1', text: ' ' }])
    // The device erases the prompt and prints the rest: no second space.
    out('s1', '\r         \rSerial: X\r\nfw1 # ')
    await vi.advanceTimersByTimeAsync(1000)
    expect(sent).toHaveLength(1)
    w.stop()
  })

  it('answers every new page, up to MAX_PAGES', async () => {
    const w = new PagerWatch()
    await w.start(['s1'])
    for (let i = 0; i < MAX_PAGES + 5; i++) {
      out('s1', `\rline ${i}\r\n--More-- `)
      await vi.advanceTimersByTimeAsync(200)
    }
    expect(sent).toHaveLength(MAX_PAGES)
    expect(w.watching).toBe(false)
  })

  it('stops watching a session that goes idle', async () => {
    const w = new PagerWatch()
    await w.start(['s1'])
    out('s1', 'show ver\r\nok\r\nsw# ')
    await vi.advanceTimersByTimeAsync(IDLE_MS + 100)
    expect(w.watching).toBe(false)
    expect(feed).toBe(null) // listener released
    out('s1', 'later\r\n--More-- ')
    await vi.advanceTimersByTimeAsync(200)
    expect(sent).toEqual([])
  })

  it('does nothing after stop', async () => {
    const w = new PagerWatch()
    await w.start(['s1'])
    w.stop()
    out('s1', 'x\r\n--More-- ')
    await vi.advanceTimersByTimeAsync(200)
    expect(sent).toEqual([])
  })
})
