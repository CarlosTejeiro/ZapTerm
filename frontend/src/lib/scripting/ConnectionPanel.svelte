<script lang="ts">
  /// The panel of an automation connection: one face for an AI assistant
  /// (MCP), one for notebooks. It says who is connected, lets the user choose
  /// what that client may send, shows how to connect (once), and lists what
  /// has been sent since the connection was turned on. Non-modal: the tabs
  /// the client drives stay visible.
  import Icon from '$lib/icons/Icon.svelte'
  import type { IconName } from '$lib/icons/icons'
  import { activity, showActivity } from '$lib/activity/activity.svelte'
  import {
    actionTag,
    clockOf,
    isRow,
    outcomeOf,
    RESULT_LABEL,
    type ActivityEntry,
    type Outcome,
  } from '$lib/activity/activityView'
  import {
    permission,
    scriptRun,
    setPermission,
    startNotebookFromSettings,
    stopNotebook,
    type Permission,
  } from './scriptHost.svelte'

  let collapsed = $state(false)
  let setupOpen = $state(true)
  let copied = $state(false)
  let listEl = $state<HTMLDivElement | null>(null)

  const assistant = $derived(scriptRun.audience === 'assistant')
  const on = $derived(scriptRun.status === 'waiting' || scriptRun.status === 'connected')
  const connected = $derived(scriptRun.status === 'connected')

  const title = $derived(assistant ? 'AI assistant' : 'Notebook connection')
  const status = $derived.by<{ text: string; tone: 'off' | 'wait' | 'ok' | 'warn' }>(() => {
    if (!on) return { text: 'Off', tone: 'off' }
    if (!connected) {
      return {
        text: assistant ? 'Waiting for an assistant…' : 'Waiting for a notebook…',
        tone: 'wait',
      }
    }
    if (scriptRun.rejected) return { text: 'Refused a client', tone: 'warn' }
    const c = scriptRun.client
    if (c?.kind === 'unknown') return { text: 'Unidentified client connected', tone: 'warn' }
    if (c?.name) return { text: `Connected · ${c.name}`, tone: 'ok' }
    return { text: connected ? 'Connected' : '', tone: 'ok' }
  })

  const LEVELS: { id: Permission; label: string; icon: IconName }[] = [
    { id: 'read', label: 'Read-only', icon: 'shield' },
    { id: 'ask', label: 'Ask me first', icon: 'check' },
    { id: 'free', label: 'No questions', icon: 'bolt' },
  ]
  const level = $derived(permission())
  const levelHelp = $derived(
    level === 'read'
      ? `Only ${scriptRun.allow.join(', ')} commands reach a device. Anything else is refused, and ${assistant ? 'the assistant' : 'the notebook'} is told so.`
      : level === 'ask'
        ? `${scriptRun.allow.join(', ')} commands go straight through. Before any other command, ZapTerm shows it to you and waits for your Yes.`
        : 'Every command is sent as it comes, without asking. Only for work you would hand over without watching.',
  )

  /** What to paste: the MCP client's configuration, or the notebook's first cell. */
  const snippet = $derived.by(() => {
    if (!assistant) {
      return (
        `import sys; sys.path.insert(0, r"${scriptRun.moduleDir}")\n` +
        `import zapterm; zapterm.connect()`
      )
    }
    const sep = scriptRun.moduleDir.includes('\\') ? '\\' : '/'
    const python = /Windows/i.test(navigator.userAgent) ? 'python' : 'python3'
    return JSON.stringify(
      {
        mcpServers: {
          zapterm: { command: python, args: [`${scriptRun.moduleDir}${sep}zapterm_mcp.py`] },
        },
      },
      null,
      2,
    )
  })
  async function copySnippet() {
    try {
      await navigator.clipboard.writeText(snippet)
      copied = true
      setTimeout(() => (copied = false), 1500)
    } catch {
      /* clipboard unavailable: the text is on screen to copy by hand */
    }
  }

  // Once a client has connected, how to connect is no longer the point.
  let everConnected = false
  $effect(() => {
    if (connected && !everConnected) {
      everConnected = true
      setupOpen = false
    }
    if (!on) everConnected = false
  })

  /** What was recorded since this connection was turned on, oldest first. */
  const events = $derived(
    activity.recent.filter((r) => r.seq > scriptRun.sinceSeq).map((r) => r.entry),
  )
  const sent = $derived(events.filter(isRow).length)

  // Follow the list as it grows (unless the user scrolled up to read).
  $effect(() => {
    void events.length
    const el = listEl
    if (!el) return
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 48
    if (nearBottom) queueMicrotask(() => (el.scrollTop = el.scrollHeight))
  })

  const OUTCOME_ICON: Record<Outcome, IconName> = {
    sent: 'check',
    blocked: 'shield',
    declined: 'ban',
    failed: 'alert',
  }
  /** A connection event in words: who it was, by kind and by its own name. */
  function boundaryText(e: ActivityEntry): string {
    const who =
      e.source === 'mcp'
        ? e.client || 'An assistant'
        : e.source === 'unknown'
          ? 'An unidentified client'
          : `A notebook${e.client ? ` (${e.client})` : ''}`
    if (e.action === 'connect')
      return e.result === 'refused' ? `${who} was refused` : `${who} connected`
    if (e.action === 'disconnect') return `${who} disconnected`
    return e.action
  }
</script>

<div class="panel" class:collapsed role="region" aria-label={title}>
  <header class="head">
    <span class="who" class:assistant><Icon name={assistant ? 'spark' : 'book'} size={14} /></span>
    <span class="title">{title}</span>
    {#if assistant}<span class="tag">MCP</span>{/if}
    <span class="status {status.tone}"><span class="dot"></span>{status.text}</span>
    <span class="spacer"></span>
    {#if on}
      <button class="btn stop" onclick={() => stopNotebook()}>Turn off</button>
    {:else}
      <button
        class="btn primary"
        onclick={() => void startNotebookFromSettings(scriptRun.audience).catch(console.error)}
        >Turn on</button
      >
    {/if}
    <button
      class="icon"
      onclick={() => (collapsed = !collapsed)}
      title={collapsed ? 'Expand' : 'Collapse'}
      aria-label={collapsed ? 'Expand' : 'Collapse'}
      ><Icon name="chevron" size={12} open={!collapsed} /></button
    >
    {#if !on}
      <button
        class="icon"
        onclick={() => (scriptRun.visible = false)}
        title="Close"
        aria-label="Close"><Icon name="x" size={12} /></button
      >
    {/if}
  </header>

  {#if !collapsed}
    <div class="body">
      {#if scriptRun.notice}
        <p class="notice"><Icon name="alert" size={13} /> <span>{scriptRun.notice}</span></p>
      {/if}

      <section>
        <h4>What {assistant ? 'the assistant' : 'a notebook'} may send</h4>
        <div class="seg" role="radiogroup" aria-label="What may be sent">
          {#each LEVELS as l (l.id)}
            <button
              role="radio"
              aria-checked={level === l.id}
              class:on={level === l.id}
              class:free={l.id === 'free'}
              onclick={() => setPermission(l.id)}><Icon name={l.icon} size={12} />{l.label}</button
            >
          {/each}
        </div>
        <p class="help">{levelHelp}</p>
      </section>

      {#if on && scriptRun.moduleDir}
        <details class="setup" bind:open={setupOpen}>
          <summary>
            <Icon name="chevron" size={11} open={setupOpen} />
            {assistant ? 'Set up your assistant (once)' : 'Connect a notebook'}
          </summary>
          {#if assistant}
            <ol>
              <li>
                Add this to your assistant's MCP configuration (Claude Desktop, Claude Code or any
                other MCP client):
              </li>
            </ol>
          {:else}
            <p class="help">
              In the first cell of your notebook (VS Code, JupyterLab) or at a <code>python</code> prompt:
            </p>
          {/if}
          <div class="snippet">
            <pre>{snippet}</pre>
            <button class="btn" onclick={copySnippet}>{copied ? 'Copied' : 'Copy'}</button>
          </div>
          {#if assistant}
            <ol start="2">
              <li>Quit the assistant completely and open it again.</li>
              <li>Ask it something about a device: it shows up here.</li>
            </ol>
            <p class="help">
              What your devices print is sent to that assistant, and so to its provider. Passwords
              are not: ZapTerm logs in from its vault.
            </p>
          {:else}
            <p class="help">
              Then <code>zapterm.tab("name")</code> for an open tab or
              <code>zapterm.open("saved session")</code>.
            </p>
          {/if}
        </details>
      {/if}

      <section class="live">
        <div class="live-head">
          <h4>Since it was turned on</h4>
          <span class="count">{sent} action{sent === 1 ? '' : 's'}</span>
          <span class="spacer"></span>
          <button class="link" onclick={showActivity} title="Everything, with filters and search"
            ><Icon name="pulse" size={12} /> All activity</button
          >
        </div>
        <div class="list" bind:this={listEl}>
          {#if events.length === 0}
            <p class="empty">
              {on
                ? `Nothing yet. What ${assistant ? 'the assistant' : 'the notebook'} sends shows up here, as it happens.`
                : 'The connection is off.'}
            </p>
          {:else}
            {#each events as e, i (i)}
              {#if isRow(e)}
                <div class="row {outcomeOf(e)}" title={RESULT_LABEL[e.result] ?? e.result}>
                  <span class="time">{clockOf(e.time)}</span>
                  <span class="device" title={e.address || e.device}>{e.device || '—'}</span>
                  <span class="cmd" title={e.detail}>
                    {#if actionTag(e)}<span class="rtag">{actionTag(e)}</span>{/if}{e.detail}
                  </span>
                  <span class="mark {outcomeOf(e)}"
                    ><Icon name={OUTCOME_ICON[outcomeOf(e)]} size={11} /></span
                  >
                </div>
              {:else}
                <div class="edge" class:refused={e.result === 'refused'}>
                  <span class="time">{clockOf(e.time)}</span>
                  <span>{boundaryText(e)}</span>
                </div>
              {/if}
            {/each}
          {/if}
        </div>
      </section>

      {#if on && scriptRun.idleMinutes > 0}
        <p class="foot">
          Only on this computer. Turns itself off after {scriptRun.idleMinutes} min without calls.
        </p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .panel {
    position: fixed;
    right: 14px;
    bottom: 40px;
    width: 600px;
    max-width: calc(100vw - 28px);
    max-height: 72vh;
    display: flex;
    flex-direction: column;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 12px;
    box-shadow: var(--zx-shadow);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    z-index: 900;
    overflow: hidden;
    --line: color-mix(in srgb, var(--zx-text) 9%, transparent);
    --fill: color-mix(in srgb, var(--zx-text) 4%, transparent);
  }
  .spacer {
    flex: 1;
  }
  h4,
  p,
  ol {
    margin: 0;
  }
  button {
    font-family: inherit;
    color: inherit;
  }

  /* ── header ─────────────────────────────────────────────────────────── */
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 10px 9px 12px;
    font-size: 12px;
    min-width: 0;
  }
  .panel:not(.collapsed) .head {
    border-bottom: 1px solid var(--line);
  }
  .who {
    width: 1.6rem;
    height: 1.6rem;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: var(--zx-accent-2);
    background: color-mix(in srgb, var(--zx-accent-2) 16%, transparent);
  }
  .who.assistant {
    color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 16%, transparent);
  }
  .title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .tag {
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--zx-text-muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 0 5px;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status.ok {
    color: var(--zx-text);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: color-mix(in srgb, var(--zx-text) 30%, transparent);
  }
  .status.wait .dot {
    background: var(--zx-warn);
    animation: breathe 1.6s ease-in-out infinite;
  }
  .status.ok .dot {
    background: var(--zx-ok);
  }
  .status.warn .dot {
    background: var(--zx-warn);
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .status.wait .dot {
      animation: none;
    }
  }
  .btn {
    all: unset;
    cursor: pointer;
    font-size: 11.5px;
    padding: 3px 10px;
    border: 1px solid var(--zx-border);
    border-radius: 6px;
    white-space: nowrap;
    color: var(--zx-text);
  }
  .btn:hover {
    background: var(--fill);
  }
  .btn.stop {
    color: var(--zx-err);
    border-color: color-mix(in srgb, var(--zx-err) 55%, transparent);
  }
  .btn.primary {
    color: var(--zx-accent);
    border-color: var(--zx-accent);
  }
  .icon {
    all: unset;
    cursor: pointer;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    color: var(--zx-text-muted);
  }
  .icon:hover {
    color: var(--zx-text);
    background: var(--fill);
  }
  .btn:focus-visible,
  .icon:focus-visible,
  .seg button:focus-visible,
  .link:focus-visible,
  summary:focus-visible {
    outline: none;
    box-shadow: var(--zx-ring);
  }

  /* ── body ───────────────────────────────────────────────────────────── */
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    min-height: 0;
    overflow-y: auto;
  }
  h4 {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--zx-text-muted);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-height: 0;
  }
  .help {
    font-size: 12px;
    line-height: 1.45;
    color: var(--zx-text-muted);
  }
  code {
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    color: var(--zx-text);
  }
  .notice {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 12px;
    line-height: 1.45;
    color: var(--zx-text);
    background: color-mix(in srgb, var(--zx-warn) 13%, transparent);
    border: 1px solid color-mix(in srgb, var(--zx-warn) 40%, transparent);
  }
  .notice :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--zx-warn);
  }

  /* The three levels, as one control. */
  .seg {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 2px;
    padding: 2px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--zx-text) 7%, transparent);
  }
  .seg button {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 7px;
    font-size: 12px;
    color: var(--zx-text-muted);
    white-space: nowrap;
  }
  .seg button:hover {
    color: var(--zx-text);
  }
  .seg button.on {
    color: var(--zx-text);
    font-weight: 600;
    background: var(--zx-surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  }
  .seg button.on :global(svg) {
    color: var(--zx-ok);
  }
  .seg button.free.on :global(svg) {
    color: var(--zx-warn);
  }

  /* How to connect. */
  .setup {
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--fill);
    padding: 0 10px;
  }
  .setup[open] {
    padding-bottom: 10px;
  }
  summary {
    cursor: pointer;
    list-style: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 0;
    font-size: 12px;
    font-weight: 600;
    border-radius: 6px;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  ol {
    padding-left: 1.3rem;
    font-size: 12px;
    line-height: 1.5;
    color: var(--zx-text-muted);
  }
  .setup > * + * {
    margin-top: 7px;
  }
  .snippet {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }
  .snippet pre {
    flex: 1;
    min-width: 0;
    margin: 0;
    padding: 8px 10px;
    border-radius: 7px;
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
    max-height: 9.5rem;
    overflow-y: auto;
  }

  /* What was sent, live. */
  .live {
    flex: 1;
  }
  .live-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .count {
    font-size: 11.5px;
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .link {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: var(--zx-accent);
    border-radius: 4px;
  }
  .link:hover {
    text-decoration: underline;
  }
  .list {
    min-height: 4.5rem;
    max-height: 14rem;
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--fill);
  }
  .empty {
    padding: 14px 12px;
    font-size: 12px;
    color: var(--zx-text-muted);
  }
  .row,
  .edge {
    display: grid;
    align-items: center;
    gap: 10px;
    padding: 4px 10px;
    font-size: 12px;
    border-left: 2px solid transparent;
  }
  .row {
    grid-template-columns: 4.2rem minmax(4rem, 8.5rem) minmax(0, 1fr) auto;
  }
  .row + .row,
  .row + .edge,
  .edge + .row,
  .edge + .edge {
    border-top: 1px solid color-mix(in srgb, var(--zx-text) 5%, transparent);
  }
  .row.blocked {
    border-left-color: color-mix(in srgb, var(--zx-warn) 70%, transparent);
  }
  .row.declined,
  .row.failed {
    border-left-color: color-mix(in srgb, var(--zx-err) 70%, transparent);
  }
  .edge {
    grid-template-columns: 4.2rem minmax(0, 1fr);
    color: var(--zx-text-muted);
    font-size: 11.5px;
  }
  .edge.refused {
    color: var(--zx-text);
    border-left-color: color-mix(in srgb, var(--zx-warn) 70%, transparent);
  }
  .time {
    font-family: var(--zx-font-mono);
    font-size: 11px;
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .device {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmd {
    font-family: var(--zx-font-mono);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.blocked .cmd,
  .row.declined .cmd {
    color: var(--zx-text-muted);
  }
  .rtag {
    font-family: var(--zx-font-ui);
    font-size: 10px;
    color: var(--zx-text-muted);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    margin-right: 6px;
  }
  .mark {
    display: inline-grid;
    place-items: center;
    width: 1.05rem;
    height: 1.05rem;
    border-radius: 50%;
  }
  .mark.sent {
    color: var(--zx-ok);
    background: color-mix(in srgb, var(--zx-ok) 16%, transparent);
  }
  .mark.blocked {
    color: var(--zx-warn);
    background: color-mix(in srgb, var(--zx-warn) 18%, transparent);
  }
  .mark.declined,
  .mark.failed {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 16%, transparent);
  }
  .foot {
    font-size: 11px;
    color: var(--zx-text-muted);
  }
</style>
