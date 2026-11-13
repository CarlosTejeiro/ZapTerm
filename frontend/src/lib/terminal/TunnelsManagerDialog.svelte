<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  /// Global tunnels manager: every active port-forward across all sessions,
  /// with a stop button each. Add new forwards from a pane's Tunnels button.
  import { onMount } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon from '$lib/icons/Icon.svelte'
  import { listAllForwards, removeForward, type SessionForwardInfo } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'

  interface Props {
    /// Maps a live session UUID to a human label (pane/session name).
    labels: Record<string, string>
    onClose: () => void
  }

  const { labels, onClose }: Props = $props()

  let forwards = $state<SessionForwardInfo[]>([])
  let loading = $state(true)

  async function refresh() {
    loading = true
    try {
      forwards = await listAllForwards()
    } catch (e) {
      showToast({ kind: 'error', title: 'Tunnels', detail: errorText(e) })
    } finally {
      loading = false
    }
  }

  onMount(refresh)

  function kindLabel(k: string): string {
    return k === 'local'
      ? '-L local'
      : k === 'dynamic'
        ? '-D dynamic'
        : k === 'remote'
          ? '-R remote'
          : k
  }

  function route(f: SessionForwardInfo): string {
    const i = f.info
    const bind = `${i.bind_addr}:${i.bind_port}`
    if (i.kind === 'dynamic') return `${bind} · SOCKS5`
    return `${bind} → ${i.target_host ?? '?'}:${i.target_port ?? '?'}`
  }

  async function stop(f: SessionForwardInfo) {
    try {
      await removeForward(f.session_id, f.info.id)
      await refresh()
    } catch (e) {
      showToast({ kind: 'error', title: 'Tunnels', detail: errorText(e) })
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Active tunnels"
  tabindex="-1"
  onkeydown={onKeydown}
  transition:fade={{ duration: 140 }}
>
  <div class="dialog" transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}>
    <div class="header">
      <h2>Active tunnels</h2>
      <div class="header-right">
        <button class="btn" onclick={refresh} title="Refresh"
          ><Icon name="refresh" size={12} /></button
        >
        <button class="btn" onclick={onClose} title="Close"><Icon name="x" size={12} /></button>
      </div>
    </div>

    {#if loading}
      <p class="muted">Loading…</p>
    {:else if forwards.length === 0}
      <p class="muted">
        No active tunnels. Open a session's <strong>Tunnels</strong> button (in the pane header) to add
        a forward, or save them on the session so they auto-start on connect.
      </p>
    {:else}
      <ul class="list">
        {#each forwards as f (f.session_id + f.info.id)}
          <li>
            <span
              class="kind"
              class:dyn={f.info.kind === 'dynamic'}
              class:remote={f.info.kind === 'remote'}
            >
              {kindLabel(f.info.kind)}
            </span>
            <div class="info">
              {#if f.info.name}<span class="name">{f.info.name}</span>{/if}
              <span class="route">{route(f)}</span>
              <span class="session">{labels[f.session_id] ?? f.session_id.slice(0, 8)}</span>
            </div>
            <button class="rm" onclick={() => stop(f)} title="Stop tunnel">
              <Icon name="x" size={12} />
            </button>
          </li>
        {/each}
      </ul>
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
    z-index: 300;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem 1rem;
  }
  .dialog {
    width: 30rem;
    max-width: 95vw;
    max-height: 100%;
    overflow-y: auto;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    box-shadow: var(--zx-shadow);
    padding: 1.25rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }
  h2 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .header-right {
    display: flex;
    gap: 0.4rem;
  }
  .muted {
    color: var(--zx-text-muted);
    font-size: 0.8rem;
    line-height: 1.6;
    margin: 0.5rem 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    background: var(--zx-body-bg);
  }
  .kind {
    font-size: 0.68rem;
    font-family: var(--zx-font-mono);
    color: var(--zx-accent);
    flex-shrink: 0;
    width: 5.5rem;
  }
  .kind.dyn {
    color: var(--zx-accent-2);
  }
  .kind.remote {
    color: var(--zx-warn);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .route {
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session {
    font-size: 0.68rem;
    color: var(--zx-text-dim);
  }
  .name {
    font-size: 0.8rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn {
    background: transparent;
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    color: var(--zx-text-muted);
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    font-size: 0.85rem;
    display: inline-flex;
    align-items: center;
  }
  .btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .rm {
    background: transparent;
    border: none;
    color: var(--zx-text-dim);
    cursor: pointer;
    padding: 0.2rem 0.35rem;
    border-radius: 4px;
    flex-shrink: 0;
    display: inline-flex;
  }
  .rm:hover {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 12%, transparent);
  }
</style>
