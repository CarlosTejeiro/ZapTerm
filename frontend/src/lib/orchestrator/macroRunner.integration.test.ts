// Integration test for runMacro: simulate the terminal-data stream + mock the
// backend (invoke / vault sends) and assert expect matching + vault send.
import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { LoginStep } from '$lib/bridge/types'
import type { TerminalDataPayload } from '$lib/bridge/events'

// ── Mocks (must be declared before importing runMacro) ──────────────────────
let feed: ((p: TerminalDataPayload) => void) | null = null
const invokeSpy = vi.fn(async (..._a: unknown[]) => undefined)
const sendVaultFieldSpy = vi.fn(async (..._a: unknown[]) => undefined)
const sendVaultSecretSpy = vi.fn(async (..._a: unknown[]) => undefined)

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeSpy(...args),
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
  sendVaultField: (...args: unknown[]) => sendVaultFieldSpy(...args),
  sendVaultSecret: (...args: unknown[]) => sendVaultSecretSpy(...args),
}))
vi.mock('$lib/stores/sessionRuntime.svelte', () => ({
  getFocusedSessionId: () => 'sess1',
  focusSession: () => {},
}))
vi.mock('$lib/ui/toast-store.svelte', () => ({ showToast: () => {} }))

const { runMacro } = await import('./macroRunner')

const bytes = (s: string) => Array.from(new TextEncoder().encode(s))
const tick = (ms = 15) => new Promise((r) => setTimeout(r, ms))

beforeEach(() => {
  feed = null
  invokeSpy.mockClear()
  sendVaultFieldSpy.mockClear()
  sendVaultSecretSpy.mockClear()
})

describe('runMacro (integration)', () => {
  it('literal expect matches streamed output, then the vault send fires', async () => {
    const steps: LoginStep[] = [
      { kind: 'send', expect: '', is_regex: false, send: 'ssh user@host\\r', timeout_ms: 10000 },
      { kind: 'expect', expect: 'password:', is_regex: false, send: '', timeout_ms: 3000 },
      {
        kind: 'send',
        expect: '',
        is_regex: false,
        send: '{{vault:admin.Password}}\\r',
        timeout_ms: 10000,
      },
    ]
    const p = runMacro('sess1', steps)
    await tick() // let subscription happen + the first send run
    feed?.({ session_id: 'sess1', data: bytes('\r\nuser@host password: ') })
    const res = await p

    expect(res.ok).toBe(true)
    // ssh command sent via send_input:
    expect(invokeSpy).toHaveBeenCalledWith('send_input', expect.anything())
    // vault password sent to the right session/name/field, with Enter:
    expect(sendVaultFieldSpy).toHaveBeenCalledWith('sess1', 'admin', 'password', true)
  })

  it('regex expect matches', async () => {
    const steps: LoginStep[] = [
      { kind: 'expect', expect: 'password:\\s*$', is_regex: true, send: '', timeout_ms: 3000 },
    ]
    const p = runMacro('sess1', steps)
    await tick()
    feed?.({ session_id: 'sess1', data: bytes('login as t760784\r\npassword:   ') })
    const res = await p
    expect(res.ok).toBe(true)
  })

  it('surfaces a vault send failure instead of swallowing it', async () => {
    sendVaultFieldSpy.mockRejectedValueOnce('{"Internal":"no vault credential named \'admin\'"}')
    const steps: LoginStep[] = [
      {
        kind: 'send',
        expect: '',
        is_regex: false,
        send: '{{vault:admin.Password}}\\r',
        timeout_ms: 10000,
      },
    ]
    const res = await runMacro('sess1', steps)
    expect(res.ok).toBe(false)
    expect(res.failedStep).toBe(0)
    expect(res.error).toContain("no vault credential named 'admin'")
  })

  it('ignores output from a DIFFERENT session (expect times out)', async () => {
    const steps: LoginStep[] = [
      { kind: 'expect', expect: 'password:', is_regex: false, send: '', timeout_ms: 300 },
    ]
    const p = runMacro('sess1', steps)
    await tick()
    feed?.({ session_id: 'OTHER', data: bytes('password: ') })
    const res = await p
    expect(res.ok).toBe(false)
    expect(res.failedStep).toBe(0)
  })
})
