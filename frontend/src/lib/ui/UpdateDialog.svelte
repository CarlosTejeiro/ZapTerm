<script lang="ts">
  import { onMount } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { errorText } from '$lib/bridge/errors'
  import {
    checkForUpdate,
    downloadUpdate,
    installUpdate,
    revealUpdate,
    openExternal,
    type UpdateInfo,
  } from '$lib/bridge/commands'

  interface Props {
    onClose: () => void
  }
  let { onClose }: Props = $props()

  type Phase = 'checking' | 'result' | 'downloading' | 'downloaded' | 'installing' | 'error'
  let phase = $state<Phase>('checking')
  let info = $state<UpdateInfo | null>(null)
  let error = $state<string | null>(null)
  let downloaded = $state(0)
  let total = $state(0)
  let filePath = $state('')

  const RELEASES = 'https://github.com/CarlosTejeiro/ZapTerm/releases'
  const busy = $derived(phase === 'checking' || phase === 'downloading' || phase === 'installing')
  const percent = $derived(total ? Math.min(100, Math.round((downloaded / total) * 100)) : 0)

  function mb(n: number) {
    return `${(n / 1_048_576).toFixed(1)} MB`
  }

  function date(iso: string | null) {
    if (!iso) return ''
    const d = new Date(iso)
    return isNaN(d.getTime())
      ? ''
      : d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
  }

  async function check() {
    phase = 'checking'
    error = null
    try {
      info = await checkForUpdate()
      phase = 'result'
    } catch (e) {
      error = errorText(e)
      phase = 'error'
    }
  }

  async function download() {
    phase = 'downloading'
    downloaded = 0
    total = info?.asset?.size ?? 0
    error = null
    try {
      filePath = await downloadUpdate()
      phase = 'downloaded'
    } catch (e) {
      error = errorText(e)
      phase = 'error'
    }
  }

  async function install() {
    phase = 'installing'
    try {
      const closing = await installUpdate()
      if (!closing) phase = 'downloaded'
    } catch (e) {
      error = errorText(e)
      phase = 'error'
    }
  }

  async function reveal() {
    try {
      await revealUpdate()
    } catch (e) {
      error = errorText(e)
    }
  }

  function open(url: string) {
    openExternal(url).catch((e) => (error = errorText(e)))
  }

  function close() {
    if (!busy) onClose()
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined
    listen<{ downloaded: number; total: number }>('update-download-progress', (e) => {
      downloaded = e.payload.downloaded
      total = e.payload.total
    }).then((u) => (unlisten = u))
    check()
    return () => unlisten?.()
  })

  /** What installing means for this kind of install, shown before and after downloading. */
  const howTo = $derived.by(() => {
    switch (info?.install_kind) {
      case 'nsis':
      case 'msi':
        return 'ZapTerm closes so the installer can replace it; open sessions are disconnected.'
      case 'dmg':
        return 'ZapTerm closes, the new version takes the place of this one and ZapTerm opens again; open sessions are disconnected. Where ZapTerm cannot replace itself, the disk image opens instead: drag ZapTerm to Applications.'
      case 'deb':
      case 'rpm':
        return 'The package opens in your software installer. If nothing opens, install it from a terminal (for example sudo apt install ./file.deb).'
      case 'portable':
        return 'Replace your ZapTerm portable EXE with the downloaded one (keep the "portable" file and the data folder next to it).'
      case 'appimage':
        return 'Replace your current AppImage with the downloaded one.'
      default:
        return ''
    }
  })
  const installLabel = $derived.by(() => {
    switch (info?.install_kind) {
      case 'nsis':
      case 'msi':
        return 'Install now'
      case 'dmg':
        return 'Install and reopen'
      case 'deb':
      case 'rpm':
        return 'Open installer'
      default:
        return 'Show in folder'
    }
  })
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Check for updates"
  tabindex="-1"
  onkeydown={(e) => {
    if (e.key === 'Escape') close()
  }}
  onclick={(e) => {
    if (e.target === e.currentTarget) close()
  }}
  transition:fade={{ duration: 120 }}
>
  <div class="dialog" transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}>
    <h3>Check for updates</h3>

    {#if phase === 'checking'}
      <p class="muted">Asking GitHub for the latest version…</p>
    {:else if phase === 'error'}
      <p class="err">{error}</p>
      <p class="muted">You can always download ZapTerm from the releases page.</p>
      <div class="actions">
        <button type="button" class="secondary" onclick={() => open(RELEASES)}>
          Open releases page
        </button>
        <button type="button" class="secondary" onclick={check}>Try again</button>
        <button type="button" class="primary" onclick={onClose}>Close</button>
      </div>
    {:else if info && !info.newer}
      <p>
        You have the latest version: <b>{info.current}</b>{#if info.beta_channel}
          (beta){/if}.
      </p>
      {#if info.latest && info.latest !== info.current}
        <p class="muted">The newest release on GitHub is {info.latest}.</p>
      {/if}
      <div class="actions">
        {#if info.tag}
          <button type="button" class="secondary" onclick={() => open(info!.release_url)}>
            Release notes
          </button>
        {/if}
        <button type="button" class="primary" onclick={onClose}>Close</button>
      </div>
    {:else if info}
      <p>
        <b>ZapTerm {info.latest}</b>{#if info.prerelease}
          <span class="tag">beta</span>{/if} is available — you have {info.current}.
        {#if info.published_at}<span class="muted"> Published {date(info.published_at)}.</span>{/if}
      </p>
      {#if info.notes}
        <pre class="notes">{info.notes}</pre>
      {/if}

      {#if phase === 'downloading'}
        <div
          class="bar"
          role="progressbar"
          aria-valuenow={percent}
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div class="fill" style:width="{percent}%"></div>
        </div>
        <p class="muted">Downloading {info.asset?.name} — {mb(downloaded)} of {mb(total)}</p>
      {:else if phase === 'downloaded' || phase === 'installing'}
        <p class="ok">Downloaded and verified: <span class="path">{filePath}</span></p>
        {#if howTo}<p class="muted">{howTo}</p>{/if}
        <div class="actions">
          {#if installLabel !== 'Show in folder'}
            <button type="button" class="secondary" onclick={reveal}>Show in folder</button>
          {/if}
          <button
            type="button"
            class="secondary"
            onclick={onClose}
            disabled={phase === 'installing'}>Later</button
          >
          <button
            type="button"
            class="primary"
            onclick={installLabel === 'Show in folder' ? reveal : install}
            disabled={phase === 'installing'}
            >{phase === 'installing' ? 'Starting…' : installLabel}</button
          >
        </div>
      {:else if info.install_kind === 'store'}
        <p class="muted">
          This copy comes from the Microsoft Store, which installs updates itself. If it hasn't yet,
          open the Store and check Library → Updates.
        </p>
        <div class="actions">
          <button type="button" class="secondary" onclick={() => open(info!.release_url)}
            >Release notes</button
          >
          <button type="button" class="primary" onclick={onClose}>Close</button>
        </div>
      {:else if info.asset}
        <p class="muted">
          Downloads {info.asset.name} ({mb(info.asset.size)}) from GitHub into your Downloads
          folder.
          {howTo}
        </p>
        <div class="actions">
          <button type="button" class="secondary" onclick={() => open(info!.release_url)}
            >Release notes</button
          >
          <button type="button" class="secondary" onclick={onClose}>Not now</button>
          <button type="button" class="primary" onclick={download}>Download and install</button>
        </div>
      {:else}
        <p class="muted">Download it from the release page.</p>
        <div class="actions">
          <button type="button" class="secondary" onclick={onClose}>Not now</button>
          <button type="button" class="primary" onclick={() => open(info!.release_url)}
            >Open release page</button
          >
        </div>
      {/if}
    {/if}
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
    align-items: center;
    justify-content: center;
    z-index: 1100;
  }
  .dialog {
    width: 30rem;
    max-width: 92vw;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    padding: var(--zx-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-2);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    font-size: 0.85rem;
    box-shadow: var(--zx-shadow);
  }
  h3 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .muted {
    color: var(--zx-text-muted);
    font-size: 0.8rem;
  }
  .err {
    color: var(--zx-err);
  }
  .ok {
    color: var(--zx-ok);
  }
  .path {
    font-family: var(--zx-font-mono);
    font-size: 0.75rem;
    word-break: break-all;
    color: var(--zx-text-muted);
  }
  .tag {
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border: 1px solid var(--zx-border);
    border-radius: 3px;
    padding: 0 0.3rem;
    margin-left: 0.3rem;
    color: var(--zx-warn);
  }
  .notes {
    margin: 0;
    max-height: 12rem;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    line-height: 1.45;
    background: color-mix(in srgb, var(--zx-text) 5%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 6px;
    padding: var(--zx-space-2);
    color: var(--zx-text-muted);
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--zx-text) 10%, transparent);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--zx-accent);
    transition: width 0.2s ease;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: var(--zx-space-1);
    margin-top: 0.2rem;
  }
  .actions button {
    border: 0;
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-3);
    font-size: 0.8rem;
    font-family: inherit;
    cursor: pointer;
  }
  .actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .secondary {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .secondary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-text) 12%, transparent);
  }
  .primary {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
</style>
