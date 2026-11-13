<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import { getSetting, setSetting, openLogsDir } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'

  type Format = 'plain' | 'raw'
  let format = $state<Format>('plain')
  let logsPath = $state<string | null>(null)

  onMount(async () => {
    try {
      const v = await getSetting('logging.format')
      if (v === 'raw') format = 'raw'
    } catch {
      // keep default
    }
  })

  async function update(next: Format) {
    format = next
    try {
      await setSetting('logging.format', next)
    } catch (e) {
      showToast({ kind: 'error', title: 'Setting', detail: errorText(e) })
    }
  }

  async function reveal() {
    try {
      logsPath = await openLogsDir()
    } catch (e) {
      showToast({ kind: 'error', title: 'Logs', detail: errorText(e) })
    }
  }
</script>

<div class="panel">
  <section>
    <h3>File format</h3>
    <p class="hint">
      Controls how the terminal content is saved when you toggle the record button (●) in a pane
      header.
    </p>

    <label class="row">
      <input
        type="radio"
        name="logfmt"
        value="plain"
        checked={format === 'plain'}
        onchange={() => update('plain')}
      />
      <div>
        <div class="row-title">Plain text <span class="default-tag">default</span></div>
        <div class="row-help">
          Strips ANSI codes (colors, window titles, paste modes) and keeps only text. Same as
          SecureCRT's <em>Log Session</em> option. Recommended for auditing and for opening the log in
          any editor.
        </div>
      </div>
    </label>

    <label class="row">
      <input
        type="radio"
        name="logfmt"
        value="raw"
        checked={format === 'raw'}
        onchange={() => update('raw')}
      />
      <div>
        <div class="row-title">Raw</div>
        <div class="row-help">
          Saves the exact bytes the remote sends, including escape sequences. Useful to replay the
          session with <code>cat logfile</code> in another terminal, or to debug rendering issues.
        </div>
      </div>
    </label>
  </section>

  <section>
    <h3>Location</h3>
    <p class="hint">Files rotate automatically every 50&nbsp;MB.</p>
    <button type="button" class="ok-btn" onclick={reveal}>📂 Open logs folder</button>
    {#if logsPath}
      <code class="path">{logsPath}</code>
    {/if}
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
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0 0 0.25rem;
    line-height: 1.5;
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.45rem 0.6rem;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    cursor: pointer;
  }
  .row input {
    accent-color: var(--zx-accent);
    margin-top: 3px;
  }
  .row-title {
    font-size: 0.82rem;
    color: var(--zx-text);
  }
  .row-help {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    line-height: 1.45;
    margin-top: 0.2rem;
  }
  .row-help code {
    background: color-mix(in srgb, var(--zx-accent) 10%, transparent);
    color: var(--zx-accent);
    padding: 0.02rem 0.3rem;
    border-radius: 3px;
    font-size: 0.68rem;
  }
  .default-tag {
    display: inline-block;
    margin-left: 0.35rem;
    padding: 0.03rem 0.35rem;
    background: color-mix(in srgb, var(--zx-accent) 12%, transparent);
    color: var(--zx-accent);
    font-size: 0.6rem;
    border-radius: 0.5rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: 600;
    vertical-align: middle;
  }
  .ok-btn {
    align-self: flex-start;
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    border: 0;
    border-radius: 4px;
    font-size: 0.78rem;
    font-family: inherit;
    font-weight: 600;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }
  .ok-btn:hover {
    filter: brightness(1.1);
  }
  .path {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    padding: 0.3rem 0.5rem;
    border-radius: 3px;
    word-break: break-all;
  }
</style>
