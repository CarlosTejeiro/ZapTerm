import { describe, expect, it } from 'vitest'

// Vitest runs without the Svelte compiler (see vitest.config.ts), so give the
// module the one rune it uses at load time: plain state is enough here.
;(globalThis as unknown as { $state: <T>(v: T) => T }).$state = (v) => v
const {
  paneToSession,
  registerSession,
  unregisterSession,
  broadcastGroup,
  broadcastTargets,
  claimBroadcastScope,
  releaseBroadcastScope,
} = await import('./sessionRuntime.svelte')

describe('pane ↔ session registry', () => {
  it('a torn-down terminal does not drop the session that replaced it', () => {
    registerSession(7, 'old')
    registerSession(7, 'new') // the replacement opened first
    unregisterSession(7, 'old') // then the old one is destroyed
    expect(paneToSession.get(7)).toBe('new')
    unregisterSession(7, 'new')
    expect(paneToSession.has(7)).toBe(false)
  })

  it('without a session id it forgets the pane unconditionally', () => {
    registerSession(8, 'x')
    unregisterSession(8)
    expect(paneToSession.has(8)).toBe(false)
  })
})

describe('grid broadcast scope', () => {
  // Two grid tabs stay mounted; switching from B to A runs A's claim and B's
  // release in either order. Typing in A must then reach only A's group.
  it('the active grid keeps the scope whatever order the effects run in', () => {
    const a = {}
    const b = {}
    registerSession(21, 'a1')
    registerSession(22, 'a2')
    registerSession(23, 'b1')
    claimBroadcastScope(b, new Set(['b1'])) // B opened last
    claimBroadcastScope(a, new Set(['a1', 'a2'])) // switch to A: A runs first…
    releaseBroadcastScope(b) // …then B's teardown, which must not clear A's
    expect(broadcastTargets('a1')).toEqual(['a2'])

    claimBroadcastScope(b, new Set(['b1'])) // back to B, other order
    releaseBroadcastScope(a)
    expect(broadcastGroup.activeSessionIds).toEqual(new Set(['b1']))
    releaseBroadcastScope(b) // B closed → all panes again
    expect(broadcastGroup.activeSessionIds).toBeNull()
    for (const p of [21, 22, 23]) unregisterSession(p)
  })
})
