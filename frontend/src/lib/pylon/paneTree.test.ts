import { describe, expect, it } from 'vitest'
import { layoutTree, leaf, removeLeaf, splitLeaf, type PaneTreeNode } from './paneTree'
import type { PaneData } from './Pane.svelte'

const pane = (id: number) => ({ id, label: `p${id}`, color: '#000' }) as PaneData

describe('layoutTree', () => {
  it('places a lone pane over the whole area, with no divider', () => {
    const l = layoutTree(leaf(pane(1)))
    expect(l.panes).toEqual([{ pane: pane(1), rect: { x: 0, y: 0, w: 1, h: 1 } }])
    expect(l.dividers).toEqual([])
  })

  it('splits by ratio, nested, with each divider on its own split', () => {
    // 1 | (2 over 3), the left side 30 %.
    let t: PaneTreeNode = splitLeaf(leaf(pane(1)), 1, 'h', pane(2))
    t = splitLeaf(t, 2, 'v', pane(3))
    if (t.kind !== 'split') throw new Error('expected a split')
    t = { ...t, ratio: 0.3 }
    const l = layoutTree(t)
    const rect = (id: number) => l.panes.find((p) => p.pane.id === id)!.rect
    expect(rect(1)).toEqual({ x: 0, y: 0, w: 0.3, h: 1 })
    expect(rect(2).x).toBeCloseTo(0.3)
    expect(rect(2).h).toBeCloseTo(0.5)
    expect(rect(3).y).toBeCloseTo(0.5)
    expect(l.dividers.map((d) => [d.path, d.direction])).toEqual([
      [[], 'h'],
      [[1], 'v'],
    ])
  })

  it('keeps the same pane objects across a split and a close (no remount)', () => {
    const one = leaf(pane(1))
    const split = splitLeaf(one, 1, 'h', pane(2))
    const back = removeLeaf(split, 2)!
    const ids = (t: PaneTreeNode) => layoutTree(t).panes.map((p) => p.pane.id)
    expect(ids(split)).toEqual([1, 2])
    expect(ids(back)).toEqual([1])
    // The surviving pane is the very same object: keyed rendering keeps it.
    expect(layoutTree(back).panes[0]!.pane).toBe(layoutTree(one).panes[0]!.pane)
  })

  it('clamps an out-of-range ratio like the old renderer', () => {
    const t = splitLeaf(leaf(pane(1)), 1, 'h', pane(2))
    if (t.kind !== 'split') throw new Error('expected a split')
    expect(layoutTree({ ...t, ratio: 0.99 }).dividers[0]!.ratio).toBe(0.9)
  })
})
