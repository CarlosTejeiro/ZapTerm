<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'

  interface Props {
    title: string
    /** Short explanation shown above the field. */
    message?: string
    /** Ask for the passphrase twice (creating a backup). */
    confirm?: boolean
    /** Minimum accepted length; 0 disables the check (opening a backup). */
    minLength?: number
    submitLabel?: string
    onSubmit: (passphrase: string) => void
    onCancel: () => void
  }

  let {
    title,
    message = '',
    confirm = false,
    minLength = 0,
    submitLabel = 'OK',
    onSubmit,
    onCancel,
  }: Props = $props()

  let value = $state('')
  let repeat = $state('')
  let reveal = $state(false)
  let inputEl = $state<HTMLInputElement | null>(null)

  $effect(() => {
    inputEl?.focus()
  })

  const tooShort = $derived(minLength > 0 && value.length > 0 && value.length < minLength)
  const mismatch = $derived(confirm && repeat.length > 0 && repeat !== value)
  const canSubmit = $derived(
    value.length > 0 &&
      (minLength === 0 || value.length >= minLength) &&
      (!confirm || repeat === value),
  )

  function submit() {
    if (!canSubmit) return
    onSubmit(value)
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel()
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  tabindex="-1"
  onkeydown={onKey}
  onclick={(e) => {
    if (e.target === e.currentTarget) onCancel()
  }}
  transition:fade={{ duration: 120 }}
>
  <form
    class="dialog"
    onsubmit={(e) => {
      e.preventDefault()
      submit()
    }}
    transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}
  >
    <h3>{title}</h3>
    {#if message}
      <p class="message">{message}</p>
    {/if}
    <label class="field">
      <span>Passphrase</span>
      <div class="input-row">
        <input
          bind:this={inputEl}
          bind:value
          type={reveal ? 'text' : 'password'}
          autocomplete="off"
          spellcheck="false"
        />
        <button
          type="button"
          class="reveal"
          onclick={() => (reveal = !reveal)}
          aria-label={reveal ? 'Hide passphrase' : 'Show passphrase'}
        >
          {reveal ? 'Hide' : 'Show'}
        </button>
      </div>
    </label>
    {#if confirm}
      <label class="field">
        <span>Repeat passphrase</span>
        <input
          bind:value={repeat}
          type={reveal ? 'text' : 'password'}
          autocomplete="off"
          spellcheck="false"
        />
      </label>
    {/if}
    <p class="validation" class:visible={tooShort || mismatch}>
      {#if tooShort}
        At least {minLength} characters.
      {:else if mismatch}
        The passphrases don't match.
      {:else}
        &nbsp;
      {/if}
    </p>
    <div class="actions">
      <button type="button" class="cancel" onclick={onCancel}>Cancel</button>
      <button type="submit" class="ok" disabled={!canSubmit}>{submitLabel}</button>
    </div>
  </form>
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
    width: 26rem;
    max-width: 92vw;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    padding: var(--zx-space-4) var(--zx-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-2);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
  }
  h3 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
  }
  .message {
    margin: 0;
    font-size: 0.74rem;
    line-height: 1.5;
    color: var(--zx-text-muted);
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
  input {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-2);
    color: var(--zx-text);
    font-size: 0.85rem;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  input:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
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
    font-size: 0.7rem;
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
    gap: var(--zx-space-1);
    margin-top: 0.1rem;
  }
  .actions button {
    border: 0;
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-3);
    font-size: 0.8rem;
    font-family: inherit;
    cursor: pointer;
  }
  .cancel {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .cancel:hover {
    background: color-mix(in srgb, var(--zx-text) 12%, transparent);
  }
  .ok {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
  .ok:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .ok:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
