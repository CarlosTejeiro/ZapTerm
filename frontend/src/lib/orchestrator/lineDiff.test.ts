import { describe, expect, it } from 'vitest'
import { lineDiff } from './lineDiff'

const kinds = (d: { kind: string; text: string }[]) => d.map((l) => `${l.kind}:${l.text}`)

describe('lineDiff', () => {
  it('marks changed, removed and added lines', () => {
    expect(kinds(lineDiff('a\nb\nc', 'a\nx\nc\nd'))).toEqual([
      'same:a',
      'a:b',
      'b:x',
      'same:c',
      'b:d',
    ])
  })

  it('handles identical and empty inputs', () => {
    expect(kinds(lineDiff('a\nb', 'a\nb'))).toEqual(['same:a', 'same:b'])
    expect(kinds(lineDiff('', ''))).toEqual(['same:'])
  })

  it('diffs two huge configs that differ in one line, quickly', () => {
    const base = Array.from({ length: 30_000 }, (_, i) => `set line ${i}`)
    const other = [...base]
    other[15_000] = 'set line CHANGED'
    const t0 = Date.now()
    const d = lineDiff(base.join('\n'), other.join('\n'))
    expect(Date.now() - t0).toBeLessThan(2000)
    expect(d.filter((l) => l.kind !== 'same').map((l) => l.text)).toEqual([
      'set line 15000',
      'set line CHANGED',
    ])
  })

  it('falls back to a block change instead of an enormous table', () => {
    const a = Array.from({ length: 50 }, (_, i) => `a${i}`).join('\n')
    const b = Array.from({ length: 50 }, (_, i) => `b${i}`).join('\n')
    const d = lineDiff(a, b, 100) // force the cap
    expect(d.filter((l) => l.kind === 'a')).toHaveLength(50)
    expect(d.filter((l) => l.kind === 'b')).toHaveLength(50)
  })
})
