<script lang="ts">
  /// Reusable expect/send/wait step list for a macro, used by the sidebar macro
  /// dialog (MacroDialog). `expect` waits for a pattern; typing goes in a
  /// separate `send` step (which carries the vault picker).
  /// `steps` is bindable so the parent owns the array.
  import type { PylonTheme } from '$lib/themes/index'
  import type { LoginStep, VaultEntry } from '$lib/bridge/types'
  import Icon from '$lib/icons/Icon.svelte'
  import { vaultRefStatus } from './vaultRef'

  interface Props {
    theme: PylonTheme
    steps: LoginStep[]
    /** Vault entries available to reference from a `send` step. */
    vaultEntries?: VaultEntry[]
  }

  let { theme, steps = $bindable(), vaultEntries = [] }: Props = $props()

  /// Set a send step to reference a vault entry by NAME + field. We append `\r`
  /// so the value is submitted (the common case: a password prompt). The stored
  /// value stays the placeholder — the secret / username is resolved
  /// backend-side at run time and never enters this editor.
  function insertVaultRef(i: number, name: string, field: 'Password' | 'Username') {
    const step = steps[i]
    if (!step) return
    step.send = `{{vault:${name}.${field}}}\\r`
  }

  export function addStep() {
    steps.push({ kind: 'expect', expect: '', is_regex: false, send: '', timeout_ms: 10000 })
  }
  function removeStep(i: number) {
    steps.splice(i, 1)
  }
  /// Swap a step with its neighbour (dir = -1 up, +1 down). No-op at the ends.
  function moveStep(i: number, dir: -1 | 1) {
    const j = i + dir
    if (j < 0 || j >= steps.length) return
    const a = steps[i]
    const b = steps[j]
    if (!a || !b) return
    steps[i] = b
    steps[j] = a
  }
</script>

<div class="steps">
  {#each steps as step, i (i)}
    <div class="mstep">
      <span class="m-move">
        <button
          type="button"
          class="m-arrow"
          style:color={theme.textDim}
          disabled={i === 0}
          title="Move up"
          aria-label="Move step up"
          onclick={() => moveStep(i, -1)}>▲</button
        >
        <button
          type="button"
          class="m-arrow"
          style:color={theme.textDim}
          disabled={i === steps.length - 1}
          title="Move down"
          aria-label="Move step down"
          onclick={() => moveStep(i, 1)}>▼</button
        >
      </span>
      <select
        class="m-kind"
        bind:value={step.kind}
        title="Step type"
        style:background={theme.bodyBg}
        style:color={theme.textPrimary}
        style:border="1px solid {theme.border}"
      >
        <option value="expect">expect</option>
        <option value="send">send</option>
        <option value="wait">wait</option>
      </select>
      {#if step.kind === 'expect'}
        <input
          class="m-in m-wide"
          placeholder="expect (wait for…)"
          title="expect: wait for this string (type in a separate send step)"
          bind:value={step.expect}
          spellcheck="false"
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        <label class="m-rx" title="Treat as a regular expression" style:color={theme.textDim}>
          <input type="checkbox" bind:checked={step.is_regex} />
          .*
        </label>
        <input
          class="m-ms"
          type="number"
          min={500}
          step={500}
          bind:value={step.timeout_ms}
          title="timeout (ms)"
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
      {:else if step.kind === 'send'}
        {@const vstatus = vaultRefStatus(step.send)}
        {#if vstatus === 'valid'}
          <span class="m-lock" title="Vault reference (secret resolved at run time)">🔒</span>
        {:else if vstatus === 'malformed'}
          <span
            class="m-lock"
            style:color={theme.err}
            title="Invalid vault reference — use Name.Password or Name.Username (pick it from the 🔒 menu)"
            >⚠️</span
          >
        {/if}
        <input
          class="m-in m-wide"
          placeholder={'send (\\r for bare Enter)'}
          bind:value={step.send}
          spellcheck="false"
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        {#if vaultEntries.length > 0}
          <select
            class="m-vault"
            title="Insert vault reference (by name + field)"
            aria-label="Insert vault reference"
            value=""
            onchange={(e) => {
              const sel = e.currentTarget as HTMLSelectElement
              const idx = sel.selectedIndex
              const opt = sel.options[idx]
              const name = opt?.dataset.name
              const field = opt?.dataset.field as 'Password' | 'Username' | undefined
              if (name && field) insertVaultRef(i, name, field)
              sel.value = ''
            }}
            style:background={theme.bodyBg}
            style:color={theme.textPrimary}
            style:border="1px solid {theme.border}"
          >
            <option value="">🔒 vault…</option>
            {#each vaultEntries as v (v.id)}
              <option value={`${v.id}:pw`} data-name={v.name} data-field="Password"
                >{v.name} · password</option
              >
              <option value={`${v.id}:un`} data-name={v.name} data-field="Username"
                >{v.name} · username{v.username ? ` (${v.username})` : ''}</option
              >
            {/each}
          </select>
        {/if}
      {:else}
        <input
          class="m-ms m-waitms"
          type="number"
          min={100}
          step={100}
          bind:value={step.timeout_ms}
          title="pause (ms)"
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        <span class="m-hint" style:color={theme.textDim}>ms pause</span>
      {/if}
      <button
        type="button"
        class="m-rm"
        style:color={theme.textDim}
        title="Remove step"
        onclick={() => removeStep(i)}
      >
        <Icon name="x" size={11} />
      </button>
    </div>
  {/each}
  <button
    type="button"
    class="m-add"
    style:color={theme.textDim}
    style:border="1px solid {theme.border}"
    onclick={addStep}>+ Add step</button
  >
</div>

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .mstep {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .m-move {
    display: inline-flex;
    flex-direction: column;
    flex-shrink: 0;
    line-height: 0.7;
  }
  .m-arrow {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0 2px;
    font-size: 8px;
    line-height: 1;
  }
  .m-arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .m-kind {
    width: 4.6rem;
    flex-shrink: 0;
    border-radius: 5px;
    padding: 4px 4px;
    font-size: 11px;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  .m-in {
    flex: 1;
    min-width: 0;
    border-radius: 5px;
    padding: 4px 6px;
    font-size: 11.5px;
    font-family: var(--zx-font-mono);
    outline: none;
  }
  .m-ms {
    width: 4.2rem;
    flex-shrink: 0;
    border-radius: 5px;
    padding: 4px 4px;
    font-size: 11.5px;
    outline: none;
  }
  .m-hint {
    font-size: 10.5px;
    flex-shrink: 0;
  }
  .m-rx {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    font-family: var(--zx-font-mono);
    font-size: 11px;
    flex-shrink: 0;
    cursor: pointer;
  }
  .m-rx input {
    accent-color: var(--zx-accent);
  }
  .m-lock {
    font-size: 11px;
    flex-shrink: 0;
    line-height: 1;
  }
  .m-vault {
    width: 5.4rem;
    flex-shrink: 0;
    border-radius: 5px;
    padding: 4px 4px;
    font-size: 11px;
    font-family: inherit;
    outline: none;
  }
  .m-rm {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 2px;
    display: inline-flex;
    flex-shrink: 0;
  }
  .m-add {
    align-self: flex-start;
    background: transparent;
    border-radius: 5px;
    padding: 3px 10px;
    font-size: 11.5px;
    font-family: inherit;
    cursor: pointer;
  }
</style>
