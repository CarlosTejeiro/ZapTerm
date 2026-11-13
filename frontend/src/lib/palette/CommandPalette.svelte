<script lang="ts">
  import { fuzzyMatch } from './fuzzy'
  import Icon from '$lib/icons/Icon.svelte'
  import type { IconName } from '$lib/icons/icons'

  export interface PaletteItem {
    id: string
    label: string
    subtitle?: string
    icon: IconName
    shortcut?: string
    section: 'Sessions' | 'Themes' | 'Actions' | 'Snippets' | 'Groups'
    run: () => void
  }

  interface Props {
    items: PaletteItem[]
    onClose: () => void
  }

  let { items, onClose }: Props = $props()

  let query = $state('')
  let selected = $state(0)
  let inputEl = $state<HTMLInputElement | null>(null)

  interface Ranked extends PaletteItem {
    score: number
    indices: number[]
  }

  const ranked = $derived.by<Ranked[]>(() => {
    const q = query.trim()
    if (!q) {
      return items.map((it) => ({ ...it, score: 0, indices: [] }))
    }
    const out: Ranked[] = []
    for (const it of items) {
      const r = fuzzyMatch(q, it.label)
      if (r) out.push({ ...it, score: r.score, indices: r.indices })
    }
    out.sort((a, b) => b.score - a.score)
    return out
  })

  function bySection(list: Ranked[]): { section: string; items: Ranked[] }[] {
    const map = new Map<string, Ranked[]>()
    for (const it of list) {
      if (!map.has(it.section)) map.set(it.section, [])
      map.get(it.section)!.push(it)
    }
    const order = ['Sessions', 'Snippets', 'Actions', 'Themes']
    return [...map.entries()]
      .sort(([a], [b]) => order.indexOf(a) - order.indexOf(b))
      .map(([section, items]) => ({ section, items }))
  }

  // Flat list for keyboard navigation in the visible order.
  const sections = $derived(bySection(ranked))
  const flat = $derived(sections.flatMap((g) => g.items))

  function highlight(text: string, indices: number[]): string {
    if (indices.length === 0) return escapeHtml(text)
    let out = ''
    let i = 0
    const set = new Set(indices)
    while (i < text.length) {
      if (set.has(i)) {
        out += `<mark>${escapeHtml(text[i] ?? '')}</mark>`
      } else {
        out += escapeHtml(text[i] ?? '')
      }
      i++
    }
    return out
  }

  function escapeHtml(s: string): string {
    return s.replace(/[&<>"]/g, (c) =>
      c === '&' ? '&amp;' : c === '<' ? '&lt;' : c === '>' ? '&gt;' : '&quot;',
    )
  }

  function execute(i: number) {
    const item = flat[i]
    if (!item) return
    onClose()
    queueMicrotask(() => item.run())
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
      return
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      selected = Math.min(selected + 1, flat.length - 1)
      return
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      selected = Math.max(selected - 1, 0)
      return
    }
    if (e.key === 'Enter') {
      e.preventDefault()
      execute(selected)
    }
  }

  $effect(() => {
    if (query) selected = 0
  })

  $effect(() => {
    if (inputEl) inputEl.focus()
  })
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Command Palette"
  tabindex="-1"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose()
  }}
  onkeydown={onKeydown}
>
  <div class="palette">
    <div class="search">
      <span class="icon" aria-hidden="true"><Icon name="search" size={16} /></span>
      <input
        bind:this={inputEl}
        bind:value={query}
        type="text"
        placeholder="Jump to sessions, run actions, switch theme…"
        spellcheck="false"
        autocomplete="off"
      />
      <kbd>Esc</kbd>
    </div>

    <div class="results" role="listbox">
      {#each sections as group (group.section)}
        <div class="section-header">{group.section}</div>
        {#each group.items as it (it.id)}
          {@const i = flat.findIndex((f) => f.id === it.id)}
          <button
            type="button"
            class="row"
            class:selected={i === selected}
            onmouseenter={() => (selected = i)}
            onclick={() => execute(i)}
            role="option"
            aria-selected={i === selected}
          >
            <span class="row-icon"><Icon name={it.icon} size={15} /></span>
            <span class="row-main">
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <span class="row-label">{@html highlight(it.label, it.indices)}</span>
              {#if it.subtitle}<span class="row-subtitle">{it.subtitle}</span>{/if}
            </span>
            {#if it.shortcut}
              <kbd class="row-shortcut">{it.shortcut}</kbd>
            {/if}
          </button>
        {/each}
      {/each}

      {#if flat.length === 0}
        <div class="empty">Sin resultados para "{query}"</div>
      {/if}
    </div>

    <div class="footer">
      <span><kbd>↑↓</kbd> navega</span>
      <span><kbd>Enter</kbd> ejecuta</span>
      <span><kbd>Esc</kbd> cierra</span>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    z-index: 1000;
    animation: overlay-in 140ms ease-out;
  }

  @keyframes overlay-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .palette {
    width: 36rem;
    max-width: 92vw;
    max-height: 70vh;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    box-shadow: var(--zx-shadow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: palette-in 180ms cubic-bezier(0.2, 0.8, 0.3, 1);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }

  @keyframes palette-in {
    from {
      opacity: 0;
      transform: translateY(-12px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.85rem 1rem 0.65rem;
    border-bottom: 1px solid var(--zx-border);
  }

  .icon {
    color: var(--zx-accent);
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
  }

  .search input {
    flex: 1;
    background: transparent;
    border: 0;
    outline: none;
    color: var(--zx-text);
    font-size: 0.95rem;
    font-family: inherit;
  }

  .search input::placeholder {
    color: var(--zx-text-dim);
  }

  kbd {
    background: var(--zx-hover-bg);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    padding: 0.05rem 0.4rem;
    font-size: 0.62rem;
    font-family: var(--zx-font-mono);
    color: var(--zx-text-muted);
  }

  .results {
    flex: 1;
    overflow-y: auto;
    padding: 0.35rem;
    min-height: 0;
  }

  .section-header {
    font-size: 0.62rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--zx-text-dim);
    padding: 0.5rem 0.6rem 0.2rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    width: 100%;
    padding: 0.45rem 0.6rem;
    background: transparent;
    border: 0;
    border-radius: 6px;
    text-align: left;
    color: inherit;
    cursor: pointer;
    font: inherit;
    font-size: 0.85rem;
  }

  .row.selected {
    background: color-mix(in srgb, var(--zx-accent) 16%, transparent);
    box-shadow: inset 2px 0 0 var(--zx-accent);
  }

  .row-icon {
    width: 1.3rem;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--zx-text-muted);
  }

  .row.selected .row-icon {
    color: var(--zx-accent);
  }

  .row-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    overflow: hidden;
  }

  .row-label :global(mark) {
    background: transparent;
    color: var(--zx-accent);
    font-weight: 700;
  }

  .row-subtitle {
    font-size: 0.7rem;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-shortcut {
    flex-shrink: 0;
  }

  .empty {
    padding: 1rem;
    text-align: center;
    color: var(--zx-text-dim);
    font-size: 0.82rem;
  }

  .footer {
    display: flex;
    gap: 0.85rem;
    justify-content: flex-end;
    padding: 0.45rem 0.85rem;
    border-top: 1px solid var(--zx-border);
    font-size: 0.68rem;
    color: var(--zx-text-dim);
  }

  .footer span {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
</style>
