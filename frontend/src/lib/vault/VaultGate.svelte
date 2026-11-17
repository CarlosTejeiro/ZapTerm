<script lang="ts">
  // Covers the app until the vault is open: the first time to choose the
  // master password, afterwards to type it (never when the vault is
  // remembered on this device — the backend opens it by itself).
  import { onMount } from 'svelte'
  import { masterCreate, masterUnlock } from '$lib/bridge/commands'
  import { errorText } from '$lib/bridge/errors'
  import { passwordProblem } from './password'
  import { refreshVault, setVaultStatus, vault } from './vault.svelte'

  let password = $state('')
  let repeat = $state('')
  let remember = $state(true)
  let reveal = $state(false)
  let busy = $state(false)
  let error = $state('')
  let loadError = $state('')
  let inputEl = $state<HTMLInputElement | null>(null)

  const status = $derived(vault.status)
  const mode = $derived(
    !status ? null : !status.exists ? 'create' : !status.unlocked ? 'unlock' : null,
  )
  const problem = $derived(
    mode === 'create' && status ? passwordProblem(password, repeat, status.min_password_chars) : '',
  )
  const canSubmit = $derived(!busy && password.length > 0 && (mode !== 'create' || problem === ''))

  onMount(() => {
    refreshVault().catch((e) => (loadError = errorText(e)))
  })

  $effect(() => {
    if (mode) inputEl?.focus()
  })

  async function submit() {
    if (!canSubmit || !mode) return
    busy = true
    error = ''
    try {
      const next =
        mode === 'create'
          ? await masterCreate(password, remember && !status?.portable)
          : await masterUnlock(password)
      setVaultStatus(next)
      password = ''
      repeat = ''
      reveal = false
    } catch (e) {
      const text = errorText(e)
      error = text.charAt(0).toUpperCase() + text.slice(1)
      // A wrong password is retyped from scratch.
      if (mode === 'unlock') password = ''
      inputEl?.focus()
    } finally {
      busy = false
    }
  }
</script>

{#if loadError}
  <div class="overlay" role="alertdialog" aria-modal="true" aria-label="Vault unavailable">
    <div class="dialog">
      <h2>The vault could not be read</h2>
      <p class="message">{loadError}</p>
    </div>
  </div>
{:else if !status}
  <!-- Until the vault's state is known, nothing behind is reachable. -->
  <div class="overlay" aria-hidden="true"></div>
{:else if mode}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-label={mode === 'create' ? 'Create master password' : 'Unlock vault'}
  >
    <form
      class="dialog"
      onsubmit={(e) => {
        e.preventDefault()
        submit()
      }}
    >
      {#if mode === 'create'}
        <h2>Create your master password</h2>
        <p class="message">
          ZapTerm keeps every saved password in an encrypted vault. The master password is the only
          thing that opens it: choose a long one you will remember, such as a few unrelated words.
        </p>
        <p class="message warn">
          It cannot be recovered. If you forget it, the saved passwords are lost unless you have an
          encrypted backup.
        </p>
      {:else}
        <h2>Unlock ZapTerm</h2>
        <p class="message">Type your master password to open the vault.</p>
      {/if}

      <label class="field">
        <span>Master password</span>
        <div class="input-row">
          <input
            bind:this={inputEl}
            bind:value={password}
            type={reveal ? 'text' : 'password'}
            autocomplete="off"
            spellcheck="false"
            disabled={busy}
          />
          <button
            type="button"
            class="reveal"
            onclick={() => (reveal = !reveal)}
            aria-label={reveal ? 'Hide password' : 'Show password'}
          >
            {reveal ? 'Hide' : 'Show'}
          </button>
        </div>
      </label>

      {#if mode === 'create'}
        <label class="field">
          <span>Repeat master password</span>
          <input
            bind:value={repeat}
            type={reveal ? 'text' : 'password'}
            autocomplete="off"
            spellcheck="false"
            disabled={busy}
          />
        </label>
        {#if status?.portable}
          <p class="message">
            This is a portable copy: it asks for the master password every time it starts and stores
            nothing on the computer it runs on.
          </p>
        {:else}
          <label class="check">
            <input type="checkbox" bind:checked={remember} disabled={busy} />
            <span>
              Remember on this computer — ZapTerm opens without asking. The vault key is kept in
              this computer's keyring, protected by your user account.
            </span>
          </label>
        {/if}
      {/if}

      <p class="validation" class:visible={error !== '' || (password.length > 0 && problem !== '')}>
        {error || problem || ' '}
      </p>

      <div class="actions">
        <button type="submit" class="ok" disabled={!canSubmit}>
          {busy ? 'Working…' : mode === 'create' ? 'Create vault' : 'Unlock'}
        </button>
      </div>
    </form>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--zx-app-bg);
    display: flex;
    align-items: center;
    justify-content: center;
    /* Above every other dialog: nothing is usable until the vault is open. */
    z-index: 5000;
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
    box-shadow: var(--zx-shadow);
  }
  h2 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }
  .message {
    margin: 0;
    font-size: 0.76rem;
    line-height: 1.5;
    color: var(--zx-text-muted);
  }
  .message.warn {
    padding: 0.45rem 0.6rem;
    border: 1px solid color-mix(in srgb, var(--zx-warn) 45%, transparent);
    background: color-mix(in srgb, var(--zx-warn) 10%, transparent);
    border-radius: var(--zx-radius);
    color: var(--zx-text);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.72rem;
    color: var(--zx-text-muted);
  }
  .input-row {
    display: flex;
    gap: 0.35rem;
  }
  .input-row input {
    flex: 1;
    min-width: 0;
  }
  input:not([type='checkbox']) {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-2);
    color: var(--zx-text);
    font-size: 0.85rem;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  input:not([type='checkbox']):focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }
  .check {
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
    font-size: 0.74rem;
    line-height: 1.45;
    color: var(--zx-text-muted);
  }
  .check input {
    margin-top: 0.15rem;
  }
  .reveal {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: 0 var(--zx-space-2);
    font-size: 0.72rem;
    font-family: inherit;
    cursor: pointer;
  }
  .reveal:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .validation {
    margin: 0;
    font-size: 0.72rem;
    color: var(--zx-warn);
    min-height: 1em;
    visibility: hidden;
  }
  .validation.visible {
    visibility: visible;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  .ok {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    border: 0;
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-3);
    font-size: 0.8rem;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .ok:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .ok:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
