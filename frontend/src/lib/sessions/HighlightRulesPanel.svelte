<script lang="ts">
  import { tick } from 'svelte'
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
  import { errorText } from '$lib/bridge/errors'
  import Icon from '$lib/icons/Icon.svelte'
  import {
    listHighlightRules,
    createHighlightRule,
    updateHighlightRule,
    toggleHighlightRule,
    deleteHighlightRule,
    exportHighlightRules,
    importHighlightRules,
  } from '$lib/bridge/commands'
  import type { HighlightRule } from '$lib/bridge/types'

  // ── state ──────────────────────────────────────────────────────────────────
  let rules = $state<HighlightRule[]>([])
  let loading = $state(false)
  let error = $state<string | null>(null)
  /** Result of the last export/import, shown under the toolbar. */
  let status = $state<{ kind: 'ok' | 'err'; text: string; details?: string[] } | null>(null)

  // Rule form — adds a rule, or edits `editing` (pattern and regex flag stay:
  // they are the rule's identity, also for sync).
  const DEFAULT_FG = '#e07474'
  const DEFAULT_BG = '#3a2a2a'
  let editing = $state<HighlightRule | null>(null)
  let formCard = $state<HTMLElement | null>(null)
  let fName = $state('')
  let fPattern = $state('')
  let fIsRegex = $state(false)
  let fUseFg = $state(true)
  let fFg = $state(DEFAULT_FG)
  let fUseBg = $state(false)
  let fBg = $state(DEFAULT_BG)
  let fBold = $state(false)
  let fUnderline = $state(false)
  let saving = $state(false)
  let formError = $state<string | null>(null)
  let testInput = $state('%LINK-3-UPDOWN: Interface Gi0/1, changed state to down')

  // ── live tester ────────────────────────────────────────────────────────────
  interface Span {
    text: string
    hit: boolean
  }

  function escapeRegex(s: string) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  }

  const patternError = $derived.by(() => {
    if (!fIsRegex || !fPattern) return null
    try {
      new RegExp(fPattern)
      return null
    } catch (e) {
      return errorText(e)
    }
  })

  const testSpans = $derived.by<Span[]>(() => {
    if (!testInput || !fPattern || patternError) return [{ text: testInput, hit: false }]
    const regex = new RegExp(fIsRegex ? fPattern : escapeRegex(fPattern), 'g')
    const spans: Span[] = []
    let last = 0
    for (const m of testInput.matchAll(regex)) {
      if (!m[0]) break // an empty match would loop forever
      const start = m.index ?? 0
      if (start > last) spans.push({ text: testInput.slice(last, start), hit: false })
      spans.push({ text: m[0], hit: true })
      last = start + m[0].length
    }
    if (last < testInput.length) spans.push({ text: testInput.slice(last), hit: false })
    return spans
  })
  const testHits = $derived(testSpans.filter((s) => s.hit).length)

  // ── data ───────────────────────────────────────────────────────────────────
  async function load() {
    loading = true
    error = null
    try {
      rules = await listHighlightRules()
    } catch (e) {
      error = errorText(e)
    } finally {
      loading = false
    }
  }

  function resetForm() {
    editing = null
    fName = ''
    fPattern = ''
    fIsRegex = false
    fUseFg = true
    fFg = DEFAULT_FG
    fUseBg = false
    fBg = DEFAULT_BG
    fBold = false
    fUnderline = false
    formError = null
  }

  async function startEdit(rule: HighlightRule) {
    editing = rule
    fName = rule.name
    fPattern = rule.pattern
    fIsRegex = rule.is_regex
    fUseFg = !!rule.fg_color
    fFg = rule.fg_color ?? DEFAULT_FG
    fUseBg = !!rule.bg_color
    fBg = rule.bg_color ?? DEFAULT_BG
    fBold = rule.bold
    fUnderline = rule.underline
    formError = null
    // The form sits under the list: bring it into view.
    await tick()
    formCard?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }

  async function handleSubmit() {
    formError = null
    if (!fName.trim()) {
      formError = 'Name is required'
      return
    }
    if (!fPattern.trim()) {
      formError = 'Pattern is required'
      return
    }
    if (patternError) {
      formError = `Invalid regex: ${patternError}`
      return
    }
    const fg = fUseFg ? fFg : null
    const bg = fUseBg ? fBg : null
    saving = true
    try {
      if (editing) {
        await updateHighlightRule(
          editing.id,
          fName.trim(),
          fg,
          bg,
          fBold,
          fUnderline,
          editing.enabled,
        )
      } else {
        await createHighlightRule(
          fName.trim(),
          fPattern.trim(),
          fIsRegex,
          fg,
          bg,
          fBold,
          fUnderline,
        )
      }
      resetForm()
      await load()
    } catch (e) {
      formError = errorText(e)
    } finally {
      saving = false
    }
  }

  async function handleToggle(rule: HighlightRule) {
    try {
      await toggleHighlightRule(rule.id, !rule.enabled)
      await load()
    } catch (e) {
      error = errorText(e)
    }
  }

  async function handleDelete(rule: HighlightRule) {
    try {
      await deleteHighlightRule(rule.id)
      if (editing?.id === rule.id) resetForm()
      await load()
    } catch (e) {
      error = errorText(e)
    }
  }

  // ── export / import ────────────────────────────────────────────────────────
  async function handleExport() {
    status = null
    const path = await saveDialog({
      title: 'Export highlight rules',
      defaultPath: 'zapterm-highlight-rules.json',
      filters: [{ name: 'Highlight rules', extensions: ['json'] }],
    })
    if (!path) return
    try {
      const n = await exportHighlightRules(path)
      status = { kind: 'ok', text: `Exported ${n} rule${n === 1 ? '' : 's'} to ${path}` }
    } catch (e) {
      status = { kind: 'err', text: errorText(e) }
    }
  }

  async function handleImport() {
    status = null
    const path = await openDialog({
      title: 'Import highlight rules',
      multiple: false,
      directory: false,
      filters: [{ name: 'Highlight rules', extensions: ['json'] }],
    })
    if (typeof path !== 'string' || !path) return
    try {
      const r = await importHighlightRules(path)
      const parts = [`${r.added} added`]
      if (r.skipped) parts.push(`${r.skipped} already present`)
      if (r.invalid.length) parts.push(`${r.invalid.length} left out`)
      status = {
        kind: r.invalid.length ? 'err' : 'ok',
        text: `Import: ${parts.join(', ')}`,
        details: r.invalid,
      }
      await load()
    } catch (e) {
      status = { kind: 'err', text: errorText(e) }
    }
  }

  $effect(() => {
    load()
  })
</script>

<div class="panel">
  <section>
    <div class="head">
      <h3>Rules</h3>
      <div class="actions">
        <button type="button" class="btn" onclick={handleImport}>
          <Icon name="upload" size={13} /> Import…
        </button>
        <button type="button" class="btn" onclick={handleExport} disabled={rules.length === 0}>
          <Icon name="download" size={13} /> Export…
        </button>
      </div>
    </div>
    <p class="help">
      Text matching a rule is colored in every terminal. Export the rules to a JSON file to share
      them or move them to another computer; importing skips rules you already have.
    </p>

    {#if status}
      <div class="status" class:err={status.kind === 'err'}>
        <p>{status.text}</p>
        {#each status.details ?? [] as d, i (i)}
          <p class="detail">{d}</p>
        {/each}
      </div>
    {/if}
    {#if error}
      <p class="msg-err">{error}</p>
    {/if}

    {#if loading && rules.length === 0}
      <p class="empty">Loading…</p>
    {:else if rules.length === 0}
      <p class="empty">No rules yet — add one below or import a file.</p>
    {:else}
      <ul class="rules">
        {#each rules as rule (rule.id)}
          <li class="rule-row" class:off={!rule.enabled} class:selected={editing?.id === rule.id}>
            <input
              type="checkbox"
              checked={rule.enabled}
              onchange={() => handleToggle(rule)}
              aria-label="Enable {rule.name}"
              title={rule.enabled ? 'Enabled' : 'Disabled'}
            />
            <span class="rule-name" title={rule.name}>{rule.name}</span>
            <span
              class="rule-chip"
              style:color={rule.fg_color ?? 'var(--zx-term-fg)'}
              style:background-color={rule.bg_color ?? 'var(--zx-term-bg)'}
              style:font-weight={rule.bold ? 'bold' : undefined}
              style:text-decoration={rule.underline ? 'underline' : undefined}
              title="Preview">Aa</span
            >
            <code class="rule-pattern" title={rule.pattern}>{rule.pattern}</code>
            <span class="rule-kind" title={rule.is_regex ? 'Regular expression' : 'Plain text'}
              >{rule.is_regex ? 'regex' : 'text'}</span
            >
            <button
              type="button"
              class="icon-btn"
              onclick={() => startEdit(rule)}
              aria-label="Edit {rule.name}"
              title="Edit"><Icon name="pencil" size={12} /></button
            >
            <button
              type="button"
              class="icon-btn danger"
              onclick={() => handleDelete(rule)}
              aria-label="Delete {rule.name}"
              title="Delete"><Icon name="x" size={12} /></button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="card" bind:this={formCard}>
    <h3>{editing ? `Edit “${editing.name}”` : 'New rule'}</h3>
    <form
      onsubmit={(e) => {
        e.preventDefault()
        handleSubmit()
      }}
    >
      <div class="grid">
        <label class="field">
          <span>Name</span>
          <input bind:value={fName} placeholder="Interface down" />
        </label>
        <label class="field">
          <span>Pattern</span>
          <input
            bind:value={fPattern}
            class="mono"
            placeholder={fIsRegex ? 'changed state to (down|administratively down)' : '%ERROR'}
            disabled={!!editing}
            title={editing ? 'The pattern of a rule cannot change — add a new rule instead' : ''}
          />
        </label>
      </div>

      <label class="check">
        <input type="checkbox" bind:checked={fIsRegex} disabled={!!editing} />
        <span>Regular expression</span>
      </label>
      {#if patternError}
        <p class="msg-err">Invalid regex: {patternError}</p>
      {/if}

      <div class="style-row">
        <div class="color">
          <label class="check">
            <input type="checkbox" bind:checked={fUseFg} />
            <span>Text color</span>
          </label>
          <input type="color" bind:value={fFg} disabled={!fUseFg} aria-label="Text color" />
        </div>
        <div class="color">
          <label class="check">
            <input type="checkbox" bind:checked={fUseBg} />
            <span>Background</span>
          </label>
          <input type="color" bind:value={fBg} disabled={!fUseBg} aria-label="Background color" />
        </div>
        <label class="check">
          <input type="checkbox" bind:checked={fBold} />
          <span>Bold</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={fUnderline} />
          <span>Underline</span>
        </label>
      </div>

      <label class="field">
        <span>Try it on sample output</span>
        <input bind:value={testInput} class="mono" placeholder="Paste a line of output…" />
      </label>
      <div class="preview" aria-label="Preview">
        {#each testSpans as span, i (i)}
          {#if span.hit}<span
              style:color={fUseFg ? fFg : undefined}
              style:background-color={fUseBg ? fBg : undefined}
              style:font-weight={fBold ? 'bold' : undefined}
              style:text-decoration={fUnderline ? 'underline' : undefined}>{span.text}</span
            >{:else}{span.text}{/if}
        {/each}
        {#if !testInput}<span class="placeholder">The preview shows here</span>{/if}
      </div>
      {#if testInput && fPattern && !patternError}
        <p class="help">
          {testHits ? `${testHits} match${testHits === 1 ? '' : 'es'}` : 'No match'}
        </p>
      {/if}

      {#if formError}
        <p class="msg-err">{formError}</p>
      {/if}

      <div class="form-actions">
        <button type="submit" class="btn primary" disabled={saving}>
          {#if editing}<Icon name="check" size={13} />
            {saving ? 'Saving…' : 'Save changes'}{:else}<Icon name="plus" size={13} />
            {saving ? 'Adding…' : 'Add rule'}{/if}
        </button>
        {#if editing}
          <button type="button" class="btn ghost" onclick={resetForm}>Cancel</button>
        {/if}
      </div>
    </form>
  </section>
</div>

<style>
  .panel {
    padding: 0.75rem 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    font-size: 0.82rem;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .card {
    background: color-mix(in srgb, var(--zx-text) 4%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius, 7px);
    padding: 0.8rem 0.9rem 0.9rem;
  }

  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .actions,
  .form-actions {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .help {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0;
    line-height: 1.45;
  }

  .empty {
    color: var(--zx-text-muted);
    margin: 0;
    font-size: 0.78rem;
  }

  .msg-err {
    color: var(--zx-err);
    margin: 0;
    font-size: 0.75rem;
  }

  .status {
    border-left: 3px solid var(--zx-ok);
    background: color-mix(in srgb, var(--zx-ok) 10%, transparent);
    padding: 0.35rem 0.6rem;
    border-radius: 0 4px 4px 0;
    font-size: 0.75rem;
  }
  .status.err {
    border-left-color: var(--zx-warn);
    background: color-mix(in srgb, var(--zx-warn) 12%, transparent);
  }
  .status p {
    margin: 0;
    word-break: break-all;
  }
  .status .detail {
    color: var(--zx-text-muted);
    font-size: 0.7rem;
  }

  /* ── buttons ─────────────────────────────────────────────────────────── */
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    background: color-mix(in srgb, var(--zx-accent) 14%, transparent);
    color: var(--zx-accent);
    border: 1px solid color-mix(in srgb, var(--zx-accent) 30%, transparent);
    border-radius: 0.25rem;
    padding: 0.3rem 0.65rem;
    font-size: 0.78rem;
    font-family: inherit;
    cursor: pointer;
  }
  .btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-accent) 24%, transparent);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn.primary {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    border-color: var(--zx-accent);
    font-weight: 600;
  }
  .btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-accent) 85%, var(--zx-text));
  }
  .btn.ghost {
    background: transparent;
    color: var(--zx-text-muted);
    border-color: var(--zx-border);
  }
  .btn.ghost:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }

  /* ── rule list ───────────────────────────────────────────────────────── */
  .rules {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .rule-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto minmax(0, 12rem) auto auto auto;
    gap: 0.6rem;
    align-items: center;
    padding: 0.35rem 0.6rem;
    background: color-mix(in srgb, var(--zx-text) 4%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
  }
  .rule-row.selected {
    border-color: var(--zx-accent);
    background: var(--zx-active-bg);
  }
  .rule-row.off .rule-name,
  .rule-row.off .rule-chip,
  .rule-row.off .rule-pattern {
    opacity: 0.45;
  }

  .rule-row input[type='checkbox'],
  .check input {
    accent-color: var(--zx-accent);
  }

  .rule-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rule-chip {
    padding: 0.05rem 0.5rem;
    border-radius: 3px;
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
    line-height: 1.3;
    min-width: 2rem;
    text-align: center;
    border: 1px solid var(--zx-border);
  }

  .rule-pattern {
    color: var(--zx-text-muted);
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: var(--zx-surface-2);
    padding: 0.05rem 0.4rem;
    border-radius: 3px;
  }

  .rule-kind {
    font-size: 0.62rem;
    color: var(--zx-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.05rem 0.35rem;
    border: 1px solid var(--zx-border);
    border-radius: 3px;
  }

  .icon-btn {
    display: inline-flex;
    background: transparent;
    border: 0;
    color: var(--zx-text-muted);
    cursor: pointer;
    padding: 0.2rem;
    border-radius: 3px;
  }
  .icon-btn:hover {
    color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 12%, transparent);
  }
  .icon-btn.danger:hover {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 12%, transparent);
  }

  /* ── form ────────────────────────────────────────────────────────────── */
  form {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr);
    gap: 0.7rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .field > span {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
  }

  .field input {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.35rem 0.5rem;
    color: var(--zx-text);
    font-size: 0.82rem;
    font-family: inherit;
    outline: none;
  }
  .field input:focus {
    border-color: var(--zx-accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--zx-accent) 25%, transparent);
  }
  .field input:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .field input::placeholder {
    color: var(--zx-text-dim);
  }
  .mono,
  .field input.mono {
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
    color: var(--zx-text);
    font-size: 0.8rem;
  }
  .check:has(input:disabled) {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .style-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.6rem 1.2rem;
  }

  .color {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
  }

  input[type='color'] {
    width: 2.2rem;
    height: 1.6rem;
    padding: 0.1rem;
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    background: var(--zx-surface-2);
    cursor: pointer;
  }
  input[type='color']:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .preview {
    font-family: var(--zx-font-mono);
    font-size: 0.8rem;
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    border-radius: 0.3rem;
    padding: 0.5rem 0.65rem;
    white-space: pre-wrap;
    word-break: break-all;
    min-height: 1.2rem;
  }
  .preview .placeholder {
    color: var(--zx-term-dim);
  }
</style>
