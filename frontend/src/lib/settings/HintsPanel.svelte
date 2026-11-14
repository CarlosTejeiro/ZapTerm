<script lang="ts">
  import { ask } from '@tauri-apps/plugin-dialog'
  import { errorText } from '$lib/bridge/errors'
  import { onMount, onDestroy } from 'svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import {
    listPlatforms,
    clearCommandHistory,
    reloadHintCatalogs,
    hintCatalogsDir,
  } from '$lib/bridge/commands'
  import type { PlatformInfo } from '$lib/bridge/types'
  import {
    hintsSettings,
    setGhostEnabled,
    setPopupEnabled,
    setDefaultPlatform,
  } from '$lib/hints/store.svelte'

  let platforms = $state<PlatformInfo[]>([])
  let clearingHistory = $state(false)
  let cleared = $state(false)

  // Custom catalog UI state.
  let catalogStatus = $state<{ kind: 'ok' | 'err'; text: string } | null>(null)
  let catalogDir = $state<string | null>(null)
  let reloading = $state(false)

  function flashCatalog(kind: 'ok' | 'err', text: string) {
    catalogStatus = { kind, text }
    setTimeout(() => {
      if (catalogStatus?.text === text) catalogStatus = null
    }, 3500)
  }

  async function openCatalogsDir() {
    try {
      const path = await hintCatalogsDir()
      catalogDir = path
      flashCatalog('ok', path ? `Carpeta abierta: ${path}` : 'Carpeta no configurada.')
    } catch (e) {
      flashCatalog('err', errorText(e))
    }
  }

  async function reloadCatalogs() {
    reloading = true
    try {
      const n = await reloadHintCatalogs()
      flashCatalog(
        'ok',
        n === 0
          ? 'No user files — using the built-in catalogs.'
          : `Reloaded: ${n} catalog${n === 1 ? '' : 's'} replaced by your files.`,
      )
    } catch (e) {
      flashCatalog('err', errorText(e))
    } finally {
      reloading = false
    }
  }

  async function refresh() {
    platforms = await listPlatforms()
  }

  async function clearAllHistory() {
    if (
      !(await ask("Clear the entire command history? This can't be undone.", {
        title: 'Hints',
        kind: 'warning',
      }))
    )
      return
    clearingHistory = true
    try {
      await clearCommandHistory(null)
      cleared = true
      setTimeout(() => (cleared = false), 2500)
    } finally {
      clearingHistory = false
    }
  }

  let unlistenReload: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null

  onMount(async () => {
    await refresh()
    // Surface auto-reloads emitted by the backend file watcher so the user
    // sees that their edit took effect without pressing the button.
    if (destroyed) return
    unlistenReload = await listen<number>('hint-catalogs-reloaded', (e) => {
      flashCatalog(
        'ok',
        e.payload === 0
          ? 'Changes detected — using the built-in catalogs.'
          : `Auto-reloaded: ${e.payload} catalog${e.payload === 1 ? '' : 's'} in use.`,
      )
    })
    unlistenError = await listen<string>('hint-catalogs-error', (e) => {
      flashCatalog('err', `Reload failed: ${e.payload}`)
    })
    // Closed while a `listen` was pending: onDestroy already ran.
    if (destroyed) {
      unlistenReload?.()
      unlistenError?.()
    }
  })

  let destroyed = false
  onDestroy(() => {
    destroyed = true
    unlistenReload?.()
    unlistenError?.()
  })
</script>

<div class="panel">
  <section>
    <h3>Appearance</h3>
    <label class="toggle">
      <input
        type="checkbox"
        checked={hintsSettings.ghostEnabled}
        onchange={(e) => setGhostEnabled((e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Show inline ghost text (zsh-autosuggestions style)</span>
    </label>
    <p class="hint-help">→ or End to accept the suggestion.</p>

    <label class="toggle">
      <input
        type="checkbox"
        checked={hintsSettings.popupEnabled}
        onchange={(e) => setPopupEnabled((e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Enable the suggestions popup on Ctrl+Space</span>
    </label>
    <p class="hint-help">↑↓ to navigate, Tab/Enter to accept, Esc to close.</p>
  </section>

  <section>
    <h3>Default platform</h3>
    <p class="hint-help">Command catalog to use when a session has no specific platform.</p>
    <select
      value={hintsSettings.defaultPlatform}
      onchange={(e) => setDefaultPlatform((e.currentTarget as HTMLSelectElement).value)}
    >
      {#each platforms as p (p.id)}
        <option value={p.id}>{p.name}</option>
      {/each}
    </select>
  </section>

  <section>
    <h3>Custom catalogs</h3>
    <p class="hint-help">
      Replace the built-in commands with your own lists: drop a
      <code>&lt;platform&gt;.json</code> file (e.g. <code>fortigate.json</code>) into the catalogs
      folder. It reloads <strong>automatically</strong> when you save the file — the button is only there
      if you want to force it.
    </p>
    <div class="catalog-actions">
      <button type="button" class="catalog-btn" onclick={openCatalogsDir}>
        <Icon name="folder" size={13} /> Open catalogs folder
      </button>
      <button type="button" class="catalog-btn" onclick={reloadCatalogs} disabled={reloading}>
        <Icon name="refresh" size={13} />
        {reloading ? 'Reloading…' : 'Reload catalogs'}
      </button>
    </div>
    {#if catalogStatus}
      <p class="catalog-msg" class:err={catalogStatus.kind === 'err'}>{catalogStatus.text}</p>
    {/if}
    {#if catalogDir}
      <p class="catalog-path" title={catalogDir}>{catalogDir}</p>
    {/if}
  </section>

  <section>
    <h3>History</h3>
    <p class="hint-help">
      Commands are saved automatically per session. Anything that looks like it contains passwords
      or tokens never enters the history.
    </p>
    <button type="button" class="danger-btn" onclick={clearAllHistory} disabled={clearingHistory}>
      {#if cleared}<Icon name="check" size={13} /> History cleared{:else if clearingHistory}Clearing…{:else}Clear
        entire history{/if}
    </button>
  </section>
</div>

<style>
  .panel {
    padding: 0.75rem 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0 0 0.15rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    cursor: pointer;
    color: var(--zx-text);
  }

  .toggle input {
    accent-color: var(--zx-accent);
  }

  .hint-help {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0 0 0.25rem 0;
    line-height: 1.45;
  }

  select {
    width: 16rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.3rem 0.4rem;
    color: var(--zx-text);
    font-size: 0.82rem;
  }

  .danger-btn {
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
    color: var(--zx-err);
    border: none;
    border-radius: 0.25rem;
    padding: 0.3rem 0.7rem;
    font-size: 0.78rem;
    cursor: pointer;
    font-weight: 600;
    font-family: inherit;
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .danger-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-err) 25%, transparent);
  }
  .danger-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .catalog-actions {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .catalog-btn {
    background: color-mix(in srgb, var(--zx-accent) 15%, transparent);
    color: var(--zx-accent);
    border: 1px solid color-mix(in srgb, var(--zx-accent) 30%, transparent);
    border-radius: 0.25rem;
    padding: 0.3rem 0.65rem;
    font-size: 0.78rem;
    cursor: pointer;
    font-family: inherit;
  }

  .catalog-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-accent) 25%, transparent);
  }

  .catalog-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .catalog-msg {
    font-size: 0.72rem;
    color: var(--zx-ok);
    margin: 0;
  }

  .catalog-msg.err {
    color: var(--zx-err);
  }

  .catalog-path {
    font-size: 0.7rem;
    color: var(--zx-text-muted);
    font-family: var(--zx-font-mono);
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  code {
    background: var(--zx-surface-2);
    border-radius: 0.2rem;
    padding: 0 0.3rem;
    font-size: 0.72rem;
  }
</style>
