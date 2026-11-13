<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'

  interface Props {
    title: string
    placeholder?: string
    initial?: string
    submitLabel?: string
    onSubmit: (value: string) => void
    onCancel: () => void
  }

  let {
    title,
    placeholder = '',
    initial = '',
    submitLabel = 'OK',
    onSubmit,
    onCancel,
  }: Props = $props()

  // Snapshot the prop into local state — the dialog is one-shot; if the
  // caller wants to seed a fresh value they remount.
  // svelte-ignore state_referenced_locally
  let value = $state(initial)
  let inputEl = $state<HTMLInputElement | null>(null)

  $effect(() => {
    if (inputEl) {
      inputEl.focus()
      inputEl.select()
    }
  })

  function submit() {
    const v = value.trim()
    if (!v) return
    onSubmit(v)
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
    <input
      bind:this={inputEl}
      bind:value
      type="text"
      {placeholder}
      autocomplete="off"
      spellcheck="false"
    />
    <div class="actions">
      <button type="button" class="cancel" onclick={onCancel}>Cancel</button>
      <button type="submit" class="ok" disabled={!value.trim()}>{submitLabel}</button>
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
    width: 24rem;
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
  input {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-2);
    color: var(--zx-text);
    font-size: 0.85rem;
    font-family: inherit;
    outline: none;
  }
  input:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
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
