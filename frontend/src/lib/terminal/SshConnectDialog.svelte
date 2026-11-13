<script lang="ts">
  import type { AuthMethod } from '../bridge/types'

  interface Props {
    onConnect: (params: SshParams) => void
    onCancel: () => void
  }

  export interface SshParams {
    host: string
    port: number
    user: string
    auth: AuthMethod
  }

  let { onConnect, onCancel }: Props = $props()

  let host = $state('')
  let port = $state(22)
  let user = $state('')
  let authType = $state<'password' | 'key' | 'keyboard-interactive' | 'agent'>('password')
  let password = $state('')
  let keyPath = $state('')
  let passphrase = $state('')
  let error = $state('')

  function buildAuth(): AuthMethod | null {
    if (authType === 'password') return { type: 'password', password }
    if (authType === 'agent') return { type: 'agent' }
    if (authType === 'keyboard-interactive') return { type: 'keyboard-interactive' }
    if (!keyPath.trim()) {
      error = 'Key file path is required'
      return null
    }
    return { type: 'key', keyPath: keyPath.trim(), passphrase: passphrase || null }
  }

  function submit() {
    error = ''
    if (!host.trim()) {
      error = 'Host is required'
      return
    }
    if (!user.trim()) {
      error = 'Username is required'
      return
    }
    const auth = buildAuth()
    if (!auth) return
    onConnect({ host: host.trim(), port, user: user.trim(), auth })
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') submit()
    if (e.key === 'Escape') onCancel()
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog">
    <h2 class="title">New SSH session</h2>

    <label class="field">
      <span>Host</span>
      <input bind:value={host} placeholder="192.168.1.1" autocomplete="off" spellcheck="false" />
    </label>

    <div class="row">
      <label class="field" style="flex:3">
        <span>Username</span>
        <input bind:value={user} placeholder="admin" autocomplete="off" spellcheck="false" />
      </label>
      <label class="field" style="flex:1">
        <span>Port</span>
        <input type="number" bind:value={port} min="1" max="65535" />
      </label>
    </div>

    <label class="field">
      <span>Authentication</span>
      <select bind:value={authType}>
        <option value="password">Password</option>
        <option value="key">Private key file</option>
        <option value="keyboard-interactive">Keyboard-interactive (2FA / OTP)</option>
        <option value="agent">SSH agent</option>
      </select>
    </label>

    {#if authType === 'password'}
      <label class="field">
        <span>Password</span>
        <input type="password" bind:value={password} autocomplete="off" />
      </label>
    {:else if authType === 'key'}
      <label class="field">
        <span>Key file</span>
        <input
          bind:value={keyPath}
          placeholder="~/.ssh/id_ed25519"
          autocomplete="off"
          spellcheck="false"
        />
      </label>
      <label class="field">
        <span>Passphrase (if any)</span>
        <input type="password" bind:value={passphrase} autocomplete="off" />
      </label>
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="actions">
      <button class="btn-cancel" onclick={onCancel}>Cancel</button>
      <button class="btn-connect" onclick={submit}>Connect</button>
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
    z-index: 100;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 0.5rem;
    padding: 1.5rem;
    width: 380px;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
  }

  .title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0 0 0.25rem;
  }

  .row {
    display: flex;
    gap: 0.5rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    flex: 1;
  }

  .field span {
    font-size: 0.75rem;
    color: var(--zx-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .field input,
  .field select {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.875rem;
    padding: 0.4rem 0.6rem;
    outline: none;
    font-family: inherit;
    width: 100%;
    box-sizing: border-box;
  }

  .field input:focus-visible,
  .field select:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
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

  .btn-cancel,
  .btn-connect {
    padding: 0.35rem 0.9rem;
    font-size: 0.8rem;
    border-radius: 0.25rem;
    border: none;
    cursor: pointer;
    font-family: inherit;
  }

  .btn-cancel {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
  }

  .btn-cancel:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }

  .btn-connect {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
  }

  .btn-connect:hover {
    filter: brightness(1.1);
  }
</style>
