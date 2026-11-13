<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  /// Centered modal to create or edit a macro from the sidebar Macros section.
  /// Hosts the shared MacroStepEditor plus name/colour. Saving is delegated to
  /// the caller (App.svelte) which persists via createSnippet/updateSnippet +
  /// setSnippetSteps.
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import type { PylonTheme } from '$lib/themes/index'
  import type { Snippet, LoginStep, VaultEntry } from '$lib/bridge/types'
  import { listVaultEntries } from '$lib/bridge/commands'
  import MacroStepEditor from './MacroStepEditor.svelte'
  import ShortcutField from '$lib/ui/ShortcutField.svelte'

  interface Props {
    theme: PylonTheme
    /** Editing an existing macro, or null when creating one. */
    macro: Snippet | null
    /** Existing folder names, offered as autocomplete suggestions. */
    folders?: string[]
    onSave: (v: {
      name: string
      color: string | null
      folder: string | null
      steps: LoginStep[]
      /** Own keyboard shortcut, or null for none. */
      shortcut: string | null
    }) => void
    onDelete?: () => void
    onClose: () => void
  }

  const { theme, macro, folders = [], onSave, onDelete, onClose }: Props = $props()

  // One-shot snapshot of the macro into editable local state (the dialog is
  // remounted via {#key} when the target changes).
  // svelte-ignore state_referenced_locally
  let name = $state(macro?.name ?? '')
  // svelte-ignore state_referenced_locally
  let color = $state<string | null>(macro?.color ?? null)
  // svelte-ignore state_referenced_locally
  let folder = $state(macro?.folder ?? '')
  // svelte-ignore state_referenced_locally
  let shortcut = $state<string | null>(macro?.shortcut ?? null)
  // svelte-ignore state_referenced_locally
  let steps = $state<LoginStep[]>(initialSteps(macro?.steps_json ?? null))

  /// Parse the saved steps, seeding one empty step so a fresh macro isn't an
  /// empty list.
  function initialSteps(json: string | null): LoginStep[] {
    if (json) {
      try {
        const v = JSON.parse(json)
        if (Array.isArray(v) && v.length > 0) return normalizeExpectSteps(v)
      } catch {
        /* fall through to the seeded default */
      }
    }
    return [{ kind: 'expect', expect: '', is_regex: false, send: '', timeout_ms: 10000 }]
  }

  /// The visual Steps editor treats `expect` as a pure wait — typing lives in a
  /// separate `send` step (which owns the vault picker). Legacy / imported
  /// macros can still carry an `expect` step that also sends a payload; split
  /// each of those into an `expect` (send:'') immediately followed by an
  /// equivalent `send` step so nothing is hidden. Run-time behaviour is
  /// identical (the runner already sends after matching). Called once on open.
  function normalizeExpectSteps(steps: LoginStep[]): LoginStep[] {
    const out: LoginStep[] = []
    for (const step of steps) {
      if (step.kind === 'expect' && typeof step.send === 'string' && step.send.length > 0) {
        out.push({
          kind: 'expect',
          expect: step.expect,
          is_regex: step.is_regex,
          send: '',
          timeout_ms: step.timeout_ms,
        })
        out.push({
          kind: 'send',
          expect: '',
          is_regex: false,
          send: step.send,
          timeout_ms: step.timeout_ms,
        })
      } else {
        out.push(step)
      }
    }
    return out
  }

  // Vault entries available to reference from a `send` step. Loaded lazily;
  // an empty list simply hides the "Insert vault reference" affordance.
  let vaultEntries = $state<VaultEntry[]>([])
  $effect(() => {
    listVaultEntries()
      .then((v) => (vaultEntries = v))
      .catch(() => {})
  })

  const SWATCHES = ['#5eb3b2', '#5b8fc9', '#9a91e8', '#3e8f60', '#b88528', '#c2410c', '#b13a3a']

  // ── Steps / JSON edit mode ──────────────────────────────────────────────
  // The JSON view is seeded from the current `steps` whenever it's entered, and
  // validated live; the parsed result is committed back to `steps` on toggle to
  // Steps and on Save. `jsonError` is non-null while the text is invalid.
  type EditMode = 'steps' | 'json'
  let editMode = $state<EditMode>('steps')
  let jsonText = $state('')
  let jsonError = $state<string | null>(null)

  const STEP_KINDS = ['expect', 'send', 'wait']

  /// Validate + normalize a parsed JSON value into a LoginStep[]. Throws a
  /// human-readable message on the first problem; coerces missing optional
  /// fields to their defaults so a terse hand-written array still loads.
  function parseStepsJson(text: string): LoginStep[] {
    let v: unknown
    try {
      v = JSON.parse(text)
    } catch (e) {
      throw new Error(e instanceof Error ? e.message : 'invalid JSON')
    }
    if (!Array.isArray(v)) throw new Error('expected a JSON array of steps')
    return v.map((raw, i) => {
      if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
        throw new Error(`step ${i + 1}: expected an object`)
      }
      const o = raw as Record<string, unknown>
      const kind = o.kind
      if (typeof kind !== 'string' || !STEP_KINDS.includes(kind)) {
        throw new Error(`step ${i + 1}: "kind" must be one of expect | send | wait`)
      }
      const tn = o.timeout_ms
      const timeout_ms = typeof tn === 'number' && Number.isFinite(tn) ? tn : 10000
      return {
        kind: kind as LoginStep['kind'],
        expect: typeof o.expect === 'string' ? o.expect : '',
        is_regex: o.is_regex === true,
        send: typeof o.send === 'string' ? o.send : '',
        timeout_ms,
      }
    })
  }

  function setMode(mode: EditMode) {
    if (mode === editMode) return
    if (mode === 'json') {
      // Seed the textarea from the current visual steps.
      jsonText = JSON.stringify(steps, null, 2)
      jsonError = null
    } else {
      // Leaving JSON → commit if valid; block the toggle while invalid.
      try {
        steps = parseStepsJson(jsonText)
        jsonError = null
      } catch (e) {
        jsonError = errorText(e)
        return
      }
    }
    editMode = mode
  }

  // Live validation while typing in JSON mode.
  $effect(() => {
    if (editMode !== 'json') return
    try {
      parseStepsJson(jsonText)
      jsonError = null
    } catch (e) {
      jsonError = errorText(e)
    }
  })

  const canSave = $derived(
    !!name.trim() &&
      (editMode === 'json' ? jsonError === null && jsonText.trim().length > 0 : steps.length > 0),
  )

  function save() {
    if (!canSave) return
    // In JSON mode, commit the textarea into `steps` first.
    let finalSteps = steps
    if (editMode === 'json') {
      try {
        finalSteps = parseStepsJson(jsonText)
      } catch (e) {
        jsonError = errorText(e)
        return
      }
    }
    onSave({
      name: name.trim(),
      color,
      folder: folder.trim() || null,
      steps: finalSteps,
      shortcut,
    })
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
    }
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label={macro ? 'Edit macro' : 'New macro'}
  tabindex="-1"
  onkeydown={onKeydown}
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose()
  }}
  transition:fade={{ duration: 120 }}
>
  <div
    class="dialog"
    style:background={theme.sidebarBg}
    style:border="1px solid {theme.border}"
    style:box-shadow={theme.windowShadow}
    style:font-family={theme.fontUi}
    transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}
  >
    <h3 style:color={theme.textPrimary}>{macro ? 'Edit macro' : 'New macro'}</h3>

    <div class="row">
      <input
        class="name"
        placeholder="Macro name"
        bind:value={name}
        style:background={theme.bodyBg}
        style:color={theme.textPrimary}
        style:border="1px solid {theme.border}"
      />
      <div class="swatches">
        <button
          class="swatch none"
          class:sel={color === null}
          title="No color (theme default)"
          style:border-color={theme.border}
          style:color={theme.textDim}
          onclick={() => (color = null)}>✕</button
        >
        {#each SWATCHES as c (c)}
          <button
            class="swatch"
            class:sel={color === c}
            style:background={c}
            style:box-shadow={color === c ? `0 0 0 2px ${theme.accent}` : 'none'}
            title={c}
            aria-label={`Color ${c}`}
            onclick={() => (color = c)}
          ></button>
        {/each}
      </div>
    </div>

    <label class="folder-label" style:color={theme.textMuted}>
      Folder
      <input
        class="folder"
        placeholder="Folder (optional)"
        list="macro-folder-list"
        bind:value={folder}
        style:background={theme.bodyBg}
        style:color={theme.textPrimary}
        style:border="1px solid {theme.border}"
      />
    </label>
    <datalist id="macro-folder-list">
      {#each folders as f (f)}
        <option value={f}></option>
      {/each}
    </datalist>

    <ShortcutField
      {theme}
      value={shortcut}
      selfId={macro?.id ?? null}
      onChange={(v) => (shortcut = v)}
    />

    <div class="mode" style:color={theme.textMuted}>
      <button
        type="button"
        class="mode-btn"
        class:active={editMode === 'steps'}
        style:color={editMode === 'steps' ? theme.accent : theme.textDim}
        style:border-color={editMode === 'steps' ? theme.accent : theme.border}
        onclick={() => setMode('steps')}>Steps</button
      >
      <button
        type="button"
        class="mode-btn"
        class:active={editMode === 'json'}
        style:color={editMode === 'json' ? theme.accent : theme.textDim}
        style:border-color={editMode === 'json' ? theme.accent : theme.border}
        onclick={() => setMode('json')}>JSON</button
      >
    </div>

    {#if editMode === 'json'}
      <textarea
        class="json"
        spellcheck="false"
        bind:value={jsonText}
        style:background={theme.bodyBg}
        style:color={theme.textPrimary}
        style:border="1px solid {jsonError ? theme.err : theme.border}"
        style:font-family={theme.fontMono}
      ></textarea>
      {#if jsonError}
        <span class="json-err" style:color={theme.err}>{jsonError}</span>
      {/if}
    {:else}
      <MacroStepEditor {theme} bind:steps {vaultEntries} />
    {/if}

    <div class="footer">
      {#if onDelete}
        <button class="btn danger" style:color={theme.err} onclick={onDelete}>Delete</button>
      {/if}
      <span class="spacer"></span>
      <button
        class="btn"
        style:color={theme.textMuted}
        style:border="1px solid {theme.border}"
        onclick={onClose}>Cancel</button
      >
      <button
        class="btn primary"
        disabled={!canSave}
        style:background={theme.accent}
        style:color={theme.onAccent}
        onclick={save}>Save</button
      >
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
  }
  .dialog {
    width: 540px;
    max-width: 94vw;
    max-height: 88vh;
    overflow-y: auto;
    border-radius: 10px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    flex: 1;
    min-width: 0;
    border-radius: 5px;
    padding: 5px 8px;
    font-size: 12.5px;
    font-family: inherit;
    outline: none;
  }
  .folder-label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
  }
  .folder {
    border-radius: 5px;
    padding: 5px 8px;
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }
  .mode {
    display: flex;
    gap: 4px;
  }
  .mode-btn {
    background: transparent;
    border: 1px solid;
    border-radius: 5px;
    padding: 2px 10px;
    font-size: 11.5px;
    font-family: inherit;
    cursor: pointer;
  }
  .json {
    min-height: 160px;
    max-height: 320px;
    resize: vertical;
    border-radius: 5px;
    padding: 7px 9px;
    font-size: 12px;
    line-height: 1.5;
    outline: none;
  }
  .json-err {
    font-size: 11px;
    line-height: 1.4;
  }
  .swatches {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .swatch {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: transform 0.1s;
  }
  .swatch:hover {
    transform: scale(1.15);
  }
  .swatch.none {
    background: transparent;
    border: 1px solid;
    font-size: 9px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .swatch.none.sel {
    font-weight: 700;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }
  .spacer {
    flex: 1;
  }
  .btn {
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 5px 12px;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
  }
  .btn:hover {
    filter: brightness(1.1);
  }
  .btn.danger {
    margin-right: auto;
  }
  .btn.primary {
    font-weight: 600;
  }
  .btn.primary:disabled {
    opacity: 0.45;
    cursor: default;
    filter: none;
  }
</style>
