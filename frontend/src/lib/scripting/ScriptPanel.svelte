<script lang="ts">
  /// Floating panel for the current Python script run: status, the summary
  /// rows the script reported, and its live log. Non-modal, so the terminal
  /// tabs the script is driving stay visible and usable.
  import Icon from '$lib/icons/Icon.svelte'
  import {
    scriptRun,
    startNotebookFromSettings,
    startScript,
    stopNotebook,
    stopScript,
  } from './scriptHost.svelte'
  import { setSetting } from '$lib/bridge/commands'

  let logEl = $state<HTMLPreElement | null>(null)
  let collapsed = $state(false)
  let copied = $state(false)

  const notebook = $derived(scriptRun.mode === 'notebook')
  const notebookOn = $derived(
    notebook && (scriptRun.status === 'waiting' || scriptRun.status === 'connected'),
  )
  const fileName = $derived(
    notebook ? 'Notebook connection' : (scriptRun.path.split(/[\\/]/).pop() ?? scriptRun.path),
  )
  const statusLabel = $derived(
    scriptRun.status === 'running'
      ? scriptRun.dryRun
        ? 'Dry run…'
        : 'Running…'
      : scriptRun.status === 'done'
        ? 'Finished'
        : scriptRun.status === 'stopped'
          ? 'Stopped'
          : scriptRun.status === 'waiting'
            ? 'Waiting for a notebook…'
            : scriptRun.status === 'connected'
              ? 'Notebook connected'
              : scriptRun.status === 'off'
                ? 'Off'
                : 'Failed',
  )
  /** What to paste in the notebook's first cell. */
  const connectSnippet = $derived(
    `import sys; sys.path.insert(0, r"${scriptRun.moduleDir}")\n` +
      `import zapterm; zapterm.connect()`,
  )

  async function copySnippet() {
    try {
      await navigator.clipboard.writeText(connectSnippet)
      copied = true
      setTimeout(() => (copied = false), 1500)
    } catch {
      /* clipboard unavailable: the text is selectable */
    }
  }

  // Follow the log as it grows (unless the user scrolled up to read).
  $effect(() => {
    void scriptRun.log.length
    const el = logEl
    if (!el) return
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
    if (nearBottom) queueMicrotask(() => (el.scrollTop = el.scrollHeight))
  })

  function rerun(dryRun: boolean, readOnly: boolean) {
    void startScript(scriptRun.path, dryRun, readOnly, scriptRun.allow)
  }

  /// Notebook mode: read-only can be switched live; remembered for next time.
  function setNotebookReadOnly(on: boolean) {
    scriptRun.readOnly = on
    void setSetting('notebook.read_only', on ? '1' : '0').catch(() => {})
  }
</script>

<div class="panel" class:collapsed role="region" aria-label="Script run">
  <div class="head">
    <span class="dot {scriptRun.status}"></span>
    <span class="title" title={scriptRun.path}>{fileName}</span>
    <span class="status">{statusLabel}</span>
    {#if scriptRun.dryRun}<span class="badge">dry run</span>{/if}
    {#if scriptRun.readOnly}<span
        class="badge ro"
        title={`Only ${scriptRun.allow.join(', ')} commands are sent`}>read-only</span
      >{/if}
    <span class="spacer"></span>
    {#if notebook}
      {#if notebookOn}
        <button class="btn stop" onclick={() => stopNotebook()}>Turn off</button>
      {:else}
        <button class="btn" onclick={() => void startNotebookFromSettings().catch(console.error)}
          >Turn on</button
        >
      {/if}
    {:else if scriptRun.status === 'running'}
      <button class="btn stop" onclick={() => stopScript()}>Stop</button>
    {:else}
      {#if scriptRun.dryRun || scriptRun.readOnly}
        <button
          class="btn"
          onclick={() => rerun(false, false)}
          title="Run it again, sending every command">Run for real</button
        >
      {/if}
      <button class="btn" onclick={() => rerun(scriptRun.dryRun, scriptRun.readOnly)}
        >Run again</button
      >
    {/if}
    <button
      class="icon"
      onclick={() => (collapsed = !collapsed)}
      title={collapsed ? 'Expand' : 'Collapse'}
      aria-label={collapsed ? 'Expand' : 'Collapse'}
      ><Icon name="chevron" size={12} open={!collapsed} /></button
    >
    {#if scriptRun.status !== 'running' && !notebookOn}
      <button
        class="icon"
        onclick={() => (scriptRun.visible = false)}
        title="Close"
        aria-label="Close"><Icon name="x" size={12} /></button
      >
    {/if}
  </div>

  {#if !collapsed}
    {#if notebookOn && scriptRun.moduleDir}
      <div class="connect">
        <label class="ro-toggle">
          <input
            type="checkbox"
            checked={scriptRun.readOnly}
            onchange={(e) => setNotebookReadOnly(e.currentTarget.checked)}
          />
          <span>
            <strong>Read-only</strong> — only <code>{scriptRun.allow.join(', ')}</code> commands are sent;
            anything else is listed as "would run".
          </span>
        </label>
        <p>
          In your notebook (VS Code, JupyterLab) or a <code>python</code> prompt, run:
        </p>
        <div class="snippet">
          <pre>{connectSnippet}</pre>
          <button class="btn" onclick={copySnippet}>{copied ? 'Copied' : 'Copy'}</button>
        </div>
        <p class="hint">
          Then <code>zapterm.tab("name")</code> for an open tab or
          <code>zapterm.open("saved session")</code>.
          {#if scriptRun.idleMinutes > 0}Turns itself off after {scriptRun.idleMinutes} min without notebook
            calls.{/if}
        </p>
      </div>
    {/if}
    {#if scriptRun.report.length > 0}
      <table class="report">
        <tbody>
          {#each scriptRun.report as row, i (i)}
            <tr>
              <td class="target">{row.target}</td>
              <td
                class="st"
                class:ok={/^ok\b/i.test(row.status)}
                class:bad={/^(fail|not\b|error)/i.test(row.status)}>{row.status}</td
              >
              <td class="detail">{row.detail}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    <pre class="log" bind:this={logEl}>{#each scriptRun.log as l, i (i)}<span class={l.stream}
          >{l.line}
</span>{/each}</pre>
    {#if scriptRun.python}
      <p class="foot">{scriptRun.python}</p>
    {:else if notebookOn && scriptRun.connectionFile}
      <p class="foot" title="zapterm.connect(path=…) if it isn't found">
        {scriptRun.connectionFile}
      </p>
    {/if}
  {/if}
</div>

<style>
  .panel {
    position: fixed;
    right: 14px;
    bottom: 40px;
    width: 560px;
    max-width: calc(100vw - 28px);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    box-shadow: var(--zx-shadow);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    z-index: 900;
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--zx-border);
    font-size: 12px;
  }
  .collapsed .head {
    border-bottom: none;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--zx-text-dim);
  }
  .dot.running,
  .dot.waiting {
    background: var(--zx-warn);
  }
  .dot.connected {
    background: var(--zx-ok);
  }
  .dot.done {
    background: var(--zx-ok);
  }
  .dot.failed {
    background: var(--zx-err);
  }
  .title {
    font-family: var(--zx-font-mono);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 14rem;
  }
  .status {
    color: var(--zx-text-muted);
  }
  .badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--zx-accent) 15%, transparent);
    color: var(--zx-accent);
  }
  .spacer {
    flex: 1;
  }
  .btn {
    border: 1px solid var(--zx-border);
    background: transparent;
    color: var(--zx-text-muted);
    border-radius: 5px;
    padding: 2px 8px;
    font-size: 11.5px;
    font-family: inherit;
    cursor: pointer;
  }
  .btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .btn.stop {
    color: var(--zx-err);
    border-color: color-mix(in srgb, var(--zx-err) 45%, transparent);
  }
  .icon {
    background: transparent;
    border: none;
    color: var(--zx-text-dim);
    cursor: pointer;
    padding: 2px;
    display: flex;
  }
  .icon:hover {
    color: var(--zx-text);
  }
  .connect {
    margin: 8px 10px 0;
    font-size: 12px;
    color: var(--zx-text-muted);
  }
  .connect p {
    margin: 0 0 6px;
  }
  .connect code {
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    color: var(--zx-text);
  }
  .snippet {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-bottom: 6px;
  }
  .snippet pre {
    flex: 1;
    margin: 0;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  .hint {
    font-size: 11.5px;
  }
  .report {
    border-collapse: collapse;
    margin: 6px 10px 0;
    font-size: 12px;
  }
  .report td {
    padding: 2px 8px 2px 0;
    vertical-align: top;
  }
  .target {
    font-family: var(--zx-font-mono);
    white-space: nowrap;
  }
  .st {
    font-weight: 600;
    white-space: nowrap;
  }
  .st.ok {
    color: var(--zx-ok);
  }
  .st.bad {
    color: var(--zx-err);
  }
  .detail {
    color: var(--zx-text-muted);
  }
  .log {
    flex: 1;
    min-height: 6rem;
    margin: 8px 10px;
    padding: 8px 10px;
    overflow: auto;
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    border-radius: 6px;
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .log .stderr {
    color: var(--zx-term-err);
  }
  .log .zapterm {
    color: var(--zx-term-dim);
  }
  .foot {
    margin: 0 10px 8px;
    font-size: 10.5px;
    color: var(--zx-text-dim);
    font-family: var(--zx-font-mono);
  }
  .badge.ro {
    background: color-mix(in srgb, var(--zx-warn) 18%, transparent);
    color: var(--zx-warn);
  }
  .ro-toggle {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin-bottom: 8px;
    cursor: pointer;
  }
</style>
