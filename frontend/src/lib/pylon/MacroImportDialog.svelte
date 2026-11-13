<script lang="ts">
  /// Paste a MobaXterm macro and import it as one or more ZapTerm macros. Accepts
  /// both the "Macro edition" line list and the real `[Macros]` ini export
  /// (auto-detected). Parsing is live so the user sees how many macros/steps
  /// will be created before importing.
  ///
  /// Vault binding: if a parsed macro types a likely password, we offer to stash
  /// that plaintext in the credential vault and replace the step's `send` with a
  /// `{{vault:<id>}}` reference — so the plaintext never lands in steps_json.
  /// Detection is heuristic and broad, so the user confirms WHICH `send` step is
  /// the password via a masked dropdown (or picks "none").
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import type { PylonTheme } from '$lib/themes/index'
  import type { LoginStep } from '$lib/bridge/types'
  import { parseMobaMacros } from '$lib/snippets/mobaMacro'
  import { createVaultEntry } from '$lib/bridge/commands'
  import { decodeEscapes } from '$lib/orchestrator/macroRunner'
  import { errorText } from '$lib/bridge/errors'

  interface Props {
    theme: PylonTheme
    /** Import a batch of parsed macros (steps already vault-substituted). */
    onImport: (macros: { name: string; steps: LoginStep[] }[]) => void
    onClose: () => void
  }

  const { theme, onImport, onClose }: Props = $props()

  let name = $state('Imported macro')
  let text = $state('')

  const parsed = $derived(parseMobaMacros(text))
  const stepCount = $derived(parsed.reduce((n, m) => n + m.steps.length, 0))
  const canImport = $derived(stepCount > 0)

  // ── Password → vault detection ─────────────────────────────────────────────
  // A candidate `send` step, addressed by macro+step index. `key` is the stable
  // `mi:si` selector value; `likely` flags the heuristic-best guess.
  interface Candidate {
    key: string
    macroIndex: number
    stepIndex: number
    plaintext: string
    likely: boolean
  }

  /// Every `send` step is selectable, but we flag the likely password ones:
  ///  (a) the send follows an expect matching pass(word|phrase) — covers the
  ///      [Macros] export format (WAITFOR → expect), OR
  ///  (b) the send follows a `wait` step AND an earlier `ssh user@host` send
  ///      exists in the same macro AND the candidate isn't itself an `ssh …` /
  ///      obvious non-secret (e.g. a "yes" host-key confirmation).
  function buildCandidates(macros: { name: string; steps: LoginStep[] }[]): Candidate[] {
    const out: Candidate[] = []
    for (let mi = 0; mi < macros.length; mi++) {
      const steps = macros[mi]!.steps
      const hasSshSend = steps.some(
        (s) => s.kind === 'send' && /ssh\s+[^@\s]+@[^\s]+/i.test(decodeEscapes(s.send)),
      )
      for (let si = 0; si < steps.length; si++) {
        const cur = steps[si]!
        if (cur.kind !== 'send') continue
        const plaintext = cur.send.replace(/\\r$/, '')
        if (plaintext.length === 0) continue
        const prev = si > 0 ? steps[si - 1] : null
        const afterPrompt = prev?.kind === 'expect' && /pass(word|phrase)/i.test(prev.expect)
        const decoded = decodeEscapes(plaintext)
        const obviousNonSecret =
          /^\s*(ssh|telnet|yes|no|y|n)\b/i.test(decoded) || /@[^\s]+/.test(decoded)
        const afterWait = prev?.kind === 'wait' && hasSshSend && !obviousNonSecret
        out.push({
          key: `${mi}:${si}`,
          macroIndex: mi,
          stepIndex: si,
          plaintext,
          likely: afterPrompt || afterWait,
        })
      }
    }
    return out
  }

  const candidates = $derived(buildCandidates(parsed))
  // The default likely candidate (first flagged), used to seed the selector.
  const detected = $derived(candidates.find((c) => c.likely) ?? null)

  let saveToVault = $state(true)
  let importError = $state('')
  let vaultName = $state('')
  let vaultUser = $state('')
  // The user's chosen candidate `key`, or '' for "none". Re-seeded when the
  // detected candidate changes.
  let chosenKey = $state('')
  let lastSeeded = ''
  $effect(() => {
    const seed = detected?.key ?? ''
    if (seed !== lastSeeded) {
      lastSeeded = seed
      chosenKey = seed
    }
  })

  const chosen = $derived(candidates.find((c) => c.key === chosenKey) ?? null)

  // Prefill name/username when a candidate is chosen. Username comes from a
  // preceding `ssh user@host` send if parseable.
  $effect(() => {
    if (!chosen) return
    const steps = parsed[chosen.macroIndex]!.steps
    const name0 = parsed[chosen.macroIndex]!.name
    if (!vaultName) vaultName = name0 || 'Imported credential'
    if (!vaultUser) vaultUser = guessUsername(steps) ?? ''
  })

  function guessUsername(steps: LoginStep[]): string | null {
    for (const s of steps) {
      if (s.kind !== 'send') continue
      const m = /ssh\s+([^@\s]+)@[^\s]+/i.exec(decodeEscapes(s.send))
      if (m) return m[1]!
    }
    return null
  }

  /// Mask a candidate's value for the dropdown so the plaintext isn't shown.
  function mask(s: string): string {
    const len = s.length
    return len <= 2 ? '••' : `${'•'.repeat(Math.min(len, 8))} (${len} chars)`
  }

  function candidateLabel(c: Candidate): string {
    const macroName = parsed[c.macroIndex]!.name || `macro ${c.macroIndex + 1}`
    const flag = c.likely ? '🔒 ' : ''
    return `${flag}${macroName} · step ${c.stepIndex + 1}: ${mask(decodeEscapes(c.plaintext))}`
  }

  async function doImport() {
    if (!canImport) return
    // Deep-clone parsed steps so the substitution doesn't mutate $derived state.
    const macros = parsed.map((m) => ({
      name: m.name,
      steps: m.steps.map((s) => ({ ...s })),
    }))

    importError = ''
    if (saveToVault && chosen) {
      try {
        const entryName = vaultName.trim() || 'Imported credential'
        // The step text is escaped for the macro runner (`\\` for a
        // backslash, `\x7f` for BACK…); the vault types its secret byte for
        // byte, so store what the macro would actually have typed.
        await createVaultEntry(entryName, vaultUser.trim() || null, decodeEscapes(chosen.plaintext))
        // Replace the chosen step's send with a name+field vault reference (so
        // the plaintext never lands in steps_json). Preserve a trailing Enter
        // if the original had one.
        const orig = parsed[chosen.macroIndex]!.steps[chosen.stepIndex]!
        const enter = /\\r$/.test(orig.send) ? '\\r' : ''
        macros[chosen.macroIndex]!.steps[chosen.stepIndex]!.send =
          `{{vault:${entryName}.Password}}${enter}`
      } catch (e) {
        // Don't import anyway: the macro would keep the password in plain
        // text, which is what the user asked to avoid. Say why (a name
        // already in the vault, typically) so they can fix it and retry.
        importError = `Couldn't save the password to the vault: ${errorText(e)}`
        return
      }
    }

    // For a single unnamed macro (editor format) use the typed name; otherwise
    // keep each parsed macro's own name.
    if (macros.length === 1 && !macros[0]!.name) {
      macros[0]!.name = name.trim() || 'Imported macro'
    } else {
      for (const m of macros) {
        if (!m.name) m.name = name.trim() || 'Imported macro'
      }
    }
    onImport(macros)
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
  aria-label="Import MobaXterm macro"
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
    <h3 style:color={theme.textPrimary}>Import macro from MobaXterm</h3>
    <p class="hint" style:color={theme.textDim}>
      Paste the macro editor lines, or a `[Macros]` export line (name=step|step|…). Multiple export
      lines import as separate macros.
    </p>

    <input
      class="name"
      placeholder="Macro name (fallback for unnamed macros)"
      bind:value={name}
      style:background={theme.bodyBg}
      style:color={theme.textPrimary}
      style:border="1px solid {theme.border}"
    />

    <textarea
      class="paste"
      placeholder={'ssh user@host\nRETURN\nSLEEP=1200\nyes\nRETURN'}
      bind:value={text}
      spellcheck="false"
      style:background={theme.bodyBg}
      style:color={theme.textPrimary}
      style:border="1px solid {theme.border}"
      style:font-family={theme.fontMono}
    ></textarea>

    {#if candidates.length > 0}
      <div
        class="vault-box"
        style:border="1px solid {theme.border}"
        style:background={theme.bodyBg}
      >
        <label class="vault-toggle" style:color={theme.textPrimary}>
          <input type="checkbox" bind:checked={saveToVault} />
          🔒 Save a typed password to the vault
        </label>
        {#if saveToVault}
          <label class="vault-pick" style:color={theme.textDim}>
            Which step is the password?
            <select
              bind:value={chosenKey}
              style:background={theme.sidebarBg}
              style:color={theme.textPrimary}
              style:border="1px solid {theme.border}"
            >
              <option value="">None — import as-is</option>
              {#each candidates as c (c.key)}
                <option value={c.key}>{candidateLabel(c)}</option>
              {/each}
            </select>
          </label>
          {#if chosen}
            <div class="vault-fields">
              <input
                class="vfield"
                placeholder="Vault entry name"
                bind:value={vaultName}
                style:background={theme.sidebarBg}
                style:color={theme.textPrimary}
                style:border="1px solid {theme.border}"
              />
              <input
                class="vfield"
                placeholder="Username (optional)"
                bind:value={vaultUser}
                style:background={theme.sidebarBg}
                style:color={theme.textPrimary}
                style:border="1px solid {theme.border}"
              />
            </div>
          {/if}
        {/if}
      </div>
    {/if}

    {#if importError}
      <p class="import-error" style:color={theme.err}>{importError}</p>
    {/if}

    <div class="footer">
      <span class="count" style:color={theme.textMuted}>
        {parsed.length}
        {parsed.length === 1 ? 'macro' : 'macros'}, {stepCount}
        {stepCount === 1 ? 'step' : 'steps'}
      </span>
      <span class="spacer"></span>
      <button
        class="btn"
        style:color={theme.textMuted}
        style:border="1px solid {theme.border}"
        onclick={onClose}>Cancel</button
      >
      <button
        class="btn primary"
        disabled={!canImport}
        style:background={theme.accent}
        style:color={theme.onAccent}
        onclick={doImport}>Import</button
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
    width: 480px;
    max-width: 94vw;
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
  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.4;
  }
  .name {
    border-radius: 5px;
    padding: 5px 8px;
    font-size: 12.5px;
    font-family: inherit;
    outline: none;
  }
  .paste {
    min-height: 160px;
    max-height: 320px;
    resize: vertical;
    border-radius: 5px;
    padding: 7px 9px;
    font-size: 12px;
    line-height: 1.5;
    outline: none;
  }
  .vault-box {
    border-radius: 6px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .vault-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    cursor: pointer;
  }
  .vault-pick {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
  }
  .vault-pick select {
    border-radius: 5px;
    padding: 4px 6px;
    font-size: 11.5px;
    font-family: inherit;
    outline: none;
  }
  .vault-fields {
    display: flex;
    gap: 6px;
  }
  .vfield {
    flex: 1;
    min-width: 0;
    border-radius: 5px;
    padding: 4px 7px;
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }
  .import-error {
    margin: 0;
    font-size: 12px;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .count {
    font-size: 11.5px;
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
  .btn.primary {
    font-weight: 600;
  }
  .btn.primary:disabled {
    opacity: 0.45;
    cursor: default;
    filter: none;
  }
</style>
