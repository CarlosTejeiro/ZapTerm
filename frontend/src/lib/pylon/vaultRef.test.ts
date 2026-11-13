import { describe, it, expect } from 'vitest'
import { vaultRefStatus } from './vaultRef'

describe('vaultRefStatus', () => {
  it('accepts well-formed refs the runner resolves correctly', () => {
    // `\r` here is a literal backslash-r (the trailing-submit marker).
    expect(vaultRefStatus('{{vault:admin.Password}}\\r')).toBe('valid')
    expect(vaultRefStatus('{{vault:admin.Username}}')).toBe('valid')
    expect(vaultRefStatus('{{vault:12}}')).toBe('valid')
    // Bare name (no dot) is a runner-supported shortcut → password.
    expect(vaultRefStatus('{{vault:admin}}')).toBe('valid')
  })

  it('flags likely field typos (has a dot, unknown suffix) as malformed', () => {
    expect(vaultRefStatus('{{vault:admin.Pass word}}')).toBe('malformed') // space in field
    expect(vaultRefStatus('{{vault:admin.Passwd}}')).toBe('malformed') // misspelled field
  })

  it('returns none for non-references', () => {
    expect(vaultRefStatus('ls -la\\r')).toBe('none')
    expect(vaultRefStatus('')).toBe('none')
    expect(vaultRefStatus('vault:admin.Password')).toBe('none')
  })
})
