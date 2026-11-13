import { describe, it, expect } from 'vitest'
import { isSaneDim, saneCols, saneRows, sanitizeDims, FALLBACK_COLS, FALLBACK_ROWS } from './dims'

describe('isSaneDim', () => {
  it('accepts finite numbers ≥ 2', () => {
    expect(isSaneDim(2)).toBe(true)
    expect(isSaneDim(80)).toBe(true)
    expect(isSaneDim(9999)).toBe(true)
  })

  it('rejects the degenerate 1×1 case and anything smaller', () => {
    expect(isSaneDim(1)).toBe(false)
    expect(isSaneDim(0)).toBe(false)
    expect(isSaneDim(-5)).toBe(false)
  })

  it('rejects non-finite values', () => {
    expect(isSaneDim(NaN)).toBe(false)
    expect(isSaneDim(Infinity)).toBe(false)
    expect(isSaneDim(-Infinity)).toBe(false)
    expect(isSaneDim(undefined as unknown as number)).toBe(false)
  })
})

describe('saneCols / saneRows', () => {
  it('passes valid values through (floored)', () => {
    expect(saneCols(120)).toBe(120)
    expect(saneRows(40)).toBe(40)
    expect(saneCols(80.9)).toBe(80)
    expect(saneRows(24.7)).toBe(24)
  })

  it('falls back for bad values', () => {
    expect(saneCols(1)).toBe(FALLBACK_COLS)
    expect(saneCols(0)).toBe(FALLBACK_COLS)
    expect(saneCols(NaN)).toBe(FALLBACK_COLS)
    expect(saneCols(undefined as unknown as number)).toBe(FALLBACK_COLS)
    expect(saneRows(1)).toBe(FALLBACK_ROWS)
    expect(saneRows(0)).toBe(FALLBACK_ROWS)
    expect(saneRows(NaN)).toBe(FALLBACK_ROWS)
    expect(saneRows(undefined as unknown as number)).toBe(FALLBACK_ROWS)
  })
})

describe('sanitizeDims', () => {
  it('passes a valid pair through unchanged', () => {
    expect(sanitizeDims(80, 24)).toEqual({ cols: 80, rows: 24 })
    expect(sanitizeDims(200, 50)).toEqual({ cols: 200, rows: 50 })
  })

  it('floors fractional measurements', () => {
    expect(sanitizeDims(100.6, 30.9)).toEqual({ cols: 100, rows: 30 })
  })

  it('falls back to 80×24 when either dimension is the degenerate 1', () => {
    expect(sanitizeDims(1, 1)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
    expect(sanitizeDims(1, 24)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
    expect(sanitizeDims(80, 1)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
  })

  it('falls back for 0 / NaN / undefined', () => {
    expect(sanitizeDims(0, 0)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
    expect(sanitizeDims(NaN, 24)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
    expect(sanitizeDims(80, NaN)).toEqual({ cols: FALLBACK_COLS, rows: FALLBACK_ROWS })
    expect(sanitizeDims(undefined as unknown as number, undefined as unknown as number)).toEqual({
      cols: FALLBACK_COLS,
      rows: FALLBACK_ROWS,
    })
  })
})
