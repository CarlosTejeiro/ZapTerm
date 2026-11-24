import { describe, expect, it } from 'vitest'
import { renderLine } from './renderLine'

describe('renderLine', () => {
  it('leaves plain text alone', () => {
    expect(renderLine('show version')).toBe('show version')
  })
  it('overwrites after a carriage return or backspaces, erasing nothing', () => {
    expect(renderLine('abcdef\rxy')).toBe('xycdef')
    expect(renderLine('abc\b\bX')).toBe('aXc')
    expect(renderLine('abc\r')).toBe('abc')
  })
})
