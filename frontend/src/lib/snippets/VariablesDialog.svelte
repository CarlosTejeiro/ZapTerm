<script lang="ts">
  /// Collects values for a snippet's `{{variables}}` before it runs. Mounted
  /// once in App.svelte; renders whenever `variablesRequest.current` is set.
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import { variablesRequest, commitVariables } from './variables.svelte'

  const req = $derived(variablesRequest.current)

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') commitVariables(true)
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) commitVariables(false)
  }
</script>

{#if req}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Snippet variables"
    tabindex="-1"
    onkeydown={onKeydown}
    transition:fade={{ duration: 120 }}
  >
    <div class="dialog" transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}>
      <h2>Fill in variables</h2>
      <div class="fields">
        {#each req.vars as v, i (v.name)}
          <label>
            <span class="vname">{v.name}</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              bind:value={req.values[v.name]}
              placeholder={v.def || v.name}
              autofocus={i === 0}
              spellcheck="false"
            />
          </label>
        {/each}
      </div>
      <div class="actions">
        <button type="button" class="cancel" onclick={() => commitVariables(true)}>Cancel</button>
        <button type="button" class="ok" onclick={() => commitVariables(false)}>Send</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    z-index: 320;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem 1rem;
  }
  .dialog {
    width: 22rem;
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
  h2 {
    margin: 0 0 0.75rem;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .vname {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    font-family: var(--zx-font-mono);
  }
  input {
    background: var(--zx-body-bg);
    color: var(--zx-text);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: 0.4rem 0.55rem;
    font-size: 0.85rem;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  input:focus {
    border-color: var(--zx-accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  button {
    font-size: 0.82rem;
    font-family: inherit;
    border-radius: 5px;
    padding: 0.4rem 0.9rem;
    cursor: pointer;
    border: 1px solid var(--zx-border);
  }
  .cancel {
    background: transparent;
    color: var(--zx-text-muted);
  }
  .cancel:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .ok {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
  .ok:hover {
    filter: brightness(1.1);
  }
</style>
