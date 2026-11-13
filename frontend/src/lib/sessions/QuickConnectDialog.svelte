<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon from '$lib/icons/Icon.svelte'

  export type ConnectParams =
    | { type: 'local' }
    | { type: 'ssh'; host: string; port: number; user: string; password: string }
    | { type: 'telnet'; host: string; port: number }

  interface Props {
    onConnect: (params: ConnectParams) => void
    onCancel: () => void
  }

  let { onConnect, onCancel }: Props = $props()

  let protocol = $state<'ssh' | 'telnet' | 'local'>('ssh')
  let host = $state('')
  let port = $state(22)
  let user = $state('')
  let password = $state('')
  let error = $state<string | null>(null)

  function setProtocol(p: 'ssh' | 'telnet' | 'local') {
    protocol = p
    error = null
    if (p === 'ssh') port = 22
    if (p === 'telnet') port = 23
  }

  function handleConnect() {
    error = null
    if (protocol === 'local') {
      onConnect({ type: 'local' })
      return
    }
    if (!host.trim()) {
      error = 'Host is required'
      return
    }
    // An emptied number field binds as null; anything out of range would only
    // fail later as an obscure deserialisation error from the backend.
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      error = 'Port must be a number from 1 to 65535'
      return
    }
    if (protocol === 'ssh') {
      if (!user.trim()) {
        error = 'Username is required'
        return
      }
      onConnect({ type: 'ssh', host: host.trim(), port, user: user.trim(), password })
    } else {
      onConnect({ type: 'telnet', host: host.trim(), port })
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel()
    if (e.key === 'Enter') {
      e.preventDefault()
      handleConnect()
    }
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Quick Connect"
  tabindex="-1"
  onkeydown={handleKeydown}
  transition:fade={{ duration: 140 }}
>
  <div class="dialog" transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}>
    <div class="dialog-header">
      <h2 class="dialog-title">Quick Connect</h2>
      <button class="close-btn" onclick={onCancel} aria-label="Close"
        ><Icon name="x" size={13} /></button
      >
    </div>

    <div class="dialog-body">
      <div class="protocol-tabs">
        {#each ['ssh', 'telnet', 'local'] as const as p}
          <button class="protocol-tab" class:active={protocol === p} onclick={() => setProtocol(p)}>
            {p === 'local' ? 'Local Shell' : p.toUpperCase()}
          </button>
        {/each}
      </div>

      {#if protocol !== 'local'}
        <div class="field">
          <label for="qc-host">Host</label>
          <input id="qc-host" class="input" bind:value={host} placeholder="192.168.1.1" />
        </div>
        <div class="field">
          <label for="qc-port">Port</label>
          <input
            id="qc-port"
            class="input input-sm"
            type="number"
            bind:value={port}
            min="1"
            max="65535"
          />
        </div>
      {/if}

      {#if protocol === 'ssh'}
        <div class="field">
          <label for="qc-user">Username</label>
          <input id="qc-user" class="input" bind:value={user} placeholder="admin" />
        </div>
        <div class="field">
          <label for="qc-pass">Password</label>
          <input
            id="qc-pass"
            class="input"
            type="password"
            bind:value={password}
            placeholder="(optional)"
          />
        </div>
      {/if}

      {#if protocol === 'local'}
        <p class="local-hint">Opens a local shell session without saving to the session list.</p>
      {/if}

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}
    </div>

    <div class="dialog-footer">
      <button class="btn-secondary" onclick={onCancel}>Cancel</button>
      <button class="btn-primary" onclick={handleConnect}>Connect</button>
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
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    width: 22rem;
    display: flex;
    flex-direction: column;
    box-shadow: var(--zx-shadow);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--zx-border);
  }

  .dialog-title {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
  }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--zx-text-muted);
    font-size: 1rem;
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-family: inherit;
  }
  .close-btn:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }

  .dialog-body {
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .protocol-tabs {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 0.25rem;
  }

  .protocol-tab {
    flex: 1;
    padding: 0.25rem 0.4rem;
    font-size: 0.72rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    cursor: pointer;
    color: var(--zx-text-muted);
    font-family: inherit;
    transition: background 0.1s;
  }
  .protocol-tab.active {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
  }
  .protocol-tab:hover:not(.active) {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  label {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
  }

  .input {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.3rem 0.5rem;
    color: var(--zx-text);
    font-size: 0.8rem;
    outline: none;
    font-family: inherit;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }
  .input-sm {
    max-width: 6rem;
    width: 6rem;
  }

  .local-hint {
    font-size: 0.78rem;
    color: var(--zx-text-muted);
    margin: 0.25rem 0;
    line-height: 1.4;
  }

  .error-msg {
    font-size: 0.75rem;
    color: var(--zx-err);
    margin: 0;
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-top: 1px solid var(--zx-border);
  }

  .btn-secondary {
    background: transparent;
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.3rem 0.75rem;
    color: var(--zx-text-muted);
    font-size: 0.8rem;
    cursor: pointer;
    font-family: inherit;
  }
  .btn-secondary:hover {
    color: var(--zx-text);
    border-color: var(--zx-text-muted);
  }

  .btn-primary {
    background: var(--zx-accent);
    border: none;
    border-radius: 0.25rem;
    padding: 0.3rem 0.75rem;
    color: var(--zx-on-accent);
    font-size: 0.8rem;
    cursor: pointer;
    font-family: inherit;
    font-weight: 500;
  }
  .btn-primary:hover {
    background: color-mix(in srgb, var(--zx-accent) 85%, black);
  }
</style>
