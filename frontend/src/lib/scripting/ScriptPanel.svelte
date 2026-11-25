<script lang="ts">
  /// Floating panel for the current Python script run: status, the summary
  /// rows the script reported, and its live log. Non-modal, so the terminal
  /// tabs the script is driving stay visible and usable.
  import Icon from '$lib/icons/Icon.svelte'
  import { scriptRun, startScript, stopScript } from './scriptHost.svelte'
  import { showActivity } from '$lib/activity/activity.svelte'
  import { createPanelSizer } from '$lib/ui/panelSizer.svelte'

  let logEl = $state<HTMLPreElement | null>(null)
  let collapsed = $state(false)
  let panelEl = $state<HTMLDivElement | null>(null)
  const sizer = createPanelSizer('ui.panel.script', { w: 380, h: 220 })

  const fileName = $derived(scriptRun.path.split(/[\\/]/).pop() ?? scriptRun.path)
  const statusLabel = $derived(
    scriptRun.status === 'running'
      ? scriptRun.dryRun
        ? 'Dry run…'
        : 'Running…'
      : scriptRun.status === 'done'
        ? 'Finished'
        : scriptRun.status === 'stopped'
          ? 'Stopped'
          : 'Failed',
  )

  function rerun(dryRun: boolean, readOnly: boolean) {
    void startScript(scriptRun.path, dryRun, readOnly, scriptRun.allow)
  }
</script>

<div
  class="panel"
  class:collapsed
  class:sized={!!sizer.size && !collapsed}
  class:dragging={sizer.dragging}
  role="region"
  aria-label="Script run"
  bind:this={panelEl}
  style:width={sizer.size ? `${sizer.size.w}px` : null}
  style:height={sizer.size && !collapsed ? `${sizer.size.h}px` : null}
>
  {#if !collapsed}
    <!-- Resize handles: left edge, top edge and their corner (the panel is
         anchored bottom-right). Double-click: the default size. -->
    <div
      class="grip gx"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize panel width"
      onpointerdown={(e) => sizer.startDrag(e, panelEl, 'x')}
      ondblclick={sizer.reset}
    ></div>
    <div
      class="grip gy"
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize panel height"
      onpointerdown={(e) => sizer.startDrag(e, panelEl, 'y')}
      ondblclick={sizer.reset}
    ></div>
    <div
      class="grip gxy"
      role="separator"
      aria-label="Resize panel"
      title="Drag to resize · double-click for the default size"
      onpointerdown={(e) => sizer.startDrag(e, panelEl, 'both')}
      ondblclick={sizer.reset}
    ></div>
  {/if}
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
    {#if scriptRun.status === 'running'}
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
      onclick={showActivity}
      title="Automation activity: everything sent, when and where"
      aria-label="Automation activity"><Icon name="pulse" size={12} /></button
    >
    {#if !collapsed}
      <button
        class="icon"
        onclick={sizer.toggleLarge}
        title={sizer.large ? 'Restore size' : 'Enlarge'}
        aria-label={sizer.large ? 'Restore size' : 'Enlarge'}
        aria-pressed={sizer.large}
        ><Icon name={sizer.large ? 'shrink' : 'expand'} size={12} /></button
      >
    {/if}
    <button
      class="icon"
      onclick={() => (collapsed = !collapsed)}
      title={collapsed ? 'Expand' : 'Collapse'}
      aria-label={collapsed ? 'Expand' : 'Collapse'}
      ><Icon name="chevron" size={12} open={!collapsed} /></button
    >
    {#if scriptRun.status !== 'running'}
      <button
        class="icon"
        onclick={() => (scriptRun.visible = false)}
        title="Close"
        aria-label="Close"><Icon name="x" size={12} /></button
      >
    {/if}
  </div>

  {#if !collapsed}
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
  /* ── resizing ───────────────────────────────────────────────────────── */
  .panel.sized {
    max-height: none;
  }
  .panel.dragging {
    user-select: none;
  }
  .grip {
    position: absolute;
    z-index: 2;
    touch-action: none;
  }
  .gx {
    left: 0;
    top: 0;
    bottom: 0;
    width: 5px;
    cursor: ew-resize;
  }
  .gy {
    left: 0;
    right: 0;
    top: 0;
    height: 5px;
    cursor: ns-resize;
  }
  .gxy {
    left: 0;
    top: 0;
    width: 13px;
    height: 13px;
    cursor: nwse-resize;
    z-index: 3;
  }
  .gx:hover,
  .gy:hover,
  .panel.dragging .gx,
  .panel.dragging .gy {
    background: color-mix(in srgb, var(--zx-accent) 35%, transparent);
  }
</style>
