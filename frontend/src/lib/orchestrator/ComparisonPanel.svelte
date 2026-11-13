<script lang="ts">
  import { lineDiff, type DiffLine } from './lineDiff'
  import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager'
  import type { PylonTheme } from '$lib/themes/index'
  import type { HostCapture } from './commandRunner.svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import { showToast } from '$lib/ui/toast-store.svelte'

  interface Props {
    theme: PylonTheme
    command: string
    hosts: HostCapture[]
    running: boolean
    onClose: () => void
  }

  const { theme, command, hosts, running, onClose }: Props = $props()

  // ── normalization ─────────────────────────────────────────────────────────
  // Strip the echoed command (first line) + the trailing prompt line, collapse
  // CRLF, trim per-line trailing whitespace, so "byte-identical" means
  // "the meaningful output matched", not "the prompt string matched".
  function normalize(buffer: string): string {
    let lines = buffer.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n')
    // Drop the first line if it's the echoed command (possibly with a prompt
    // prefix ending in the command text).
    if (lines.length && command && lines[0]!.includes(command)) lines = lines.slice(1)
    // Drop a trailing prompt-ish line (last non-empty line) + trailing blanks.
    while (lines.length && lines[lines.length - 1]!.trim() === '') lines.pop()
    if (lines.length) lines = lines.slice(0, -1) // the returned prompt line
    return lines
      .map((l) => l.replace(/\s+$/, ''))
      .join('\n')
      .trim()
  }

  interface OutputGroup {
    text: string
    hosts: HostCapture[]
  }

  const groups = $derived.by<OutputGroup[]>(() => {
    const map = new Map<string, OutputGroup>()
    for (const h of hosts) {
      // Only a genuine non-responder — timed out with nothing captured — is
      // excluded. A host that captured output is grouped even if we never
      // confirmed completion (prompt unrecognised), so its output isn't lost.
      if (h.state === 'timeout' && normalize(h.buffer) === '') continue
      const text = normalize(h.buffer)
      const g = map.get(text)
      if (g) g.hosts.push(h)
      else map.set(text, { text, hosts: [h] })
    }
    return [...map.values()].sort((a, b) => b.hosts.length - a.hosts.length)
  })

  const timedOut = $derived(
    hosts.filter((h) => h.state === 'timeout' && normalize(h.buffer) === ''),
  )
  const allSame = $derived(!running && groups.length === 1 && timedOut.length === 0)

  // Expand/collapse a group's output.
  let open = $state<Set<number>>(new Set([0]))
  function toggle(i: number) {
    const next = new Set(open)
    if (next.has(i)) next.delete(i)
    else next.add(i)
    open = next
  }

  // ── diff between two groups ────────────────────────────────────────────────
  let diffA = $state<number | null>(null)
  let diffB = $state<number | null>(null)

  const diff = $derived.by<DiffLine[] | null>(() => {
    if (diffA == null || diffB == null) return null
    const ga = groups[diffA]
    const gb = groups[diffB]
    if (!ga || !gb) return null
    return lineDiff(ga.text, gb.text)
  })

  function pickDiff(i: number) {
    if (diffA === i) {
      diffA = null
      return
    }
    if (diffB === i) {
      diffB = null
      return
    }
    if (diffA == null) diffA = i
    else if (diffB == null) diffB = i
    else {
      diffA = i
      diffB = null
    }
  }

  // ── export ─────────────────────────────────────────────────────────────────
  /// Assemble a plain-text audit report: the command, a one-line summary, each
  /// output variant with the hosts that produced it, and any non-responders.
  /// Suitable for pasting straight into a ticket or change record.
  function buildReport(): string {
    const out: string[] = []
    out.push(`$ ${command || '—'}`)
    out.push(
      `# ${new Date().toISOString().replace('T', ' ').slice(0, 19)} · ${hosts.length} hosts · ${groups.length} variant${groups.length === 1 ? '' : 's'}${timedOut.length ? ` · ${timedOut.length} no reply` : ''}`,
    )
    for (const g of groups) {
      out.push('')
      out.push(
        `── ${g.hosts.length} host${g.hosts.length === 1 ? '' : 's'}: ${g.hosts.map((h) => h.label).join(', ')} ──`,
      )
      out.push(g.text || '(no output)')
    }
    if (timedOut.length) {
      out.push('')
      out.push(`── No reply (${timedOut.length}): ${timedOut.map((h) => h.label).join(', ')} ──`)
    }
    return out.join('\n') + '\n'
  }

  async function copyReport() {
    const report = buildReport()
    try {
      await clipboardWriteText(report)
    } catch {
      await navigator.clipboard.writeText(report).catch(() => {})
    }
    showToast({ kind: 'success', title: 'Comparison copied', detail: `${hosts.length} hosts` })
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="panel" style:font-family={theme.fontUi}>
    <div class="header">
      <div class="title">
        <span class="cmd" style:color={theme.accent}>$ {command || '—'}</span>
        {#if running}
          <span class="badge running">running…</span>
        {:else if allSame}
          <span class="badge ok"
            ><Icon name="check" size={11} /> {hosts.length} hosts identical</span
          >
        {:else}
          <span class="badge warn"
            >{groups.length} variants{timedOut.length ? ` · ${timedOut.length} no reply` : ''}</span
          >
        {/if}
      </div>
      <div class="header-actions">
        <button
          class="btn small"
          onclick={copyReport}
          disabled={running || groups.length === 0}
          title="Copy the comparison as text"><Icon name="copy" size={12} /> Copy</button
        >
        <button class="btn" onclick={onClose} title="Close"><Icon name="x" size={12} /></button>
      </div>
    </div>

    <p class="hint">Outputs grouped by identical content. Select two groups to see the diff.</p>

    <div class="groups">
      {#each groups as g, i (i)}
        <div class="group" class:diff-a={diffA === i} class:diff-b={diffB === i}>
          <div class="group-head">
            <button class="caret" onclick={() => toggle(i)}
              ><Icon name="chevron" size={11} open={open.has(i)} /></button
            >
            <span class="count" style:background={theme.accent}>{g.hosts.length}</span>
            <span class="hostnames">{g.hosts.map((h) => h.label).join(', ')}</span>
            <button
              class="btn small"
              class:active={diffA === i || diffB === i}
              onclick={() => pickDiff(i)}
              title="Mark for diff">diff</button
            >
          </div>
          {#if open.has(i)}
            <pre class="output">{g.text || '(no output)'}</pre>
          {/if}
        </div>
      {/each}

      {#if timedOut.length}
        <div class="group timeout">
          <div class="group-head">
            <span class="count to">{timedOut.length}</span>
            <span class="hostnames">{timedOut.map((h) => h.label).join(', ')}</span>
            <span class="to-label">timed out</span>
          </div>
        </div>
      {/if}

      {#if groups.length === 0 && !running}
        <p class="empty">No host returned output.</p>
      {/if}
    </div>

    {#if diff}
      <div class="diff">
        <div class="diff-head">
          Diff: group {(diffA ?? 0) + 1} (red) ↔ group {(diffB ?? 0) + 1} (green)
        </div>
        <pre class="diff-body">{#each diff as l (l.text + l.kind)}<span class="dl {l.kind}"
              >{l.kind === 'a' ? '- ' : l.kind === 'b' ? '+ ' : '  '}{l.text}
</span>{/each}</pre>
      </div>
    {/if}
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
    z-index: 120;
  }

  .panel {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    box-shadow: var(--zx-shadow);
    color: var(--zx-text);
    padding: var(--zx-space-4) var(--zx-space-4) var(--zx-space-4);
    width: 52rem;
    max-width: 96vw;
    height: 40rem;
    max-height: 92vh;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
  }

  .cmd {
    font-family: var(--zx-font-mono);
    font-size: 0.9rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.7rem;
    font-weight: 600;
    padding: 0.1rem 0.45rem;
    border-radius: 0.6rem;
    flex-shrink: 0;
  }
  .badge.ok {
    background: color-mix(in srgb, var(--zx-ok) 15%, transparent);
    color: var(--zx-ok);
  }
  .badge.warn {
    background: color-mix(in srgb, var(--zx-warn) 15%, transparent);
    color: var(--zx-warn);
  }
  .badge.running {
    background: color-mix(in srgb, var(--zx-accent) 15%, transparent);
    color: var(--zx-accent);
  }

  .hint {
    margin: 0;
    font-size: 0.75rem;
    color: var(--zx-text-muted);
  }

  .groups {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-height: 0;
  }

  /* Each group is a small terminal card: device output reads best on the
     theme's terminal palette, in both light and dark themes. */
  .group {
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    border: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    border-radius: var(--zx-radius);
    overflow: hidden;
  }
  .group.diff-a {
    border-color: var(--zx-term-err);
  }
  .group.diff-b {
    border-color: var(--zx-term-ok);
  }
  .group.timeout {
    border-style: dashed;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
  }

  .caret {
    background: none;
    border: none;
    color: var(--zx-term-dim);
    cursor: pointer;
    padding: 0;
    display: flex;
  }
  .caret:hover {
    color: var(--zx-term-fg);
  }

  .count {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--zx-on-accent);
    border-radius: 0.5rem;
    padding: 0.05rem 0.4rem;
    flex-shrink: 0;
  }
  .count.to {
    background: color-mix(in srgb, var(--zx-term-fg) 18%, transparent);
    color: var(--zx-term-fg);
  }

  .hostnames {
    flex: 1;
    font-size: 0.78rem;
    color: var(--zx-term-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .to-label {
    font-size: 0.72rem;
    color: var(--zx-term-dim);
    flex-shrink: 0;
  }

  .output {
    margin: 0;
    padding: 0.5rem 0.7rem;
    border-top: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    font-family: var(--zx-font-mono);
    font-size: 0.75rem;
    color: var(--zx-term-fg);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 14rem;
    overflow-y: auto;
  }

  .empty {
    font-size: 0.8rem;
    color: var(--zx-text-dim);
  }

  .diff {
    background: var(--zx-term-bg);
    border: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    border-radius: var(--zx-radius);
    max-height: 12rem;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .diff-head {
    font-size: 0.72rem;
    color: var(--zx-term-dim);
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    position: sticky;
    top: 0;
    background: var(--zx-term-bg);
  }

  .diff-body {
    margin: 0;
    padding: 0.4rem 0.6rem;
    font-family: var(--zx-font-mono);
    font-size: 0.74rem;
  }

  .dl {
    display: block;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .dl.same {
    color: var(--zx-term-dim);
  }
  .dl.a {
    color: var(--zx-term-err);
    background: color-mix(in srgb, var(--zx-term-err) 10%, transparent);
  }
  .dl.b {
    color: var(--zx-term-ok);
    background: color-mix(in srgb, var(--zx-term-ok) 10%, transparent);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: transparent;
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    color: var(--zx-text-muted);
    font-size: 0.78rem;
    padding: 0.25rem 0.55rem;
    cursor: pointer;
    font-family: inherit;
    flex-shrink: 0;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn:hover:not(:disabled) {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .btn.small {
    font-size: 0.7rem;
    padding: 0.15rem 0.45rem;
  }
  /* "diff" toggles live inside the terminal-coloured group cards. */
  .group .btn {
    border-color: color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    color: var(--zx-term-dim);
  }
  .group .btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-term-fg) 8%, transparent);
    color: var(--zx-term-fg);
  }
  .btn.active,
  .group .btn.active {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
  }
</style>
