import { describe, expect, it } from 'vitest'
import { dragSize, fitSize, formatSize, parseSize, roomFor } from './panelSize'

const WIN = { w: 1440, h: 900 }
const MIN = { w: 420, h: 300 }

describe('roomFor', () => {
  it('leaves the gaps the panel keeps to the window edges', () => {
    expect(roomFor(WIN)).toEqual({ w: 1412, h: 816 })
  })
  it('never goes below zero', () => {
    expect(roomFor({ w: 10, h: 10 })).toEqual({ w: 0, h: 0 })
  })
})

describe('fitSize', () => {
  it('keeps a size that fits', () => {
    expect(fitSize({ w: 800, h: 500 }, WIN, MIN)).toEqual({ w: 800, h: 500 })
  })
  it('does not go below the minimum', () => {
    expect(fitSize({ w: 100, h: 50 }, WIN, MIN)).toEqual(MIN)
  })
  it('does not go beyond the window', () => {
    expect(fitSize({ w: 5000, h: 5000 }, WIN, MIN)).toEqual({ w: 1412, h: 816 })
  })
  it('lets the window win when it is smaller than the minimum', () => {
    expect(fitSize({ w: 800, h: 500 }, { w: 400, h: 300 }, MIN)).toEqual({ w: 372, h: 216 })
  })
})

describe('dragSize', () => {
  const start = { w: 600, h: 400 }
  it('grows when the corner moves up and left', () => {
    expect(dragSize(start, -100, -50, 'both')).toEqual({ w: 700, h: 450 })
  })
  it('shrinks when it moves down and right', () => {
    expect(dragSize(start, 80, 40, 'both')).toEqual({ w: 520, h: 360 })
  })
  it('changes only the width from the left edge', () => {
    expect(dragSize(start, -100, -50, 'x')).toEqual({ w: 700, h: 400 })
  })
  it('changes only the height from the top edge', () => {
    expect(dragSize(start, -100, -50, 'y')).toEqual({ w: 600, h: 450 })
  })
})

describe('saved sizes', () => {
  it('round-trip', () => {
    expect(parseSize(formatSize({ w: 812.4, h: 600 }))).toEqual({ w: 812, h: 600 })
  })
  it('anything else reads as no saved size', () => {
    for (const bad of [null, undefined, '', 'big', '800', '800x', 'x600', '-5x10', '8x6'])
      expect(parseSize(bad)).toBeNull()
  })
})
