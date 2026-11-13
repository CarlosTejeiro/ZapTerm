<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import {
    addLocalForward,
    addDynamicForward,
    addRemoteForward,
    listForwards,
    removeForward,
  } from '$lib/bridge/commands'
  import type { ForwardInfo } from '$lib/bridge/types'

  interface Props {
    sessionId: string
    onClose: () => void
  }

  let { sessionId, onClose }: Props = $props()

  let forwards = $state<ForwardInfo[]>([])
  let error = $state('')
  let busy = $state(false)

  let kind = $state<'local' | 'dynamic' | 'remote'>('local')
  let bindAddr = $state('127.0.0.1')
  let bindPort = $state<number | null>(null)
  let targetHost = $state('')
  let targetPort = $state<number | null>(null)
  let name = $state('')

  onMount(refresh)

  async function refresh() {
    try {
      forwards = await listForwards(sessionId)
    } catch {
      forwards = []
    }
  }

  async function add() {
    error = ''
    if (!bindPort || bindPort < 1 || bindPort > 65535) {
      error = 'Bind port must be 1–65535.'
      return
    }
    if ((kind === 'local' || kind === 'remote') && (!targetHost.trim() || !targetPort)) {
      error = `Target host and port are required for ${kind} forward.`
      return
    }
    busy = true
    try {
      if (kind === 'local') {
        await addLocalForward(
          sessionId,
          bindAddr.trim(),
          bindPort,
          targetHost.trim(),
          targetPort!,
          name.trim(),
        )
      } else if (kind === 'remote') {
        // For -R, `bindAddr` is the address the SSH server will listen on.
        // "0.0.0.0" exposes it on every interface of the remote host;
        // "127.0.0.1" keeps it loopback-only.
        await addRemoteForward(
          sessionId,
          bindAddr.trim(),
          bindPort,
          targetHost.trim(),
          targetPort!,
          name.trim(),
        )
      } else {
        await addDynamicForward(sessionId, bindAddr.trim(), bindPort, name.trim())
      }
      bindPort = null
      targetHost = ''
      targetPort = null
      name = ''
      await refresh()
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }

  async function remove(id: string) {
    try {
      await removeForward(sessionId, id)
      await refresh()
    } catch (e) {
      error = errorText(e)
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog">
    <h2>Port forwards</h2>

    {#if forwards.length === 0}
      <p class="muted">No active forwards.</p>
    {:else}
      <ul class="list">
        {#each forwards as f (f.id)}
          <li>
            <span class="kind" class:dyn={f.kind === 'dynamic'} class:remote={f.kind === 'remote'}>
              {f.kind === 'local' ? 'L' : f.kind === 'remote' ? 'R' : 'D'}
            </span>
            <span class="fwd-info">
              {#if f.name}<span class="name" title={f.name}>{f.name}</span>{/if}
              <span class="route">
                <span class="bind">{f.bind_addr}:{f.bind_port}</span>
                <span class="arrow">{f.kind === 'remote' ? '←' : '→'}</span>
                {#if f.target_host}
                  <span class="target">{f.target_host}:{f.target_port}</span>
                {:else}
                  <span class="target socks">SOCKS5</span>
                {/if}
              </span>
            </span>
            <button class="rm" onclick={() => remove(f.id)} title="Remove forward"
              ><Icon name="x" size={12} /></button
            >
          </li>
        {/each}
      </ul>
    {/if}

    <form
      class="add"
      onsubmit={(e) => {
        e.preventDefault()
        add()
      }}
    >
      <h3>Add forward</h3>

      <label>
        Type
        <select bind:value={kind}>
          <option value="local">Local (-L): local port → remote target</option>
          <option value="remote">Remote (-R): remote port → local target</option>
          <option value="dynamic">Dynamic SOCKS5 (-D)</option>
        </select>
      </label>

      <label>
        <span>Name <span class="opt">(optional)</span></span>
        <input bind:value={name} maxlength="80" placeholder="e.g. FortiGate GUI" />
      </label>

      {#if kind === 'remote'}
        <p class="hint">
          The SSH server listens on <code>{bindAddr || '?'}:{bindPort ?? '?'}</code>; every inbound
          connection there is tunnelled to <code>{targetHost || '?'}:{targetPort ?? '?'}</code> on
          this machine. Set bind address to <code>0.0.0.0</code> to expose on every interface of the remote
          host (server's GatewayPorts must allow it).
        </p>
      {/if}

      <div class="row">
        <label class="grow">
          {kind === 'remote' ? 'Remote bind address' : 'Bind address'}
          <input
            bind:value={bindAddr}
            placeholder={kind === 'remote' ? '127.0.0.1 or 0.0.0.0' : '127.0.0.1'}
          />
        </label>
        <label class="port">
          {kind === 'remote' ? 'Remote bind port' : 'Bind port'}
          <input type="number" bind:value={bindPort} min={0} max={65535} placeholder="8080" />
        </label>
      </div>

      {#if kind === 'local' || kind === 'remote'}
        <div class="row">
          <label class="grow">
            {kind === 'remote' ? 'Local target host' : 'Target host'}
            <input
              bind:value={targetHost}
              placeholder={kind === 'remote' ? 'localhost' : 'db.internal'}
            />
          </label>
          <label class="port">
            {kind === 'remote' ? 'Local target port' : 'Target port'}
            <input type="number" bind:value={targetPort} min={1} max={65535} placeholder="5432" />
          </label>
        </div>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <div class="actions">
        <button type="button" class="cancel" onclick={onClose}>Close</button>
        <button type="submit" class="add-btn" disabled={busy}>
          {busy ? 'Adding…' : 'Add forward'}
        </button>
      </div>
    </form>
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
    z-index: 110;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    color: var(--zx-text);
    box-shadow: var(--zx-shadow);
    font-family: var(--zx-font-ui);
    padding: 1.5rem;
    width: 30rem;
    max-width: 95vw;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--zx-text);
  }

  h3 {
    margin: 0 0 0.5rem;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--zx-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .muted {
    color: var(--zx-text-muted);
    font-size: 0.82rem;
    margin: 0;
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.4rem 0.6rem;
    font-size: 0.82rem;
    color: var(--zx-text);
    font-family: var(--zx-font-mono);
  }

  .kind {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 700;
    border-radius: 0.2rem;
    padding: 0.05rem 0.35rem;
    font-size: 0.7rem;
  }

  .kind.dyn {
    background: var(--zx-accent-2);
  }

  .kind.remote {
    background: var(--zx-warn);
  }

  .hint {
    margin: 0;
    font-size: 0.72rem;
    line-height: 1.45;
    color: var(--zx-text-muted);
    background: color-mix(in srgb, var(--zx-warn) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--zx-warn) 25%, transparent);
    border-radius: 0.25rem;
    padding: 0.45rem 0.6rem;
  }

  .hint code {
    background: var(--zx-surface-2);
    border-radius: 0.18rem;
    padding: 0 0.25rem;
    font-size: 0.7rem;
    color: var(--zx-warn);
  }

  .bind,
  .target {
    color: var(--zx-text);
  }

  .fwd-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .route {
    display: flex;
    gap: 0.5rem;
    min-width: 0;
  }

  .name {
    font-family: var(--zx-font-ui);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .opt {
    color: var(--zx-text-dim);
    font-weight: 400;
  }

  .target.socks {
    color: var(--zx-accent-2);
  }

  .arrow {
    color: var(--zx-text-dim);
  }

  .rm {
    margin-left: auto;
    background: transparent;
    color: var(--zx-text-dim);
    border: none;
    cursor: pointer;
    font-size: 0.9rem;
    line-height: 1;
    padding: 0.15rem 0.4rem;
    border-radius: 0.2rem;
  }

  .rm:hover {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    border-top: 1px solid var(--zx-border);
    padding-top: 0.85rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--zx-text-muted);
  }

  input,
  select {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.85rem;
    padding: 0.35rem 0.5rem;
    outline: none;
    font-family: inherit;
  }

  input:focus-visible,
  select:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }

  .row {
    display: flex;
    gap: 0.5rem;
  }

  .grow {
    flex: 1;
  }

  .port {
    width: 7rem;
    flex-shrink: 0;
  }

  .error {
    font-size: 0.8rem;
    color: var(--zx-err);
    margin: 0;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
  }

  button.cancel,
  button.add-btn {
    font-size: 0.82rem;
    font-family: inherit;
    padding: 0.35rem 0.9rem;
    border-radius: 0.25rem;
    border: none;
    cursor: pointer;
  }

  .cancel {
    background: var(--zx-surface-2);
    color: var(--zx-text-muted);
  }

  .cancel:hover {
    background: var(--zx-hover-bg);
  }

  .add-btn {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
  }

  .add-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .add-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
