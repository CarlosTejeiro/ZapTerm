<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  /// Confirms a Python script run: shows the file, the tab it starts on, the
  /// interpreter ZapTerm will use (overridable) and the mode: dry run, read-only
  /// (only allowlisted display commands are typed) or for real.
  import { invoke } from '@tauri-apps/api/core'
  import { getSetting, setSetting } from '$lib/bridge/commands'
  import { parseAllowList } from './output'

  interface Props {
    path: string
    /** Name of the focused tab, or null when none (the script can still open
     *  saved sessions). */
    target: string | null
    onRun: (mode: RunMode, allow: string[]) => void
    onCancel: () => void
  }

  type RunMode = 'dry' | 'read-only' | 'real'

  const { path, target, onRun, onCancel }: Props = $props()

  let mode = $state<RunMode>('dry')
  let allowText = $state('get, show')
  let python = $state('')
  let pythonInfo = $state<{ ok: boolean; text: string } | null>(null)
  let checking = $state(false)

  const fileName = $derived(path.split(/[\\/]/).pop() ?? path)

  async function check(override: string | null) {
    checking = true
    try {
      const text = await invoke<string>('script_python_info', { python: override })
      pythonInfo = { ok: true, text }
    } catch (e) {
      pythonInfo = { ok: false, text: errorText(e) }
    } finally {
      checking = false
    }
  }

  $effect(() => {
    getSetting('scripts.read_only_allow')
      .then((v) => {
        if (v) allowText = v
      })
      .catch(() => {})
    getSetting('scripts.python')
      .then((v) => {
        python = v ?? ''
      })
      .catch(() => {})
      .finally(() => check(null))
  })

  async function run() {
    // Persist an explicit interpreter (blank = auto-detect) before starting.
    await setSetting('scripts.python', python.trim()).catch(() => {})
    const allow = parseAllowList(allowText)
    await setSetting('scripts.read_only_allow', allow.join(', ')).catch(() => {})
    onRun(mode, allow)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onCancel()
    }
  }
</script>

<div class="overlay" role="presentation" onkeydown={onKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Run script" tabindex="-1">
    <h3>Run script</h3>
    <p class="file" title={path}>{fileName}</p>
    <p class="muted">
      {#if target}
        Starts on the focused tab <strong>{target}</strong>; it can also open saved sessions.
      {:else}
        No tab is focused — the script can only work on saved sessions it opens.
      {/if}
    </p>

    <fieldset class="modes">
      <legend class="sr-only">Mode</legend>
      <label class="check">
        <input type="radio" name="mode" value="dry" bind:group={mode} />
        <span>
          <strong>Dry run</strong> — nothing is sent or opened; every action is listed instead.
        </span>
      </label>
      <label class="check">
        <input type="radio" name="mode" value="read-only" bind:group={mode} />
        <span>
          <strong>Read-only</strong> — logs in and sends only commands starting with:
        </span>
      </label>
      {#if mode === 'read-only'}
        <input
          class="allow"
          bind:value={allowText}
          spellcheck="false"
          aria-label="Allowed command prefixes"
          placeholder="get, show"
        />
        <p class="muted indent">
          Anything else is listed as "would run" instead of sent, so the script sees real output
          without changing the device.
        </p>
      {/if}
      <label class="check">
        <input type="radio" name="mode" value="real" bind:group={mode} />
        <span><strong>Run</strong> — every command is sent.</span>
      </label>
    </fieldset>

    <label class="python">
      <span>Python</span>
      <input
        placeholder="auto (python3 / python / py -3)"
        bind:value={python}
        onchange={() => check(python.trim() || null)}
        spellcheck="false"
      />
    </label>
    {#if checking}
      <p class="muted small">Checking…</p>
    {:else if pythonInfo}
      <p class="small" class:err={!pythonInfo.ok}>{pythonInfo.text}</p>
    {/if}

    <div class="actions">
      <button class="cancel" onclick={onCancel}>Cancel</button>
      <button class="run" onclick={run} disabled={!pythonInfo?.ok}>
        {mode === 'dry' ? 'Dry run' : mode === 'read-only' ? 'Run read-only' : 'Run'}
      </button>
    </div>
  </div>
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
    width: 30rem;
    max-width: 92vw;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    padding: var(--zx-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-2);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
  }
  h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .file {
    margin: 0;
    font-family: var(--zx-font-mono);
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    margin: 0;
    color: var(--zx-text-muted);
    font-size: 0.8rem;
    line-height: 1.4;
  }
  .small {
    margin: 0;
    font-size: 0.75rem;
    color: var(--zx-text-muted);
    font-family: var(--zx-font-mono);
  }
  .small.err {
    color: var(--zx-err);
    font-family: var(--zx-font-ui);
  }
  .check {
    display: flex;
    gap: var(--zx-space-2);
    align-items: flex-start;
    font-size: 0.8rem;
    line-height: 1.4;
    cursor: pointer;
  }
  .python {
    display: flex;
    gap: var(--zx-space-2);
    align-items: center;
    font-size: 0.8rem;
    color: var(--zx-text-muted);
  }
  .python input {
    flex: 1;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-2);
    color: var(--zx-text);
    font-family: var(--zx-font-mono);
    font-size: 0.8rem;
    outline: none;
  }
  .python input:focus-visible {
    border-color: var(--zx-accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--zx-space-1);
    margin-top: var(--zx-space-1);
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
    color: var(--zx-text-muted);
  }
  .run {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
  .run:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .modes {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .allow {
    margin-left: 1.6rem;
    font-family: var(--zx-font-mono);
    font-size: 0.8rem;
  }
  .indent {
    margin: 0 0 0 1.6rem;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
