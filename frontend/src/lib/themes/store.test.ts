import { describe, it, expect, vi, beforeAll } from 'vitest'

vi.mock('$lib/bridge/commands', () => ({ setSetting: vi.fn() }))

// Vitest has no Svelte compiler: give the runes a plain implementation.
type Store = typeof import('./store.svelte')
let store: Store
beforeAll(async () => {
  ;(globalThis as unknown as { $state: <T>(v: T) => T }).$state = (v) => v
  store = await import('./store.svelte')
})

describe('theme store', () => {
  it('maps the themes renamed in 0.29.0 to their replacement', () => {
    store.setTheme('fjord')
    expect(store.getTheme().name).toBe('midnight')
    store.setTheme('nocturne')
    expect(store.getTheme().name).toBe('aurora')
    store.setTheme('oxide')
    expect(store.getTheme().name).toBe('ember')
  })

  it('keeps the current theme for an unknown name', () => {
    store.setTheme('porcelain')
    store.setTheme('no-such-theme')
    expect(store.getTheme().name).toBe('porcelain')
  })
})
