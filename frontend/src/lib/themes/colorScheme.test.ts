import { describe, expect, it } from 'vitest'
import { colorSchemeFor, themes } from './index'

describe('colorSchemeFor', () => {
  it('classifies the bundled themes', () => {
    expect(colorSchemeFor(themes.parchment!.appBg)).toBe('light')
    expect(colorSchemeFor(themes.porcelain!.appBg)).toBe('light')
    expect(colorSchemeFor(themes.midnight!.appBg)).toBe('dark')
    expect(colorSchemeFor(themes.aurora!.appBg)).toBe('dark')
    expect(colorSchemeFor(themes.ember!.appBg)).toBe('dark')
  })

  it('falls back to light for colors it cannot parse', () => {
    expect(colorSchemeFor('rgb(0,0,0)')).toBe('light')
  })
})
