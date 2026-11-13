<script lang="ts">
  import { onMount } from 'svelte'
  import { errorText } from '$lib/bridge/errors'
  import ThemeSelector from './ThemeSelector.svelte'
  import {
    isWindowsHost,
    localShellSetting,
    loadLocalShellSetting,
    saveLocalShellSetting,
  } from '$lib/stores/localShell.svelte'
  import {
    terminalSettings,
    colorSchemes,
    applyFontSize,
    applyFontFamily,
    applyCursorStyle,
    applyCursorBlink,
    DEFAULT_FONT_FAMILY,
  } from '$lib/stores/settings.svelte'

  const FONT_FAMILIES = [
    DEFAULT_FONT_FAMILY,
    'JetBrains Mono Variable, JetBrains Mono, monospace',
    'JetBrains Mono, monospace',
    'Fira Code, monospace',
    'Consolas, monospace',
    'monospace',
  ]

  const FONT_SIZES = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24]

  // ── local shell ──────────────────────────────────────────────────────────
  const SHELL_PRESETS: { value: string; label: string }[] = isWindowsHost
    ? [
        { value: '', label: 'Automatic (PowerShell 7 › PowerShell › cmd)' },
        { value: 'pwsh', label: 'PowerShell 7 (pwsh)' },
        { value: 'powershell', label: 'Windows PowerShell' },
        { value: 'cmd', label: 'Command Prompt (cmd)' },
      ]
    : [{ value: '', label: 'Automatic ($SHELL)' }]
  const CUSTOM = '__custom__'
  const presetValues = SHELL_PRESETS.map((p) => p.value)

  let customOpen = $state(false)
  let customPath = $state('')
  let shellSaved = $state(false)
  let shellError = $state('')
  let savedTimer: ReturnType<typeof setTimeout> | undefined
  const shellChoice = $derived(
    customOpen || !presetValues.includes(localShellSetting.value)
      ? CUSTOM
      : localShellSetting.value,
  )

  onMount(async () => {
    await loadLocalShellSetting()
    if (!presetValues.includes(localShellSetting.value)) customPath = localShellSetting.value
  })

  async function saveShell(value: string) {
    if (value.trim() === localShellSetting.value) return
    try {
      await saveLocalShellSetting(value)
      shellError = ''
      shellSaved = true
      clearTimeout(savedTimer)
      savedTimer = setTimeout(() => (shellSaved = false), 1500)
    } catch (e) {
      shellError = `Couldn't save: ${errorText(e)}`
    }
  }

  function onShellChoice(value: string) {
    if (value === CUSTOM) {
      customOpen = true
      return
    }
    customOpen = false
    void saveShell(value)
  }
</script>

<div class="panel">
  <section>
    <h3 class="section-title">Theme</h3>
    {#if colorSchemes.length === 0}
      <p class="hint">Loading…</p>
    {:else}
      <ThemeSelector />
    {/if}
  </section>

  <section>
    <h3 class="section-title">Font</h3>
    <label class="field">
      <span class="label">Family</span>
      <select
        class="select"
        value={terminalSettings.fontFamily}
        onchange={(e) => applyFontFamily((e.target as HTMLSelectElement).value)}
      >
        {#each FONT_FAMILIES as f}
          <option value={f}>{f.split(',')[0]}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span class="label">Size</span>
      <select
        class="select"
        value={terminalSettings.fontSize}
        onchange={(e) => applyFontSize(parseInt((e.target as HTMLSelectElement).value, 10))}
      >
        {#each FONT_SIZES as s}
          <option value={s}>{s}px</option>
        {/each}
      </select>
    </label>
  </section>

  <section>
    <h3 class="section-title">Local shell</h3>
    <label class="field">
      <span class="label">Shell</span>
      <select
        class="select"
        value={shellChoice}
        onchange={(e) => onShellChoice((e.target as HTMLSelectElement).value)}
      >
        {#each SHELL_PRESETS as p (p.value)}
          <option value={p.value}>{p.label}</option>
        {/each}
        <option value={CUSTOM}>Custom path…</option>
      </select>
    </label>
    {#if shellChoice === CUSTOM}
      <label class="field">
        <span class="label">Path</span>
        <input
          class="select mono"
          bind:value={customPath}
          placeholder={isWindowsHost ? 'C:\\Program Files\\Git\\bin\\bash.exe' : '/bin/zsh'}
          onchange={() => customPath.trim() && saveShell(customPath)}
          onkeydown={(e) => e.key === 'Enter' && customPath.trim() && saveShell(customPath)}
        />
      </label>
    {/if}
    <p class="hint">
      For new local tabs. If it can't start, ZapTerm falls back to the automatic choice.
      {#if shellSaved}<span class="saved">Saved</span>{/if}
    </p>
    {#if shellError}<p class="hint error">{shellError}</p>{/if}
  </section>

  <section>
    <h3 class="section-title">Cursor</h3>
    <div class="cursor-row">
      {#each ['block', 'underline', 'bar'] as const as style}
        <button
          class="cursor-btn"
          class:active={terminalSettings.cursorStyle === style}
          onclick={() => applyCursorStyle(style)}
        >
          {style}
        </button>
      {/each}
    </div>
    <label class="field">
      <span class="label">Blink</span>
      <input
        type="checkbox"
        checked={terminalSettings.cursorBlink}
        onchange={(e) => applyCursorBlink((e.target as HTMLInputElement).checked)}
        class="accent-blue-500"
      />
    </label>
  </section>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 0;
    font-size: 0.8rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }

  section {
    border-bottom: 1px solid var(--zx-border);
    padding-bottom: 0.5rem;
    margin-bottom: 0;
  }

  .section-title {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--zx-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.5rem 0.75rem 0.25rem;
    margin: 0;
  }

  .hint {
    padding: 0.25rem 0.75rem;
    color: var(--zx-text-dim);
    font-size: 0.75rem;
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.2rem 0.75rem;
    gap: 0.5rem;
  }

  .label {
    color: var(--zx-text-muted);
    flex-shrink: 0;
  }

  .select {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    flex: 1;
    max-width: 210px;
  }

  .mono {
    font-family: var(--zx-font-mono);
  }

  .saved {
    color: var(--zx-ok);
    margin-left: 0.4rem;
  }

  .error {
    color: var(--zx-err);
  }

  .cursor-row {
    display: flex;
    gap: 0.25rem;
    padding: 0.2rem 0.75rem;
  }

  .cursor-btn {
    flex: 1;
    padding: 0.2rem 0;
    font-size: 0.7rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text-muted);
    cursor: pointer;
    font-family: inherit;
  }

  .cursor-btn:hover {
    color: var(--zx-text);
    border-color: var(--zx-border);
  }

  .cursor-btn.active {
    background: var(--zx-active-bg);
    border-color: var(--zx-active-border);
    color: var(--zx-accent);
  }
</style>
