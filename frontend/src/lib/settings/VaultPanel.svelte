<script lang="ts">
  import { masterChangePassword, masterLock, masterSetRemember } from '$lib/bridge/commands'
  import { errorText } from '$lib/bridge/errors'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import { passwordProblem } from '$lib/vault/password'
  import { setVaultStatus, vault } from '$lib/vault/vault.svelte'

  let busy = $state(false)
  let changing = $state(false)
  let oldPassword = $state('')
  let newPassword = $state('')
  let repeat = $state('')
  let reveal = $state(false)

  const status = $derived(vault.status)
  const problem = $derived(
    status ? passwordProblem(newPassword, repeat, status.min_password_chars) : '',
  )
  const canChange = $derived(!busy && oldPassword.length > 0 && problem === '')

  function resetForm() {
    changing = false
    oldPassword = ''
    newPassword = ''
    repeat = ''
    reveal = false
  }

  async function toggleRemember(next: boolean) {
    busy = true
    try {
      setVaultStatus(await masterSetRemember(next))
    } catch (e) {
      showToast({ kind: 'error', title: 'Vault', detail: errorText(e) })
    } finally {
      busy = false
    }
  }

  async function changePassword() {
    if (!canChange) return
    busy = true
    try {
      await masterChangePassword(oldPassword, newPassword)
      resetForm()
      showToast({ kind: 'success', title: 'Vault', detail: 'Master password changed.' })
    } catch (e) {
      showToast({ kind: 'error', title: 'Vault', detail: errorText(e) })
    } finally {
      busy = false
    }
  }

  async function lockNow() {
    busy = true
    try {
      // The gate takes over the window as soon as the status says "locked".
      setVaultStatus(await masterLock())
    } catch (e) {
      showToast({ kind: 'error', title: 'Vault', detail: errorText(e) })
    } finally {
      busy = false
    }
  }
</script>

<div class="panel">
  <section>
    <h3>Vault</h3>
    <p class="hint">
      Saved passwords are kept in an encrypted vault (AES-256-GCM). Its key is protected by your
      master password (Argon2id), so a copy of ZapTerm's data is useless without it. The master
      password cannot be recovered: keep an encrypted backup (Backup tab) as a way back.
    </p>
  </section>

  {#if status}
    <section>
      <h3>Opening ZapTerm</h3>
      {#if status.portable}
        <p class="hint">
          Portable copy: the master password is asked every time ZapTerm starts, and nothing is
          stored on the computer it runs on.
        </p>
      {:else}
        <label class="check">
          <input
            type="checkbox"
            checked={status.remember}
            disabled={busy}
            onchange={(e) => toggleRemember(e.currentTarget.checked)}
          />
          <span>
            Remember on this computer — open without asking for the master password. The vault key
            is kept in this computer's keyring, protected by your user account.
          </span>
        </label>
      {/if}
      {#if status.portable || !status.remember}
        <div class="actions">
          <button type="button" class="ghost-btn" disabled={busy} onclick={lockNow}>
            Lock now
          </button>
        </div>
      {/if}
    </section>

    <section>
      <h3>Master password</h3>
      {#if !changing}
        <div class="actions">
          <button type="button" class="ghost-btn" disabled={busy} onclick={() => (changing = true)}>
            Change master password…
          </button>
        </div>
      {:else}
        <form
          class="form"
          onsubmit={(e) => {
            e.preventDefault()
            changePassword()
          }}
        >
          <label class="field">
            <span>Current master password</span>
            <input
              bind:value={oldPassword}
              type={reveal ? 'text' : 'password'}
              autocomplete="off"
              spellcheck="false"
            />
          </label>
          <label class="field">
            <span>New master password (at least {status.min_password_chars} characters)</span>
            <input
              bind:value={newPassword}
              type={reveal ? 'text' : 'password'}
              autocomplete="off"
              spellcheck="false"
            />
          </label>
          <label class="field">
            <span>Repeat new master password</span>
            <input
              bind:value={repeat}
              type={reveal ? 'text' : 'password'}
              autocomplete="off"
              spellcheck="false"
            />
          </label>
          <label class="check">
            <input type="checkbox" bind:checked={reveal} />
            <span>Show passwords</span>
          </label>
          <p class="validation" class:visible={newPassword.length > 0 && problem !== ''}>
            {problem || ' '}
          </p>
          <div class="actions">
            <button type="submit" class="ok-btn" disabled={!canChange}>Change</button>
            <button type="button" class="ghost-btn" disabled={busy} onclick={resetForm}>
              Cancel
            </button>
          </div>
        </form>
      {/if}
    </section>
  {/if}
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
  section,
  .form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .form {
    max-width: 26rem;
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
  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.72rem;
    color: var(--zx-text-muted);
  }
  .field input {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-2);
    color: var(--zx-text);
    font-size: 0.85rem;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  .field input:focus-visible {
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
  .validation {
    margin: 0;
    font-size: 0.7rem;
    color: var(--zx-warn);
    min-height: 1em;
    visibility: hidden;
  }
  .validation.visible {
    visibility: visible;
  }
  .ok-btn {
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
