<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import { open as openDialog, ask } from '@tauri-apps/plugin-dialog'
  import {
    getDataDirInfo,
    setDataDir,
    resetDataDir,
    restartApp,
    type DataDirInfo,
  } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'

  let info = $state<DataDirInfo | null>(null)
  let busy = $state(false)
  let pendingRestart = $state(false)

  const SOURCE_LABELS: Record<DataDirInfo['source'], string> = {
    'cli-flag': '--data-dir flag',
    'env-var': 'ZAPTERM_DATA_DIR variable',
    portable: 'portable mode (marker next to the executable)',
    pointer: 'custom folder',
    default: 'system default location',
  }

  onMount(async () => {
    try {
      info = await getDataDirInfo()
    } catch (e) {
      showToast({ kind: 'error', title: 'Data', detail: errorText(e) })
    }
  })

  async function promptRestart() {
    pendingRestart = true
    const yes = await ask(
      'The change takes effect when ZapTerm restarts. Restart now?\n\nOpen sessions will be closed.',
      { title: 'Restart ZapTerm', kind: 'info' },
    )
    if (yes) await restartApp()
  }

  async function changeFolder() {
    if (busy) return
    const dir = await openDialog({ directory: true, title: 'ZapTerm data folder' })
    if (typeof dir !== 'string' || !dir) return
    busy = true
    try {
      const summary = await setDataDir(dir)
      showToast({ kind: 'success', title: 'Data folder', detail: summary })
      await promptRestart()
    } catch (e) {
      showToast({ kind: 'error', title: 'Data folder', detail: errorText(e) })
    } finally {
      busy = false
    }
  }

  async function useDefault() {
    if (busy) return
    busy = true
    try {
      await resetDataDir()
      showToast({
        kind: 'success',
        title: 'Data folder',
        detail:
          'The default location will be used from the next launch. Current data is not moved.',
      })
      await promptRestart()
    } catch (e) {
      showToast({ kind: 'error', title: 'Data folder', detail: errorText(e) })
    } finally {
      busy = false
    }
  }
</script>

<div class="panel">
  <section>
    <h3>Data folder</h3>
    <p class="hint">
      Where ZapTerm keeps the sessions database (<code>zapterm.db</code>), session logs and the
      command lists and hint catalogs. Handy for moving it to a synced folder (Drive, OneDrive) or a
      network share.
    </p>

    {#if !info}
      <p class="hint">Loading…</p>
    {:else}
      <code class="path">{info.path}</code>
      <p class="hint">
        Source: <strong>{SOURCE_LABELS[info.source]}</strong>
        {#if pendingRestart}
          · <strong>restart pending</strong>
        {/if}
      </p>

      {#if info.portable}
        <p class="notice">
          You're in <strong>portable mode</strong>: data lives next to the executable and
          credentials are stored encrypted (AES-256-GCM) in the database instead of the OS keyring.
          Anyone with access to the folder can use them — treat it like a physical key.
        </p>
      {:else if !info.changeable}
        <p class="notice">
          The location is forced by {SOURCE_LABELS[info.source]} and can't be changed here.
        </p>
      {:else}
        <div class="actions">
          <button type="button" class="ok-btn" disabled={busy} onclick={changeFolder}>
            Change folder…
          </button>
          {#if info.source === 'pointer'}
            <button type="button" class="ghost-btn" disabled={busy} onclick={useDefault}>
              Back to default location
            </button>
          {/if}
        </div>
        <p class="hint">
          Changing it copies the database, logs and catalogs to the new folder (if it already holds
          ZapTerm data, that's adopted as-is). The change takes effect after a restart. Default: <code
            >{info.default_path}</code
          >
        </p>
      {/if}
    {/if}
  </section>

  <section>
    <h3>Portable mode (Windows)</h3>
    <p class="hint">
      Download <code>ZapTerm_x64_portable.exe</code> from the release, put it in any folder or USB
      stick and create an empty file named
      <code>portable</code> next to it. All data then lives in <code>data/</code>
      beside the executable and travels with it.
    </p>
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
    gap: 0.5rem;
  }
  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0;
    line-height: 1.5;
  }
  .hint code {
    background: color-mix(in srgb, var(--zx-accent) 10%, transparent);
    color: var(--zx-accent);
    padding: 0.02rem 0.3rem;
    border-radius: 3px;
    font-size: 0.68rem;
    word-break: break-all;
  }
  .path {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    padding: 0.3rem 0.5rem;
    border-radius: 3px;
    word-break: break-all;
  }
  .notice {
    font-size: 0.72rem;
    line-height: 1.5;
    margin: 0;
    padding: 0.45rem 0.6rem;
    border: 1px solid color-mix(in srgb, var(--zx-warn) 45%, transparent);
    background: color-mix(in srgb, var(--zx-warn) 10%, transparent);
    border-radius: var(--zx-radius);
    color: var(--zx-text);
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .ok-btn {
    align-self: flex-start;
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    border: 0;
    border-radius: 4px;
    font-size: 0.78rem;
    font-family: inherit;
    font-weight: 600;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }
  .ok-btn:hover {
    filter: brightness(1.1);
  }
  .ok-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .ghost-btn {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    font-size: 0.78rem;
    font-family: inherit;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }
  .ghost-btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .ghost-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
