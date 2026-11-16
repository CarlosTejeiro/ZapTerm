import { describe, expect, it } from 'vitest'
import { passwordProblem } from './password'

describe('passwordProblem', () => {
  it('asks for the minimum length first', () => {
    expect(passwordProblem('short', 'short', 14)).toBe('At least 14 characters.')
    expect(passwordProblem('', '', 14)).toBe('At least 14 characters.')
  })

  it('counts characters, not UTF-16 units or bytes', () => {
    expect(passwordProblem('ñ'.repeat(14), 'ñ'.repeat(14), 14)).toBe('')
    expect(passwordProblem('ñ'.repeat(13), 'ñ'.repeat(13), 14)).not.toBe('')
    // 7 emoji are 14 UTF-16 units but only 7 characters.
    expect(passwordProblem('😀'.repeat(7), '😀'.repeat(7), 14)).not.toBe('')
  })

  it('needs both fields to match', () => {
    expect(passwordProblem('correct horse battery', 'correct horse batterx', 14)).toBe(
      "The passwords don't match.",
    )
    expect(passwordProblem('correct horse battery', 'correct horse battery', 14)).toBe('')
  })
})
