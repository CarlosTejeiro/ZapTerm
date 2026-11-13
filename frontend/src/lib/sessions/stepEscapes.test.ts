import { describe, expect, it } from 'vitest'
import { escapeControls, unescapeControls } from './stepEscapes'

describe('login-step escapes', () => {
  it('turns typed escapes into characters', () => {
    expect(unescapeControls('admin\\r')).toBe('admin\r')
    expect(unescapeControls('a\\nb\\tc\\\\d')).toBe('a\nb\tc\\d')
    expect(unescapeControls('keep \\x and trailing \\')).toBe('keep \\x and trailing \\')
  })
  it('round-trips stored text exactly (Enter, newlines, backslashes)', () => {
    for (const stored of ['\r', 'admin\r', 'l1\nl2\n', 'C:\\path\\n', 'tab\there', '']) {
      expect(unescapeControls(escapeControls(stored))).toBe(stored)
    }
  })
  it('shows a bare Enter as \\r instead of an empty field', () => {
    expect(escapeControls('\r')).toBe('\\r')
  })
})
