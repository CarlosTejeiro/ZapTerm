<script lang="ts">
  import Pane from './Pane.svelte'
  import { layoutTree, type PaneTreeNode, type Rect, type SplitDirection } from './paneTree'
  import type { PylonTheme } from '$lib/themes/index'

  type PaneStatus = 'connecting' | 'connected' | 'error' | 'lost' | 'closed'

  interface Props {
    node: PaneTreeNode
    theme: PylonTheme
    focusedPaneId: number
    /** When true, each leaf shows a close button — disable when only one leaf left. */
    canClosePanes: boolean
    onFocus: (paneId: number) => void
    onSplit: (paneId: number, direction: SplitDirection) => void
    onClosePane: (paneId: number) => void
    onStatusChange: (paneId: number, status: PaneStatus) => void
    onGlobalShortcut?: (key: string, e: KeyboardEvent) => void
    onResize: (path: number[], ratio: number) => void
  }

  const {
    node,
    theme,
    focusedPaneId,
    canClosePanes,
    onFocus,
    onSplit,
    onClosePane,
    onStatusChange,
    onGlobalShortcut,
    onResize,
  }: Props = $props()

  // Panes are drawn from a FLAT list keyed by pane id and placed absolutely,
  // never nested per split: reshaping the tree (a split, or closing a
  // sibling) then only moves a pane, it doesn't remount it — remounting tore
  // down the terminal and reconnected its session, losing the scrollback.
  const layout = $derived(layoutTree(node))
  const multiple = $derived(layout.panes.length > 1)

  /** Half of the gap between cards (the divider is the whole gap). */
  const HALF_GAP = 5
  const EPS = 1e-6

  /** CSS box for a pane: its rect, inset by half a gap on inner edges. */
  function paneBox(r: Rect): string {
    const l = r.x > EPS ? HALF_GAP : 0
    const t = r.y > EPS ? HALF_GAP : 0
    const rr = r.x + r.w < 1 - EPS ? HALF_GAP : 0
    const b = r.y + r.h < 1 - EPS ? HALF_GAP : 0
    return (
      `left:calc(${r.x * 100}% + ${l}px);top:calc(${r.y * 100}% + ${t}px);` +
      `width:calc(${r.w * 100}% - ${l + rr}px);height:calc(${r.h * 100}% - ${t + b}px)`
    )
  }

  /** CSS box for a divider: the gap at `ratio` across its split's rect. */
  function dividerBox(r: Rect, direction: SplitDirection, ratio: number): string {
    if (direction === 'h') {
      const x = r.x + r.w * ratio
      return (
        `left:calc(${x * 100}% - ${HALF_GAP}px);top:${r.y * 100}%;` +
        `width:${HALF_GAP * 2}px;height:${r.h * 100}%`
      )
    }
    const y = r.y + r.h * ratio
    return (
      `top:calc(${y * 100}% - ${HALF_GAP}px);left:${r.x * 100}%;` +
      `height:${HALF_GAP * 2}px;width:${r.w * 100}%`
    )
  }

  let containerEl = $state<HTMLDivElement | null>(null)

  function startResize(e: MouseEvent, path: number[], direction: SplitDirection, rect: Rect) {
    e.preventDefault()
    e.stopPropagation()
    const container = containerEl
    if (!container) return
    const box = container.getBoundingClientRect()
    function onMove(me: MouseEvent) {
      const ratio =
        direction === 'h'
          ? (me.clientX - (box.left + rect.x * box.width)) / (rect.w * box.width)
          : (me.clientY - (box.top + rect.y * box.height)) / (rect.h * box.height)
      onResize(path, Math.max(0.1, Math.min(0.9, ratio)))
    }
    function onUp() {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }
</script>

<div class="split-area" bind:this={containerEl}>
  {#each layout.panes as { pane, rect } (pane.id)}
    <div class="split-cell" style={paneBox(rect)}>
      <Pane
        {theme}
        {pane}
        focused={focusedPaneId === pane.id}
        hasSiblings={multiple}
        canClose={canClosePanes}
        onFocus={() => onFocus(pane.id)}
        onSplitH={() => onSplit(pane.id, 'h')}
        onSplitV={() => onSplit(pane.id, 'v')}
        onClosePane={() => onClosePane(pane.id)}
        onStatusChange={(s) => onStatusChange(pane.id, s)}
        {onGlobalShortcut}
      />
    </div>
  {/each}
  {#each layout.dividers as d (d.path.join('.'))}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="split-handle"
      class:vertical={d.direction === 'v'}
      style={dividerBox(d.rect, d.direction, d.ratio)}
      style:--accent={theme.accent}
      role="separator"
      aria-orientation={d.direction === 'h' ? 'vertical' : 'horizontal'}
      onmousedown={(e) => startResize(e, d.path, d.direction, d.rect)}
    ></div>
  {/each}
</div>

<style>
  .split-area {
    position: relative;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }
  .split-cell {
    position: absolute;
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Floating-card layout: the handle is the transparent 10px gap between
     terminal cards; an accent stripe appears while hovering/dragging. */
  .split-handle {
    position: absolute;
    cursor: col-resize;
    background: transparent;
    transition: background 0.1s;
    user-select: none;
    border-radius: 3px;
    z-index: 1;
  }
  .split-handle:hover {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .split-handle.vertical {
    cursor: row-resize;
  }
</style>
