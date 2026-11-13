// Recursive split-pane tree for the workspace.
//
// Each `AppTab` owns one [`PaneTreeNode`] root. A leaf wraps a [`PaneData`]
// (the connection / metadata for a single terminal). A `split` node holds two
// children plus a direction and a 0..1 ratio for the divider position. The
// renderer ([`SplitTree.svelte`]) draws the flat [`layoutTree`] of it, so a
// reshaped tree never remounts (and reconnects) a terminal.

import type { PaneData } from '$lib/pylon/Pane.svelte'

export type SplitDirection = 'h' | 'v'

export type PaneTreeNode =
  | { kind: 'leaf'; pane: PaneData }
  | {
      kind: 'split'
      direction: SplitDirection
      /** Fraction of the split given to child `a` (`0.1..0.9` clamped at draw-time). */
      ratio: number
      a: PaneTreeNode
      b: PaneTreeNode
    }

/** Build a leaf node. */
export function leaf(pane: PaneData): PaneTreeNode {
  return { kind: 'leaf', pane }
}

/** Apply `visit` to every leaf in the tree (depth-first, left-to-right). */
export function eachLeaf(node: PaneTreeNode, visit: (p: PaneData) => void): void {
  if (node.kind === 'leaf') {
    visit(node.pane)
  } else {
    eachLeaf(node.a, visit)
    eachLeaf(node.b, visit)
  }
}

/** Locate the [`PaneData`] for a given pane id, or `null` if not in the tree. */
export function findLeaf(node: PaneTreeNode, paneId: number): PaneData | null {
  if (node.kind === 'leaf') return node.pane.id === paneId ? node.pane : null
  return findLeaf(node.a, paneId) ?? findLeaf(node.b, paneId)
}

/** Id of the first (left/top) leaf in the tree — used to refocus after a close. */
export function firstLeafId(node: PaneTreeNode): number {
  return node.kind === 'leaf' ? node.pane.id : firstLeafId(node.a)
}

/** Total leaf count in the subtree. */
export function leafCount(node: PaneTreeNode): number {
  return node.kind === 'leaf' ? 1 : leafCount(node.a) + leafCount(node.b)
}

/**
 * Replace the leaf identified by `paneId` with a split node. The original
 * leaf becomes child `a` (top/left); `newPane` becomes child `b` (bottom/right).
 * If `paneId` isn't found the tree is returned unchanged.
 */
export function splitLeaf(
  node: PaneTreeNode,
  paneId: number,
  direction: SplitDirection,
  newPane: PaneData,
): PaneTreeNode {
  if (node.kind === 'leaf') {
    if (node.pane.id !== paneId) return node
    return { kind: 'split', direction, ratio: 0.5, a: node, b: leaf(newPane) }
  }
  const a = splitLeaf(node.a, paneId, direction, newPane)
  if (a !== node.a) return { ...node, a }
  const b = splitLeaf(node.b, paneId, direction, newPane)
  if (b !== node.b) return { ...node, b }
  return node
}

/**
 * Remove the leaf identified by `paneId`. If the leaf is the root, returns
 * `null` (caller should drop the tab). Otherwise the leaf's sibling replaces
 * the parent split node, collapsing the tree as expected.
 */
export function removeLeaf(node: PaneTreeNode, paneId: number): PaneTreeNode | null {
  if (node.kind === 'leaf') return node.pane.id === paneId ? null : node
  if (node.a.kind === 'leaf' && node.a.pane.id === paneId) return node.b
  if (node.b.kind === 'leaf' && node.b.pane.id === paneId) return node.a
  const a = removeLeaf(node.a, paneId)
  if (a === null) return node.b
  if (a !== node.a) return { ...node, a }
  const b = removeLeaf(node.b, paneId)
  if (b === null) return node.a
  if (b !== node.b) return { ...node, b }
  return node
}

/**
 * Update the ratio of the split node at `path` (array of 0/1 children indices
 * walked from the root). No-op if the path leads to a leaf.
 */
export function setRatio(node: PaneTreeNode, path: number[], ratio: number): PaneTreeNode {
  if (node.kind === 'leaf') return node
  if (path.length === 0) return { ...node, ratio }
  const [head, ...rest] = path
  if (head === 0) {
    const a = setRatio(node.a, rest, ratio)
    return a === node.a ? node : { ...node, a }
  }
  const b = setRatio(node.b, rest, ratio)
  return b === node.b ? node : { ...node, b }
}

/** A rectangle in fractions (0..1) of the tab's pane area. */
export interface Rect {
  x: number
  y: number
  w: number
  h: number
}

/** Where each leaf goes, and where each split's divider sits. */
export interface TreeLayout {
  panes: { pane: PaneData; rect: Rect }[]
  dividers: { path: number[]; direction: SplitDirection; ratio: number; rect: Rect }[]
}

const clampRatio = (r: number) => Math.max(0.1, Math.min(0.9, r))

/**
 * Flatten the tree into positioned leaves (`rect` of each pane) and dividers
 * (`rect` of the split they divide, plus its ratio). Rendering panes from this
 * flat list — keyed by pane id — keeps each terminal mounted when the tree is
 * reshaped: splitting a pane or closing its sibling used to move it to another
 * place in a recursive component tree, which tore it down and reconnected it.
 */
export function layoutTree(node: PaneTreeNode): TreeLayout {
  const out: TreeLayout = { panes: [], dividers: [] }
  const walk = (n: PaneTreeNode, rect: Rect, path: number[]) => {
    if (n.kind === 'leaf') {
      out.panes.push({ pane: n.pane, rect })
      return
    }
    const ratio = clampRatio(n.ratio)
    out.dividers.push({ path, direction: n.direction, ratio, rect })
    if (n.direction === 'h') {
      walk(n.a, { ...rect, w: rect.w * ratio }, [...path, 0])
      walk(n.b, { ...rect, x: rect.x + rect.w * ratio, w: rect.w * (1 - ratio) }, [...path, 1])
    } else {
      walk(n.a, { ...rect, h: rect.h * ratio }, [...path, 0])
      walk(n.b, { ...rect, y: rect.y + rect.h * ratio, h: rect.h * (1 - ratio) }, [...path, 1])
    }
  }
  walk(node, { x: 0, y: 0, w: 1, h: 1 }, [])
  return out
}
