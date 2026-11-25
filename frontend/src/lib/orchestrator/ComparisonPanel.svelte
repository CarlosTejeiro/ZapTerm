<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager'
  import type { PylonTheme } from '$lib/themes/index'
  import { MAX_PAGES, type HostCapture } from './commandRunner.svelte'
  import { compareSettings } from './compareSettings.svelte'
  import { lineDiff } from './lineDiff'
  import {
    collapseSame,
    compareOutputs,
    displayNames,
    endsAtPager,
    joinNames,
    sameDeviceSets,
    splitRows,
    type CompareHost,
    type DiffRow,
    type SplitCell,
    type Variant,
  } from './compare'
  import { setCompareBadges, clearCompareBadges, type CompareBadge } from './compareBadges.svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import { showToast } from '$lib/ui/toast-store.svelte'

  interface Props {
    theme: PylonTheme
    command: string
    hosts: HostCapture[]
    running: boolean
    /** The runner's time limit, for the "no prompt within N s" note. */
    timeoutMs: number
    onClose: () => void
  }

  const { theme, command, hosts, running, timeoutMs, onClose }: Props = $props()

  // ── what the comparison found ─────────────────────────────────────────────
  const result = $derived(compareOutputs(hosts, command))
  const variants = $derived(result.variants)
  const noReply = $derived(result.noReply)
  const names = $derived(displayNames(hosts))
  const nameOf = (h: CompareHost) => names.get(h.sessionId) ?? h.label
  const namesOf = (hs: CompareHost[]) => joinNames(hs.map(nameOf))

  /** A is the most common output only when no other version has as many
   *  panes; on a tie it is simply the version of the left-most pane. */
  const aLeads = $derived(
    variants.length > 1 && variants[0]!.hosts.length > variants[1]!.hosts.length,
  )
  const aLabel = $derived(aLeads ? 'the most common output' : 'the left-most pane')
  const sameDevice = $derived(sameDeviceSets(hosts))
  const pagered = $derived(hosts.filter((h) => h.state !== 'running' && endsAtPager(h.buffer)))
  /** What to do about a capture stuck at a pager prompt. */
  const pagerAdvice = $derived(
    !compareSettings.autoPage
      ? 'turn on “Page through --More--” below, or turn paging off on the device, and run the command again'
      : pagered.every((h) => h.pages >= MAX_PAGES)
        ? `it was paged on ${MAX_PAGES} times and still had more: turn paging off on the device to compare all of it`
        : 'turn paging off on the device and run the command again',
  )
  const timedOutWithOutput = $derived(
    hosts.filter((h) => h.state === 'timeout' && !noReply.includes(h) && !pagered.includes(h)),
  )
  const incomplete = $derived(new Set([...pagered, ...timedOutWithOutput].map((h) => h.sessionId)))
  const pending = $derived(hosts.filter((h) => h.state === 'running').length)
  const answered = $derived(hosts.length - noReply.length)

  const status = $derived.by<{ text: string; tone: 'ok' | 'warn' | 'running' }>(() => {
    if (running) {
      return {
        tone: 'running',
        text: `Waiting for ${pending} of ${hosts.length} pane${hosts.length === 1 ? '' : 's'}…`,
      }
    }
    const maybe = incomplete.size ? ' (may be incomplete)' : ''
    if (variants.length === 0) return { tone: 'warn', text: 'No pane replied' }
    if (variants.length === 1) {
      const all =
        answered === hosts.length
          ? hosts.length === 2
            ? 'both panes'
            : `all ${hosts.length} panes`
          : `${answered} of ${hosts.length} panes`
      return {
        tone: incomplete.size || noReply.length ? 'warn' : 'ok',
        text: `Same output on ${all}${maybe}`,
      }
    }
    return {
      tone: 'warn',
      text: `Output differs: ${variants.length} versions${maybe}`,
    }
  })

  // ── marks in the pane headers ─────────────────────────────────────────────
  let published: string[] = []
  $effect(() => {
    const entries: Record<string, CompareBadge> = {}
    const several = variants.length > 1
    for (const h of hosts) {
      const inc = incomplete.has(h.sessionId)
      const note = inc
        ? endsAtPager(h.buffer)
          ? ' — stopped at a --More-- prompt'
          : ' — no prompt before the time limit'
        : ''
      if (h.state === 'running') {
        entries[h.sessionId] = { kind: 'running', title: 'Run & compare: waiting for output' }
      } else if (noReply.includes(h)) {
        entries[h.sessionId] = { kind: 'noreply', title: 'Run & compare: no reply' }
      } else {
        const v = variants.find((x) => x.hosts.includes(h))
        if (!v) continue
        entries[h.sessionId] = several
          ? {
              kind: 'variant',
              letter: v.letter,
              incomplete: inc,
              title:
                v.letter === 'A'
                  ? `Run & compare: output A (${aLabel})${note}`
                  : `Run & compare: output ${v.letter}, differs from A${note}`,
            }
          : {
              kind: 'same',
              incomplete: inc,
              title: `Run & compare: same output as the other panes${note}`,
            }
      }
    }
    // Writing the shared store must not make this effect depend on it (it
    // reads the store to merge): that would loop.
    untrack(() => setCompareBadges(entries, published))
    published = Object.keys(entries)
  })
  onDestroy(() => untrack(() => clearCompareBadges(published)))

  // ── view state ─────────────────────────────────────────────────────────────
  let collapsed = $state(false)
  let showOutputs = $state(false)
  /** With three or more versions: which ones show their output / their diff
   *  against A. */
  let openOutput = $state<Set<string>>(new Set())
  let openDiff = $state<Set<string>>(new Set())

  function toggle(set: Set<string>, key: string): Set<string> {
    const next = new Set(set)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    return next
  }

  function diffRows(a: Variant, b: Variant): DiffRow[] {
    return collapseSame(lineDiff(a.text, b.text))
  }

  // ── copy as text ───────────────────────────────────────────────────────────
  function buildReport(): string {
    const out: string[] = []
    out.push(`$ ${command || '—'}`)
    out.push(`# ${new Date().toISOString().replace('T', ' ').slice(0, 19)} · ${status.text}`)
    for (const s of sameDevice) out.push(`# Same device (${s.target}): ${namesOf(s.hosts)}`)
    if (pagered.length) out.push(`# Stopped at a pager prompt: ${namesOf(pagered)}`)
    if (timedOutWithOutput.length) {
      out.push(
        `# No prompt within ${Math.round(timeoutMs / 1000)} s: ${namesOf(timedOutWithOutput)}`,
      )
    }
    for (const v of variants) {
      out.push('')
      out.push(`── ${variants.length > 1 ? `Output ${v.letter}: ` : ''}${namesOf(v.hosts)} ──`)
      out.push(v.text || '(no output)')
    }
    if (noReply.length) {
      out.push('')
      out.push(`── No reply: ${namesOf(noReply)} ──`)
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
    showToast({ kind: 'success', title: 'Comparison copied', detail: status.text })
  }
</script>

{#snippet diffView(a: Variant, b: Variant)}
  <div class="legend">
    <span class="lg a">− {a.letter}: {namesOf(a.hosts)}</span>
    <span class="lg b">+ {b.letter}: {namesOf(b.hosts)}</span>
    <span class="spacer"></span>
    <span class="layout" role="group" aria-label="Diff layout">
      <button
        class:on={compareSettings.diffLayout === 'split'}
        aria-pressed={compareSettings.diffLayout === 'split'}
        onclick={() => (compareSettings.diffLayout = 'split')}
        title="Side by side: the left output on the left, the right one on the right">Split</button
      ><button
        class:on={compareSettings.diffLayout === 'unified'}
        aria-pressed={compareSettings.diffLayout === 'unified'}
        onclick={() => (compareSettings.diffLayout = 'unified')}
        title="One column: removed lines (−) then added lines (+)">Unified</button
      >
    </span>
  </div>
  {#if compareSettings.diffLayout === 'split'}
    <div class="sdiff" role="table" aria-label="Differences side by side">
      {#each splitRows(diffRows(a, b)) as r, i (i)}
        {#if 'count' in r}
          <div class="sgap" role="row">
            ··· {r.count} identical line{r.count === 1 ? '' : 's'} ···
          </div>
        {:else}
          {@render half(r.left, false)}{@render half(r.right, true)}
        {/if}
      {/each}
    </div>
  {:else}
    <pre class="diff">{#each diffRows(a, b) as r, i (i)}{#if r.kind === 'gap'}<span
            class="dl gap">  ··· {r.count} identical line{r.count === 1 ? '' : 's'} ···
</span>{:else}<span class="dl {r.kind}"
            >{r.kind === 'a' ? '− ' : r.kind === 'b' ? '+ ' : '  '}{r.text}
</span>{/if}{/each}</pre>
  {/if}
{/snippet}

{#snippet half(c: SplitCell | null, right: boolean)}
  <span class="sn {c?.kind ?? 'none'}" class:r={right}>{c?.n ?? ''}</span><span
    class="st {c?.kind ?? 'none'}"
    >{c ? (c.kind === 'a' ? '− ' : c.kind === 'b' ? '+ ' : '  ') + c.text : ''}</span
  >
{/snippet}

<section class="compare" style:font-family={theme.fontUi} aria-label="Run and compare">
  <header class="head">
    <button
      class="icon"
      onclick={() => (collapsed = !collapsed)}
      title={collapsed ? 'Show the comparison' : 'Hide the comparison'}
      aria-expanded={!collapsed}><Icon name="chevron" size={11} open={!collapsed} /></button
    >
    <span class="cmd" style:color={theme.accent} title={command}>$ {command || '—'}</span>
    <span class="status {status.tone}">
      {#if status.tone === 'ok'}<Icon name="check" size={11} />{/if}
      {status.text}
    </span>
    <span class="spacer"></span>
    <button
      class="btn"
      onclick={copyReport}
      disabled={running || variants.length === 0}
      title="Copy the comparison as text"><Icon name="copy" size={12} /> Copy</button
    >
    <button class="icon" onclick={onClose} title="Close the comparison"
      ><Icon name="x" size={12} /></button
    >
  </header>

  {#if !collapsed}
    <div class="body">
      <p class="what">
        Compares what each pane printed between the command and the prompt that came back; spaces at
        line ends are ignored. The mark in each pane's header shows its result.
      </p>

      {#each sameDevice as s (s.target)}
        <p class="warn">
          ⚠ {namesOf(s.hosts)} are connected to the same device ({s.target}), so the same output is
          expected.
        </p>
      {/each}
      {#if pagered.length}
        <p class="warn">
          ⚠ The output stopped at a “--More--” prompt on {namesOf(pagered)}: {pagerAdvice}. {pagered.length ===
          1
            ? 'That pane is'
            : 'Those panes are'} still waiting at the prompt.
        </p>
      {/if}
      {#if timedOutWithOutput.length}
        <p class="warn">
          ⚠ No prompt came back within {Math.round(timeoutMs / 1000)} s on {namesOf(
            timedOutWithOutput,
          )}: the output may be incomplete.
        </p>
      {/if}

      {#if running}
        <p class="muted">
          Finished: {hosts
            .filter((h) => h.state !== 'running')
            .map(nameOf)
            .join(', ') || 'none yet'}.
        </p>
      {:else if variants.length === 1}
        <p class="muted">{namesOf(variants[0]!.hosts)} returned exactly the same text.</p>
        <button class="link" onclick={() => (showOutputs = !showOutputs)}
          >{showOutputs ? 'Hide the output' : 'Show the output'}</button
        >
        {#if showOutputs}
          <pre class="output">{variants[0]!.text || '(no output)'}</pre>
        {/if}
      {:else if variants.length === 2}
        {@render diffView(variants[0]!, variants[1]!)}
        <button class="link" onclick={() => (showOutputs = !showOutputs)}
          >{showOutputs ? 'Hide the full outputs' : 'Show the full outputs'}</button
        >
        {#if showOutputs}
          {#each variants as v (v.letter)}
            <p class="vhead"><b>{v.letter}</b> · {namesOf(v.hosts)}</p>
            <pre class="output">{v.text || '(no output)'}</pre>
          {/each}
        {/if}
      {:else if variants.length > 2}
        {#each variants as v (v.letter)}
          <div class="variant">
            <span class="letter" class:main={v.letter === 'A'}>{v.letter}</span>
            <span class="vnames">{namesOf(v.hosts)}{v.letter === 'A' ? ` — ${aLabel}` : ''}</span>
            <button class="btn small" onclick={() => (openOutput = toggle(openOutput, v.letter))}
              >{openOutput.has(v.letter) ? 'Hide output' : 'Output'}</button
            >
            {#if v.letter !== 'A'}
              <button
                class="btn small"
                class:active={openDiff.has(v.letter)}
                onclick={() => (openDiff = toggle(openDiff, v.letter))}>Compare with A</button
              >
            {/if}
          </div>
          {#if openOutput.has(v.letter)}
            <pre class="output">{v.text || '(no output)'}</pre>
          {/if}
          {#if openDiff.has(v.letter)}
            {@render diffView(variants[0]!, v)}
          {/if}
        {/each}
      {/if}

      {#if noReply.length && !running}
        <p class="muted">No reply from {namesOf(noReply)}.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .compare {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    max-height: 42%;
    min-height: 0;
    background: var(--zx-surface);
    border-top: 1px solid var(--zx-border);
    color: var(--zx-text);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    flex-shrink: 0;
  }
  .spacer {
    flex: 1;
  }

  .cmd {
    font-family: var(--zx-font-mono);
    font-size: 0.8rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    max-width: 40%;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.1rem 0.5rem;
    border-radius: 0.6rem;
    flex-shrink: 0;
  }
  .status.ok {
    background: color-mix(in srgb, var(--zx-ok) 15%, transparent);
    color: var(--zx-ok);
  }
  .status.warn {
    background: color-mix(in srgb, var(--zx-warn) 15%, transparent);
    color: var(--zx-warn);
  }
  .status.running {
    background: color-mix(in srgb, var(--zx-accent) 15%, transparent);
    color: var(--zx-accent);
  }

  .body {
    overflow-y: auto;
    padding: 0 0.75rem 0.6rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-height: 0;
  }

  p {
    margin: 0;
    font-size: 0.76rem;
    line-height: 1.45;
  }
  .what,
  .muted {
    color: var(--zx-text-muted);
  }
  .warn {
    color: var(--zx-warn);
  }
  .vhead {
    color: var(--zx-text-muted);
    margin-top: 0.2rem;
  }

  .output,
  .diff {
    margin: 0;
    padding: 0.45rem 0.65rem;
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    border: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    border-radius: var(--zx-radius);
    font-family: var(--zx-font-mono);
    font-size: 0.74rem;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 16rem;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem 1rem;
    font-size: 0.74rem;
    font-family: var(--zx-font-mono);
  }
  .layout {
    display: inline-flex;
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    overflow: hidden;
    font-family: var(--zx-font-ui);
  }
  .layout button {
    all: unset;
    cursor: pointer;
    padding: 0.1rem 0.55rem;
    font-size: 0.7rem;
    color: var(--zx-text-muted);
  }
  .layout button + button {
    border-left: 1px solid var(--zx-border);
  }
  .layout button.on {
    background: color-mix(in srgb, var(--zx-accent) 18%, transparent);
    color: var(--zx-accent);
  }
  .layout button:focus-visible {
    outline: 1px solid var(--zx-accent);
    outline-offset: -1px;
  }

  /* Side by side: line number | left output | line number | right output. */
  .sdiff {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto minmax(0, 1fr);
    background: var(--zx-term-bg);
    color: var(--zx-term-fg);
    border: 1px solid color-mix(in srgb, var(--zx-term-fg) 12%, transparent);
    border-radius: var(--zx-radius);
    font-family: var(--zx-font-mono);
    font-size: 0.74rem;
    max-height: 16rem;
    overflow-y: auto;
    flex-shrink: 0;
    padding: 0.3rem 0;
  }
  .sn {
    padding: 0 0.45rem;
    text-align: right;
    color: var(--zx-term-dim);
    user-select: none;
    font-variant-numeric: tabular-nums;
  }
  /* The divider between the two outputs. */
  .sn.r {
    border-left: 1px solid color-mix(in srgb, var(--zx-term-fg) 14%, transparent);
  }
  .st {
    padding-right: 0.6rem;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .st.same {
    color: var(--zx-term-dim);
  }
  .sn.a,
  .st.a {
    color: var(--zx-term-err);
    background: color-mix(in srgb, var(--zx-term-err) 10%, transparent);
  }
  .sn.b,
  .st.b {
    color: var(--zx-term-ok);
    background: color-mix(in srgb, var(--zx-term-ok) 10%, transparent);
  }
  .sn.none,
  .st.none {
    background: color-mix(in srgb, var(--zx-term-fg) 4%, transparent);
  }
  .sgap {
    grid-column: 1 / -1;
    padding: 0.1rem 0.65rem;
    color: var(--zx-term-dim);
    font-style: italic;
  }
  .lg.a {
    color: var(--zx-err);
  }
  .lg.b {
    color: var(--zx-ok);
  }

  .dl {
    display: block;
  }
  .dl.same {
    color: var(--zx-term-dim);
  }
  .dl.gap {
    color: var(--zx-term-dim);
    font-style: italic;
  }
  .dl.a {
    color: var(--zx-term-err);
    background: color-mix(in srgb, var(--zx-term-err) 10%, transparent);
  }
  .dl.b {
    color: var(--zx-term-ok);
    background: color-mix(in srgb, var(--zx-term-ok) 10%, transparent);
  }

  .variant {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.78rem;
  }
  .letter {
    font-size: 0.7rem;
    font-weight: 700;
    border-radius: 0.5rem;
    padding: 0.05rem 0.45rem;
    background: color-mix(in srgb, var(--zx-warn) 18%, transparent);
    color: var(--zx-warn);
    flex-shrink: 0;
  }
  .letter.main {
    background: color-mix(in srgb, var(--zx-text) 12%, transparent);
    color: var(--zx-text);
  }
  .vnames {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .link {
    align-self: flex-start;
    background: none;
    border: 0;
    padding: 0;
    color: var(--zx-accent);
    font-size: 0.74rem;
    font-family: inherit;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }

  .icon {
    display: inline-flex;
    background: none;
    border: 0;
    color: var(--zx-text-muted);
    cursor: pointer;
    padding: 0.2rem;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .icon:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: transparent;
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    color: var(--zx-text-muted);
    font-size: 0.74rem;
    padding: 0.2rem 0.5rem;
    cursor: pointer;
    font-family: inherit;
    flex-shrink: 0;
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
    padding: 0.12rem 0.45rem;
  }
  .btn.active {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
  }
</style>
