<script lang="ts">
  import { fly } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import type { PylonTheme } from '$lib/themes/index'
  import Icon from '$lib/icons/Icon.svelte'

  export interface TabEntry {
    id: number
    label: string
    color: string
    status: 'connecting' | 'connected' | 'error' | 'lost' | 'closed'
  }

  interface Props {
    theme: PylonTheme
    tabs: TabEntry[]
    activeTabId: number
    splitOn?: boolean
    multiOn?: boolean
    onActivate: (id: number) => void
    onAdd: () => void
    onClose: (id: number) => void
    onRename?: (id: number, label: string) => void
    onDuplicate?: (id: number) => void
    onSetColor?: (id: number, color: string) => void
    onToggleSplit?: () => void
    onToggleMulti?: () => void
  }

  const {
    theme,
    tabs,
    activeTabId,
    splitOn = false,
    multiOn = false,
    onActivate,
    onAdd,
    onClose,
    onRename,
    onDuplicate,
    onSetColor,
    onToggleSplit,
    onToggleMulti,
  }: Props = $props()

  function statusColor(status: TabEntry['status']): string {
    if (status === 'connected') return theme.ok
    if (status === 'error' || status === 'lost') return theme.err
    if (status === 'closed') return theme.textDim
    return theme.warn
  }

  let hoveredTab = $state<number | null>(null)

  /** Each tab is a chip tinted with its session colour: a light wash when
   *  idle, stronger on hover, and the active one a solid surface with a
   *  clearer tint, so it reads at a glance which tab is which and which is
   *  in front. */
  function tabBackground(color: string, active: boolean, hovered: boolean): string {
    if (active) return `color-mix(in srgb, ${color} 20%, ${theme.tabActiveBg})`
    return `color-mix(in srgb, ${color} ${hovered ? 16 : 9}%, transparent)`
  }

  // Swatch palette for the per-tab colour (right-click → colour).
  const TAB_COLORS = [
    '#ef4444',
    '#f59e0b',
    '#eab308',
    '#22c55e',
    '#06b6d4',
    '#3b82f6',
    '#8b5cf6',
    '#ec4899',
  ]

  // Inline rename (double-click the label).
  let editingTab = $state<number | null>(null)
  let editValue = $state('')

  function startRename(tab: TabEntry) {
    editingTab = tab.id
    editValue = tab.label
  }
  function commitRename() {
    if (editingTab != null) onRename?.(editingTab, editValue)
    editingTab = null
  }

  // Right-click context menu.
  let menuTab = $state<number | null>(null)
  let menuX = $state(0)
  let menuY = $state(0)

  function openMenu(e: MouseEvent, tab: TabEntry) {
    e.preventDefault()
    menuTab = tab.id
    menuX = e.clientX
    menuY = e.clientY
  }
  function closeMenu() {
    menuTab = null
  }
</script>

<div
  class="tabbar"
  style:background={theme.tabBarBg}
  style:border-bottom="1px solid {theme.border}"
  style:font-family={theme.fontUi}
  style:--item-hover-bg={theme.itemHoverBg}
  style:--text-primary={theme.textPrimary}
  style:--err={theme.err}
>
  <!-- Tabs strip -->
  <div class="tab-strip">
    {#each tabs as tab (tab.id)}
      {@const active = tab.id === activeTabId}
      {@const hovered = hoveredTab === tab.id}
      <div
        class="tab"
        class:tab-active={active}
        in:fly={{ y: -8, duration: 180, easing: cubicOut }}
        style:background={tabBackground(tab.color, active, hovered)}
        style:border="1px solid {active
          ? `color-mix(in srgb, ${tab.color} 55%, ${theme.border})`
          : `color-mix(in srgb, ${tab.color} 22%, transparent)`}"
        style:border-radius={theme.radius}
        style:box-shadow={active
          ? `inset 0 3px 0 ${tab.color}, 0 1px 6px color-mix(in srgb, ${tab.color} 22%, transparent)`
          : `inset 0 2px 0 color-mix(in srgb, ${tab.color} 60%, transparent)`}
        onclick={() => onActivate(tab.id)}
        ondblclick={() => startRename(tab)}
        oncontextmenu={(e) => openMenu(e, tab)}
        onmouseenter={() => (hoveredTab = tab.id)}
        onmouseleave={() => (hoveredTab = null)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === 'Enter' && onActivate(tab.id)}
      >
        <span
          class="tab-dot"
          class:pulse={tab.status === 'connecting'}
          style:background={statusColor(tab.status)}
        ></span>
        {#if editingTab === tab.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="tab-rename"
            bind:value={editValue}
            autofocus
            onclick={(e) => e.stopPropagation()}
            onblur={commitRename}
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                commitRename()
              }
              if (e.key === 'Escape') {
                editingTab = null
              }
            }}
          />
        {:else}
          <span class="tab-label" style:color={active ? theme.textPrimary : theme.textMuted}>
            {tab.label}
          </span>
        {/if}
        {#if active || hovered}
          <button
            class="tab-close"
            style:color={theme.textDim}
            onclick={(e) => {
              e.stopPropagation()
              onClose(tab.id)
            }}
            title="Close tab"><Icon name="x" size={11} /></button
          >
        {/if}
      </div>
    {/each}

    <!-- New tab button -->
    <button
      class="tab-add"
      style:color={theme.textDim}
      style:border-radius={theme.radius}
      onclick={onAdd}
      title="New tab"><Icon name="plus" size={14} /></button
    >
  </div>

  <!-- Right controls: Split + Multi -->
  <div class="tab-actions">
    <button
      class="tab-action-btn"
      class:active={splitOn}
      style:color={splitOn ? theme.accent : theme.textMuted}
      style:background={splitOn ? `${theme.accent}18` : 'transparent'}
      style:border="1px solid {splitOn ? `${theme.accent}55` : theme.border}"
      style:border-radius={theme.radius}
      onclick={onToggleSplit}
      title="Toggle split pane (Ctrl+\)"
    >
      <Icon name="split" size={13} />
      Split
    </button>
    <button
      class="tab-action-btn"
      class:active={multiOn}
      style:color={multiOn ? theme.accent2 : theme.textMuted}
      style:background={multiOn ? `${theme.accent2}18` : 'transparent'}
      style:border="1px solid {multiOn ? `${theme.accent2}55` : theme.border}"
      style:border-radius={theme.radius}
      onclick={onToggleMulti}
      title="Toggle MultiExec broadcast (Ctrl+Shift+M)"
    >
      <Icon name="cast" size={13} />
      Multi
    </button>
  </div>
</div>

{#if menuTab !== null}
  <button
    class="menu-backdrop"
    aria-label="Close menu"
    onclick={closeMenu}
    oncontextmenu={(e) => {
      e.preventDefault()
      closeMenu()
    }}
  ></button>
  <div
    class="tab-menu"
    style:left="{menuX}px"
    style:top="{menuY}px"
    style:background={theme.tabActiveBg}
    style:border="1px solid {theme.border}"
    style:color={theme.textPrimary}
  >
    <button
      class="menu-item"
      onclick={() => {
        const t = tabs.find((x) => x.id === menuTab)
        if (t) startRename(t)
        closeMenu()
      }}>Rename</button
    >
    <button
      class="menu-item"
      onclick={() => {
        if (menuTab != null) onDuplicate?.(menuTab)
        closeMenu()
      }}>Duplicate</button
    >
    <div class="menu-colors">
      {#each TAB_COLORS as c (c)}
        <button
          class="swatch"
          style:background={c}
          title="Set tab colour"
          aria-label="Set tab colour"
          onclick={() => {
            if (menuTab != null) onSetColor?.(menuTab, c)
            closeMenu()
          }}
        ></button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .tabbar {
    height: 42px;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    overflow: hidden;
    padding: 0 10px;
    gap: 4px;
  }

  .tab-strip {
    display: flex;
    align-items: center;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    gap: 4px;
    scrollbar-width: none;
  }

  .tab-strip::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 10px 0 11px;
    min-width: 140px;
    max-width: 220px;
    cursor: pointer;
    flex-shrink: 0;
    position: relative;
    transition:
      background 0.12s,
      border-color 0.12s,
      box-shadow 0.12s;
  }

  .tab-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  /* Connecting: gentle pulse, mirroring the sidebar avatar's status dot so
     tabs and the session list speak the same visual language. */
  .tab-dot.pulse {
    animation: tab-dot-pulse 1.3s ease-in-out infinite;
  }

  @keyframes tab-dot-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tab-dot.pulse {
      animation: none;
    }
  }

  .tab-label {
    font-size: 11.5px;
    font-weight: 400;
    font-family: inherit;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color 0.1s;
  }

  .tab-active .tab-label {
    font-weight: 600;
  }

  .tab-close {
    background: none;
    border: none;
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    color: inherit;
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .tab-close:hover {
    background: color-mix(in srgb, var(--err) 15%, transparent);
    color: var(--err);
  }

  .tab-add {
    background: none;
    border: none;
    cursor: pointer;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition:
      background 0.1s,
      color 0.1s;
    color: inherit;
  }

  .tab-add:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.06));
    color: var(--text-primary, #2c2924);
  }

  .tab-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .tab-action-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    cursor: pointer;
    font-size: 11.5px;
    font-weight: 500;
    padding: 0 10px;
    height: 28px;
    font-family: inherit;
    transition:
      background 0.1s,
      color 0.1s,
      border-color 0.1s;
    color: inherit;
  }

  .tab-action-btn:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .tab-rename {
    flex: 1;
    min-width: 0;
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.1));
    border: 1px solid var(--text-primary, #888);
    border-radius: 3px;
    color: var(--text-primary, #2c2924);
    font: inherit;
    font-size: 12.5px;
    padding: 1px 4px;
    outline: none;
  }

  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 400;
    background: none;
    border: none;
    padding: 0;
    cursor: default;
  }

  .tab-menu {
    position: fixed;
    z-index: 401;
    min-width: 150px;
    padding: 4px;
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12.5px;
  }

  .menu-item {
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    padding: 6px 10px;
    border-radius: 4px;
    color: inherit;
    font: inherit;
  }

  .menu-item:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .menu-colors {
    display: flex;
    gap: 5px;
    padding: 6px 10px 4px;
    flex-wrap: wrap;
  }

  .swatch {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid rgba(0, 0, 0, 0.3);
    cursor: pointer;
    padding: 0;
  }

  .swatch:hover {
    transform: scale(1.15);
  }
</style>
