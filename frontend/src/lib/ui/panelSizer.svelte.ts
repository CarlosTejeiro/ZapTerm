// The size of one floating automation panel: the default its CSS gives, a
// size the user dragged it to (remembered between runs), or "large" (as big
// as the window allows, until restored).

import { innerHeight, innerWidth } from 'svelte/reactivity/window'
import { getSetting, setSetting } from '$lib/bridge/commands'
import {
  dragSize,
  fitSize,
  formatSize,
  parseSize,
  roomFor,
  type DragAxis,
  type PanelSize,
} from './panelSize'

export interface PanelSizer {
  /** The size to give the panel, or `null` to leave it to its CSS. */
  readonly size: PanelSize | null
  readonly large: boolean
  readonly dragging: boolean
  /** Fill the window, or go back to the size it had. */
  toggleLarge(): void
  /** Back to the default size (and forget the dragged one). */
  reset(): void
  /** `onpointerdown` of a resize handle of `panel`. */
  startDrag(e: PointerEvent, panel: HTMLElement | null, axis: DragAxis): void
}

/**
 * @param key  the setting the dragged size is saved under
 * @param min  the smallest size the panel is still usable at
 */
export function createPanelSizer(key: string, min: PanelSize): PanelSizer {
  let custom = $state<PanelSize | null>(null)
  let large = $state(false)
  let dragging = $state(false)

  // What the user does before the saved size arrives wins over it.
  let touched = false
  getSetting(key)
    .then((saved) => {
      if (!touched) custom = parseSize(saved)
    })
    .catch(() => {
      /* no saved size: the default one */
    })

  const win = (): PanelSize => ({ w: innerWidth.current ?? 1280, h: innerHeight.current ?? 800 })
  const size = $derived(large ? roomFor(win()) : custom ? fitSize(custom, win(), min) : null)

  function save(value: PanelSize | null) {
    setSetting(key, value ? formatSize(value) : '').catch(console.error)
  }

  return {
    get size() {
      return size
    },
    get large() {
      return large
    },
    get dragging() {
      return dragging
    },
    toggleLarge() {
      large = !large
    },
    reset() {
      touched = true
      large = false
      custom = null
      save(null)
    },
    startDrag(e, panel, axis) {
      if (!panel || e.button !== 0) return
      e.preventDefault()
      const handle = e.currentTarget as HTMLElement
      const box = panel.getBoundingClientRect()
      const start = { w: box.width, h: box.height }
      const from = { x: e.clientX, y: e.clientY }
      dragging = true
      touched = true
      // Dragging from "large" carries on from that size.
      large = false
      custom = fitSize(start, win(), min)
      // Capture the pointer so a release outside the window still ends the drag.
      handle.setPointerCapture(e.pointerId)
      const move = (ev: PointerEvent) => {
        custom = fitSize(
          dragSize(start, ev.clientX - from.x, ev.clientY - from.y, axis),
          win(),
          min,
        )
      }
      const end = () => {
        dragging = false
        handle.removeEventListener('pointermove', move)
        handle.removeEventListener('pointerup', end)
        handle.removeEventListener('pointercancel', end)
        try {
          handle.releasePointerCapture(e.pointerId)
        } catch {
          /* the capture may already be gone (pointercancel) */
        }
        save(custom)
      }
      handle.addEventListener('pointermove', move)
      handle.addEventListener('pointerup', end)
      handle.addEventListener('pointercancel', end)
    },
  }
}
