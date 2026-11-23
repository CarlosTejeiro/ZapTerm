// CommandRunner with the Tauri bridge mocked: what it sends, and when it
// considers each host finished.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import type { TerminalDataPayload } from '$lib/bridge/events'

// The runner is a `.svelte.ts` module; these tests run without the Svelte
// compiler, so `$state` is a plain identity function here.
;(globalThis as unknown as { $state: <T>(v: T) => T }).$state = (v) => v

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
vi.mock('$lib/bridge/commands', () => ({
  // Unknown platform in these tests: completion comes from going quiet.
  promptReturned: async () => false,
}))

const { CommandRunner } = await import('./commandRunner.svelte')

const out = (sessionId: string, text: string) =>
  feed?.({ session_id: sessionId, data: Array.from(new TextEncoder().encode(text)) })

const host = (n: number) => ({
  sessionId: `s${n}`,
  label: `fw${n}`,
  platform: null,
  position: n,
  target: `10.0.0.${n}:22`,
})

beforeEach(() => {
  vi.useFakeTimers()
  feed = null
  sent.length = 0
})
afterEach(() => {
  vi.useRealTimers()
})

describe('CommandRunner', () => {
  it('sends the command with Enter to every host', async () => {
    const r = new CommandRunner()
    await r.run([host(1), host(2)], 'show ver')
    expect(sent).toEqual([
      { sessionId: 's1', text: 'show ver\r' },
      { sessionId: 's2', text: 'show ver\r' },
    ])
    r.reset()
  })

  it('in typed mode only presses Enter and needs no echo', async () => {
    const r = new CommandRunner()
    await r.run([host(1)], 'show ver', { typed: true })
    expect(sent).toEqual([{ sessionId: 's1', text: '\r' }])
    out('s1', '\r\nVersion 1\r\nfw1# ')
    await vi.advanceTimersByTimeAsync(1000)
    expect(r.hosts[0]!.state).toBe('done')
    r.reset()
  })

  it('keeps capturing when a quiet host carries on', async () => {
    const r = new CommandRunner()
    await r.run([host(1)], 'show run')
    out('s1', 'show run\r\npart 1\r\n')
    await vi.advanceTimersByTimeAsync(1000) // quiet → presumed done
    expect(r.hosts[0]!.state).toBe('done')
    expect(r.running).toBe(true) // grace period: still listening
    out('s1', 'part 2\r\nfw1# ')
    expect(r.hosts[0]!.state).toBe('running')
    await vi.advanceTimersByTimeAsync(1000 + 1600)
    expect(r.running).toBe(false)
    expect(r.hosts[0]!.buffer).toContain('part 2')
    r.reset()
  })

  it('times out a silent host and closes the run at the deadline', async () => {
    const r = new CommandRunner(3000)
    await r.run([host(1), host(2)], 'show ver')
    out('s1', 'show ver\r\nok\r\nfw1# ')
    await vi.advanceTimersByTimeAsync(3000)
    expect(r.hosts.map((h) => h.state)).toEqual(['done', 'timeout'])
    expect(r.running).toBe(false)
    r.reset()
  })
})
