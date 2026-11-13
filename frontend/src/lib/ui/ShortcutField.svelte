<script lang="ts">
  /// Records a keyboard shortcut for a snippet or macro. Click "Set", press the
  /// combo; it is validated (needs Ctrl/Alt/Cmd, not a terminal control key,
  /// not the reserved Ctrl+Shift+1..9 row) and checked against app shortcuts
  /// and other snippets before it is accepted. Escape cancels recording.
  import type { PylonTheme } from '$lib/themes/index'
  import { bindings } from '$lib/stores/keybindings.svelte'
  import { snippets } from '$lib/stores/snippets.svelte'
  import { eventToCombo, shortcutClash, snippetComboProblem } from '$lib/stores/shortcuts'

  interface Props {
    theme: PylonTheme
    /** Current combo, or null for none. */
    value: string | null
    /** Id of the snippet being edited (null when creating) — excluded from
     *  the clash check so it can keep its own combo. */
    selfId: number | null
    onChange: (v: string | null) => void
  }

  const { theme, value, selfId, onChange }: Props = $props()

  let recording = $state(false)
  let error = $state<string | null>(null)

  function onKeydown(e: KeyboardEvent) {
    if (!recording) return
    // Keep the press away from the editor (Escape would close it) and from
    // App's global shortcut listener.
    e.preventDefault()
    e.stopPropagation()
    if (e.key === 'Escape') {
      recording = false
      error = null
      return
    }
    const combo = eventToCombo(e)
    if (combo === null) return // modifiers still being held
    const problem = snippetComboProblem(e) ?? shortcutClash(combo, bindings, snippets, selfId)
    if (problem) {
      error = `${combo}: ${problem}`
      return
    }
    error = null
    recording = false
    onChange(combo)
  }
</script>

<div class="field">
  <span class="label" style:color={theme.textMuted}>Shortcut</span>
  <button
    type="button"
    class="combo"
    class:recording
    style:color={value || recording ? theme.textPrimary : theme.textDim}
    style:border-color={recording ? theme.accent : theme.border}
    style:background={theme.bodyBg}
    style:font-family={theme.fontMono}
    onclick={() => {
      recording = !recording
      error = null
    }}
    onkeydown={onKeydown}
    onblur={() => (recording = false)}
    title={recording ? 'Press the combo (Esc to cancel)' : 'Click, then press a key combo'}
  >
    {#if recording}Press keys…{:else}{value ?? 'None'}{/if}
  </button>
  {#if value && !recording}
    <button
      type="button"
      class="clear"
      style:color={theme.textDim}
      onclick={() => onChange(null)}
      title="Remove shortcut">Clear</button
    >
  {/if}
</div>
{#if error}
  <p class="error" style:color={theme.err}>{error}</p>
{/if}

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
  }
  .combo {
    min-width: 9rem;
    padding: 3px 8px;
    border: 1px solid;
    border-radius: 5px;
    font-size: 11.5px;
    cursor: pointer;
    text-align: left;
  }
  .combo.recording {
    outline: none;
  }
  .clear {
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 11px;
    padding: 2px 4px;
  }
  .clear:hover {
    text-decoration: underline;
  }
  .error {
    margin: 4px 0 0;
    font-size: 11px;
  }
</style>
