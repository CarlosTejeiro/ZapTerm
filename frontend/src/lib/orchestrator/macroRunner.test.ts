import { describe, it, expect } from 'vitest'
import { matchEnd } from './macroMatch'
import type { LoginStep } from '$lib/bridge/types'

function expectStep(pattern: string, is_regex = false): LoginStep {
  return { kind: 'expect', expect: pattern, is_regex, send: '', timeout_ms: 10000 }
}

describe('matchEnd', () => {
  it('returns the end index for a literal match in the middle of the buffer', () => {
    // "login:" ends at index 6 within "..." prefix + match.
    const buf = 'welcome\nlogin: '
    const end = matchEnd(expectStep('login:'), buf)
    expect(end).toBe(buf.indexOf('login:') + 'login:'.length)
  })

  it('returns -1 when there is no match', () => {
    expect(matchEnd(expectStep('password:'), 'nothing here')).toBe(-1)
  })

  it('treats an empty pattern as no match', () => {
    expect(matchEnd(expectStep(''), 'anything')).toBe(-1)
  })

  it('consuming past the match end prevents a later expect from re-matching', () => {
    const buf = 'login: user\npassword: '
    const first = matchEnd(expectStep('login:'), buf)
    expect(first).toBeGreaterThanOrEqual(0)
    const rest = buf.slice(first)
    // "login:" must not be found again in the consumed remainder…
    expect(matchEnd(expectStep('login:'), rest)).toBe(-1)
    // …but a later prompt still matches in what remains.
    expect(matchEnd(expectStep('password:'), rest)).toBeGreaterThanOrEqual(0)
  })

  it('matches a regex pattern and returns its end offset', () => {
    const buf = 'foo password:   '
    const end = matchEnd(expectStep('password:\\s*$', true), buf)
    expect(end).toBe(buf.length)
  })

  it('falls back to a literal search when the regex is invalid', () => {
    const buf = 'a(b'
    // "(" is an invalid regex; literal fallback finds "(b".
    const end = matchEnd(expectStep('(b', true), buf)
    expect(end).toBe(3)
  })

  it('returns a zero-width regex match as its start index (matches immediately, terminates)', () => {
    // `a*` / `^` match empty at index 0. matchEnd must return 0 (>= 0 → the
    // runner treats it as an immediate match and breaks, so no infinite poll).
    expect(matchEnd(expectStep('a*', true), 'xyz')).toBe(0)
    expect(matchEnd(expectStep('^', true), 'xyz')).toBe(0)
  })
})
