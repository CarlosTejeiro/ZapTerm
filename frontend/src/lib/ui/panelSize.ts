// Sizes of the floating automation panels (AI assistant, notebook, script
// run). Pure arithmetic, free of Svelte and the DOM so it can be unit-tested.
// The panels sit in the bottom-right corner, so they grow up and to the left.

export interface PanelSize {
  w: number
  h: number
}

/** The gaps a floating panel keeps to the window's edges: its CSS puts it
 *  14 px from the right and 40 px from the bottom, and the top gap leaves
 *  the title bar and its menus in reach. */
const GAP = { left: 14, right: 14, top: 44, bottom: 40 }

/** The biggest a panel can be in a window of size `win`. */
export function roomFor(win: PanelSize): PanelSize {
  return {
    w: Math.max(0, Math.floor(win.w - GAP.left - GAP.right)),
    h: Math.max(0, Math.floor(win.h - GAP.top - GAP.bottom)),
  }
}

/** `want`, no smaller than `min` and no bigger than what fits in the window
 *  (in a window smaller than `min`, what fits wins). */
export function fitSize(want: PanelSize, win: PanelSize, min: PanelSize): PanelSize {
  const room = roomFor(win)
  return {
    w: Math.round(Math.min(Math.max(want.w, min.w), room.w)),
    h: Math.round(Math.min(Math.max(want.h, min.h), room.h)),
  }
}

/** Which way a handle resizes: the left edge, the top edge, or their corner. */
export type DragAxis = 'x' | 'y' | 'both'

/** The size after dragging a handle by (`dx`, `dy`) from where it was
 *  pressed: moving left or up makes the panel bigger. */
export function dragSize(start: PanelSize, dx: number, dy: number, axis: DragAxis): PanelSize {
  return {
    w: axis === 'y' ? start.w : start.w - dx,
    h: axis === 'x' ? start.h : start.h - dy,
  }
}

/** A saved size (`640x480`) back into numbers; `null` for anything else. */
export function parseSize(text: string | null | undefined): PanelSize | null {
  const m = /^(\d{2,5})x(\d{2,5})$/.exec((text ?? '').trim())
  return m ? { w: Number(m[1]), h: Number(m[2]) } : null
}

export function formatSize(size: PanelSize): string {
  return `${Math.round(size.w)}x${Math.round(size.h)}`
}
