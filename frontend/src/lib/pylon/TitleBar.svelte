<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import type { PylonTheme } from '$lib/themes/index'
  import { themeLabels } from '$lib/themes/index'
  import Icon from '$lib/icons/Icon.svelte'
  import MarkTile from '$lib/icons/MarkTile.svelte'

  interface MenuItem {
    label: string
    /** Right-aligned keyboard-shortcut chip, e.g. `Ctrl+N`. */
    shortcut?: string
    action?: () => void
    divider?: boolean
    disabled?: boolean
    /** Render a leading check dot (active theme). */
    checked?: boolean
  }

  interface Props {
    theme: PylonTheme
    sessionName?: string
    onNewSession?: () => void
    onQuickConnect?: () => void
    onSettings?: () => void
    onRunScript?: () => void
    onScriptExamples?: () => void
    onNotebook?: () => void
    onCheckUpdates?: () => void
    onSetTheme?: (key: string) => void
    onAbout?: () => void
    onExport?: () => void
    onImport?: () => void
    onImportSshConfig?: () => void
    onImportPutty?: () => void
    onImportMobaXterm?: () => void
    onImportSecureCrt?: () => void
    onCommandList?: () => void
    onTunnelsManager?: () => void
    themeName?: string
  }

  const {
    theme,
    sessionName = '',
    onNewSession,
    onQuickConnect,
    onSettings,
    onRunScript,
    onScriptExamples,
    onNotebook,
    onCheckUpdates,
    onSetTheme,
    onAbout,
    onExport,
    onImport,
    onImportSshConfig,
    onImportPutty,
    onImportMobaXterm,
    onImportSecureCrt,
    onCommandList,
    onTunnelsManager,
    themeName = 'parchment',
  }: Props = $props()

  const win = getCurrentWindow()

  let openMenu = $state<string | null>(null)

  const menus = $derived<Record<string, MenuItem[]>>({
    File: [
      {
        label: 'Export sessions…',
        action: () => {
          onExport?.()
          openMenu = null
        },
      },
      {
        label: 'Import sessions…',
        action: () => {
          onImport?.()
          openMenu = null
        },
      },
      { label: '', divider: true },
      {
        label: 'Import from SSH config…',
        action: () => {
          onImportSshConfig?.()
          openMenu = null
        },
      },
      {
        label: 'Import from PuTTY…',
        action: () => {
          onImportPutty?.()
          openMenu = null
        },
      },
      {
        label: 'Import from MobaXterm…',
        action: () => {
          onImportMobaXterm?.()
          openMenu = null
        },
      },
      {
        label: 'Import from SecureCRT…',
        action: () => {
          onImportSecureCrt?.()
          openMenu = null
        },
      },
      { label: '', divider: true },
      { label: 'Quit', action: () => win.close() },
    ],
    View: Object.entries(themeLabels).map(([key, label]) => ({
      label,
      checked: themeName === key,
      action: () => {
        onSetTheme?.(key)
        openMenu = null
      },
    })),
    Session: [
      {
        label: 'New Session',
        shortcut: 'Ctrl+N',
        action: () => {
          onNewSession?.()
          openMenu = null
        },
      },
      {
        label: 'Quick Connect',
        shortcut: 'Ctrl+Shift+N',
        action: () => {
          onQuickConnect?.()
          openMenu = null
        },
      },
    ],
    Tools: [
      {
        label: 'Run Python script…',
        action: () => {
          onRunScript?.()
          openMenu = null
        },
      },
      {
        label: 'Script examples…',
        action: () => {
          onScriptExamples?.()
          openMenu = null
        },
      },
      {
        label: 'Allow notebook connection…',
        action: () => {
          onNotebook?.()
          openMenu = null
        },
      },
      {
        label: 'Send command list…',
        action: () => {
          onCommandList?.()
          openMenu = null
        },
      },
      {
        label: 'Active tunnels…',
        action: () => {
          onTunnelsManager?.()
          openMenu = null
        },
      },
      {
        label: 'Settings',
        shortcut: 'Ctrl+,',
        action: () => {
          onSettings?.()
          openMenu = null
        },
      },
    ],
    Help: [
      {
        label: 'Check for updates…',
        action: () => {
          onCheckUpdates?.()
          openMenu = null
        },
      },
      {
        label: 'About ZapTerm',
        action: () => {
          onAbout?.()
          openMenu = null
        },
      },
    ],
  })

  function toggleMenu(name: string) {
    openMenu = openMenu === name ? null : name
  }

  function closeMenus() {
    openMenu = null
  }
</script>

<!-- close menus on outside click -->
<svelte:window
  onclick={(e) => {
    const target = e.target as Element
    if (!target.closest?.('.tb-menu-wrap')) closeMenus()
  }}
/>

<header
  class="titlebar"
  style:background={theme.titlebarBg}
  style:border-bottom="1px solid {theme.border}"
  style:color={theme.textMuted}
  style:--item-hover-bg={theme.itemHoverBg}
  style:--text-primary={theme.textPrimary}
>
  <!-- Left: brand + active session -->
  <div class="tb-left">
    <MarkTile size={15} accent={theme.accent} paper={theme.appBg} />

    <span class="tb-wordmark" style:color={theme.textPrimary} style:font-family={theme.fontUi}>
      ZapTerm
    </span>

    {#if sessionName}
      <span class="tb-sep" style:color={theme.textDim}>/</span>
      <span class="tb-session" style:color={theme.textMuted} style:font-family={theme.fontUi}>
        {sessionName}
      </span>
    {/if}
  </div>

  <!-- Center: drag region -->
  <div class="tb-drag" data-tauri-drag-region></div>

  <!-- Right: menu strip + window controls -->
  <div class="tb-right">
    <nav class="tb-menu-bar" style:font-family={theme.fontUi}>
      {#each Object.entries(menus) as [name, items]}
        <div class="tb-menu-wrap" style:position="relative">
          <button
            class="tb-menu-item"
            class:active={openMenu === name}
            style:color={openMenu === name ? theme.textPrimary : theme.textMuted}
            style:background={openMenu === name ? theme.itemHoverBg : 'transparent'}
            onclick={() => toggleMenu(name)}>{name}</button
          >

          {#if openMenu === name}
            <div
              class="tb-dropdown"
              style:background={theme.sidebarBg}
              style:border="1px solid {theme.border}"
            >
              {#each items as item}
                {#if item.divider}
                  <div class="dd-divider" style:background={theme.border}></div>
                {:else}
                  <button
                    class="dd-item"
                    disabled={item.disabled}
                    style:color={item.disabled ? theme.textDim : theme.textPrimary}
                    style:font-family={theme.fontUi}
                    onclick={item.action}
                  >
                    <span class="dd-check" style:color={theme.accent}>
                      {#if item.checked}<span class="dd-check-dot"></span>{/if}
                    </span>
                    <span class="dd-label">{item.label}</span>
                    {#if item.shortcut}
                      <kbd
                        class="dd-kbd"
                        style:border="1px solid {theme.border}"
                        style:color={theme.textDim}
                        style:font-family={theme.fontMono}>{item.shortcut}</kbd
                      >
                    {/if}
                  </button>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </nav>

    <div class="tb-controls">
      <button
        class="tb-ctrl"
        title="Minimize"
        style:color={theme.textDim}
        onclick={() => win.minimize()}
      >
        <Icon name="min" size={13} />
      </button>
      <button
        class="tb-ctrl"
        title="Maximize"
        style:color={theme.textDim}
        onclick={() => win.toggleMaximize()}
      >
        <Icon name="max" size={12} />
      </button>
      <button
        class="tb-ctrl tb-ctrl-close"
        title="Close"
        style:color={theme.textDim}
        onclick={() => win.close()}
      >
        <Icon name="x" size={13} />
      </button>
    </div>
  </div>
</header>

<style>
  .titlebar {
    height: 38px;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    user-select: none;
    -webkit-user-select: none;
    position: relative;
    z-index: 100;
  }

  .tb-left {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .tb-wordmark {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 1.5px;
  }

  .tb-sep {
    font-size: 12px;
  }

  .tb-session {
    font-size: 12px;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tb-drag {
    flex: 1;
    height: 100%;
  }

  .tb-right {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .tb-menu-bar {
    display: flex;
    align-items: center;
  }

  .tb-menu-wrap {
    position: relative;
  }

  .tb-menu-item {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    font-weight: 400;
    padding: 4px 10px;
    border-radius: 5px;
    color: inherit;
    transition:
      color 0.1s,
      background 0.1s;
  }

  .tb-menu-item:hover,
  .tb-menu-item.active {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.06));
    color: var(--text-primary, #2c2924);
  }

  .tb-dropdown {
    position: absolute;
    top: 32px;
    left: 0;
    min-width: 180px;
    border-radius: 7px;
    padding: 4px 0;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
    z-index: 200;
  }

  .dd-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    padding: 5px 12px 5px 8px;
    white-space: nowrap;
    transition: background 0.08s;
    font-family: inherit;
  }

  .dd-check {
    width: 10px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .dd-label {
    flex: 1;
  }

  .dd-kbd {
    flex-shrink: 0;
    margin-left: 18px;
    font-size: 10px;
    line-height: 1.4;
    padding: 1px 5px;
    border-radius: 4px;
  }

  .dd-check-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
  }

  .dd-item:not(:disabled):hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.07));
  }

  .dd-item:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .dd-divider {
    height: 1px;
    margin: 4px 0;
  }

  /* Sit above the window resize handles (z-index 9999) so the min/max/close
     buttons stay clickable where the top-right corner handle overlaps them. */
  .tb-controls {
    display: flex;
    height: 100%;
    position: relative;
    z-index: 10000;
  }

  .tb-ctrl {
    background: none;
    border: none;
    cursor: pointer;
    width: 44px;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background 0.1s,
      color 0.1s;
    color: inherit;
  }

  .tb-ctrl:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.08));
    color: var(--text-primary, #2c2924);
  }

  .tb-ctrl-close:hover {
    background: #e81123;
    color: #ffffff;
  }
</style>
