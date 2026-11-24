<script lang="ts">
  /// Automation activity: what scripts, notebooks and AI assistants sent — or
  /// tried to send — through ZapTerm, read from the text file the backend
  /// keeps. Counters and a 14-day strip on top (both filter the list), then
  /// the list itself: newest day first, one card per client visit.
  import { onMount } from 'svelte'
  import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager'
  import Icon from '$lib/icons/Icon.svelte'
  import type { IconName } from '$lib/icons/icons'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import { scriptRun } from '$lib/scripting/scriptHost.svelte'
  import { activity, loadActivity, openActivity } from './activity.svelte'
  import {
    actionTag,
    clockOf,
    dayCounts,
    dayLabel,
    durationText,
    groupActivity,
    isRow,
    isoDay,
    limitDays,
    matches,
    NO_FILTER,
    outcomeOf,
    RESULT_LABEL,
    rowsAsText,
    SOURCE_LABEL,
    statsOf,
    visibleDays,
    type ActivityEntry,
    type ActivityFilter,
    type ActivityGroup,
    type ActivitySource,
    type Outcome,
  } from './activityView'

  interface Props {
    onClose: () => void
  }
  const { onClose }: Props = $props()

  let filter = $state<ActivityFilter>({ ...NO_FILTER })
  /** Rows shown before "Show older". */
  let maxRows = $state(300)
  /** Day under the pointer / focus in the strip. */
  let hoverDay = $state('')
  const today = new Date()

  onMount(() => {
    void loadActivity()
  })

  const rows = $derived(activity.entries.filter(isRow))
  const groups = $derived(groupActivity(activity.entries))
  /** What each counter would show: every filter but the result one. */
  const stats = $derived(statsOf(rows.filter((r) => matches(r, filter, 'outcome'))))
  /** Columns of the strip: every filter but the day one. */
  const strip = $derived(
    dayCounts(
      rows.filter((r) => matches(r, filter, 'day')),
      today,
    ),
  )
  const stripMax = $derived(Math.max(1, ...strip.map((d) => d.count)))
  const allDays = $derived(visibleDays(groups, filter))
  const limited = $derived(limitDays(allDays, maxRows))
  const shownRows = $derived(
    allDays.reduce((n, d) => n + d.groups.reduce((m, g) => m + g.rows.length, 0), 0),
  )
  const devices = $derived([...new Set(rows.map((r) => r.device).filter(Boolean))].sort())
  const filtering = $derived(
    filter.source !== 'all' ||
      filter.outcome !== 'all' ||
      filter.device !== '' ||
      filter.day !== '' ||
      filter.search.trim() !== '',
  )
  /** The visit still going on, if any: the newest one, while a client is
   *  connected or a script runs. */
  const liveKey = $derived.by(() => {
    const last = groups[groups.length - 1]
    const busy = scriptRun.status === 'connected' || scriptRun.status === 'running'
    return last && last.open && busy ? last.key : ''
  })

  const SOURCES: { id: ActivitySource | 'all'; label: string; icon: IconName | null }[] = [
    { id: 'all', label: 'All', icon: null },
    { id: 'mcp', label: 'AI assistant', icon: 'spark' },
    { id: 'notebook', label: 'Notebook', icon: 'book' },
    { id: 'script', label: 'Script', icon: 'file' },
  ]
  const SOURCE_ICON: Record<ActivitySource, IconName> = {
    mcp: 'spark',
    notebook: 'book',
    script: 'file',
    unknown: 'info',
  }
  const TILES: { id: Outcome | 'all'; label: string; icon: IconName | null; hint: string }[] = [
    { id: 'all', label: 'All actions', icon: null, hint: 'Everything that was sent or attempted' },
    {
      id: 'sent',
      label: 'Sent',
      icon: 'check',
      hint: 'Carried out: commands sent, sessions opened',
    },
    {
      id: 'blocked',
      label: 'Blocked',
      icon: 'shield',
      hint: 'Not sent: read-only mode refused it',
    },
    { id: 'declined', label: 'Declined', icon: 'ban', hint: 'Not sent: you said No when asked' },
    {
      id: 'failed',
      label: 'Failed',
      icon: 'alert',
      hint: 'Sent, but no prompt came back in time, or it ended in an error',
    },
  ]
  const OUTCOME_ICON: Record<Outcome, IconName> = {
    sent: 'check',
    blocked: 'shield',
    declined: 'ban',
    failed: 'alert',
  }

  function groupTitle(g: ActivityGroup): string {
    if (g.source === 'notebook') return g.client ? `Notebook · ${g.client}` : 'Notebook'
    return g.client || SOURCE_LABEL[g.source]
  }
  function groupKind(g: ActivityGroup): string {
    if (g.source === 'unknown') return 'older zapterm.py — restart the app or kernel that runs it'
    return g.source === 'mcp' ? 'AI assistant · MCP' : g.source === 'script' ? 'Script' : ''
  }
  function groupSpan(g: ActivityGroup): string {
    const a = clockOf(g.start).slice(0, 5)
    const b = clockOf(g.end).slice(0, 5)
    return a === b ? a : `${a} – ${b}`
  }
  function groupCounts(g: ActivityGroup): { outcome: Outcome; n: number }[] {
    const s = statsOf(g.rows)
    return (['sent', 'blocked', 'declined', 'failed'] as Outcome[])
      .map((outcome) => ({ outcome, n: s[outcome] }))
      .filter((c) => c.n > 0)
  }
  function stripLabel(day: string): string {
    const d = strip.find((x) => x.day === day)
    if (!d) return ''
    return `${dayLabel(day, today)} · ${d.count} action${d.count === 1 ? '' : 's'}`
  }
  const shortDay = (day: string) =>
    new Date(`${day}T00:00:00`).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })

  async function copyText(text: string, title: string) {
    try {
      await clipboardWriteText(text)
    } catch {
      await navigator.clipboard.writeText(text).catch(() => {})
    }
    showToast({ kind: 'success', title })
  }
  function copyRow(r: ActivityEntry) {
    void copyText(r.detail, 'Copied')
  }
  function copyShown() {
    void copyText(rowsAsText(allDays), `${shownRows} line${shownRows === 1 ? '' : 's'} copied`)
  }
  async function open(file: boolean) {
    try {
      await openActivity(file)
    } catch (e) {
      showToast({ kind: 'error', title: 'Activity', detail: String(e) })
    }
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
    }
  }
</script>

<div class="overlay" role="presentation" onkeydown={onKeydown}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Automation activity"
    tabindex="-1"
  >
    <header class="top">
      <span class="glyph"><Icon name="pulse" size={16} /></span>
      <div class="titles">
        <h3>Automation activity</h3>
        <p class="muted">
          Every command a script, a notebook or an AI assistant sent — or tried to send — through
          ZapTerm. Kept as a text file on this computer.
        </p>
      </div>
      <span class="spacer"></span>
      <button
        class="ghost"
        onclick={copyShown}
        disabled={shownRows === 0}
        title="Copy the lines shown"><Icon name="copy" size={13} /> Copy</button
      >
      <button class="ghost" onclick={() => open(true)} title="Open this month's file in your editor"
        ><Icon name="file" size={13} /> Open file</button
      >
      <button class="ghost" onclick={() => open(false)} title="Show the folder with the files"
        ><Icon name="folder" size={13} /> Folder</button
      >
      <!-- svelte-ignore a11y_autofocus -->
      <button class="icon" onclick={onClose} title="Close" aria-label="Close" autofocus
        ><Icon name="x" size={14} /></button
      >
    </header>

    <section class="summary" aria-label="Summary">
      <div class="tiles">
        {#each TILES as t (t.id)}
          <button
            class="tile {t.id}"
            class:on={filter.outcome === t.id}
            aria-pressed={filter.outcome === t.id}
            title={t.hint}
            onclick={() => (filter.outcome = t.id)}
          >
            <span class="tile-label">
              {#if t.icon}<span class="mark {t.id}"><Icon name={t.icon} size={12} /></span>{/if}
              {t.label}
            </span>
            <span class="tile-value">{stats[t.id]}</span>
          </button>
        {/each}
      </div>

      <div class="strip" aria-label="Actions per day, last 14 days">
        <div class="strip-head">
          <span class="strip-title">Last 14 days</span>
          <span class="strip-read">{stripLabel(hoverDay || filter.day)}</span>
        </div>
        <div class="cols">
          {#each strip as d (d.day)}
            <button
              class="col"
              class:on={filter.day === d.day}
              class:empty={d.count === 0}
              aria-pressed={filter.day === d.day}
              aria-label={stripLabel(d.day)}
              title={stripLabel(d.day)}
              onpointerenter={() => (hoverDay = d.day)}
              onpointerleave={() => (hoverDay = '')}
              onfocus={() => (hoverDay = d.day)}
              onblur={() => (hoverDay = '')}
              onclick={() => (filter.day = filter.day === d.day ? '' : d.day)}
            >
              <span
                class="bar"
                style:height={d.count ? `${Math.max(8, (d.count / stripMax) * 100)}%` : '2px'}
              ></span>
            </button>
          {/each}
        </div>
        <div class="strip-axis">
          <span>{shortDay(strip[0]?.day ?? isoDay(today))}</span>
          <span>Today</span>
        </div>
      </div>
    </section>

    <div class="filters" role="group" aria-label="Filters">
      <div class="seg" role="group" aria-label="Source">
        {#each SOURCES as s (s.id)}
          <button
            class:on={filter.source === s.id}
            aria-pressed={filter.source === s.id}
            onclick={() => (filter.source = s.id)}
            >{#if s.icon}<Icon name={s.icon} size={12} />{/if}{s.label}</button
          >
        {/each}
      </div>
      <select bind:value={filter.device} aria-label="Device" class:set={filter.device !== ''}>
        <option value="">All devices</option>
        {#each devices as d (d)}<option value={d}>{d}</option>{/each}
      </select>
      <label class="search">
        <Icon name="search" size={13} />
        <input
          type="search"
          placeholder="Search commands, devices…"
          bind:value={filter.search}
          spellcheck="false"
          aria-label="Search"
        />
      </label>
      {#if filter.day}
        <button class="chip" onclick={() => (filter.day = '')} title="Show every day"
          >{dayLabel(filter.day, today)} <Icon name="x" size={10} /></button
        >
      {/if}
      <span class="spacer"></span>
      {#if filtering}
        <button class="link" onclick={() => (filter = { ...NO_FILTER })}>Clear filters</button>
      {/if}
      <span class="count">{shownRows} of {rows.length}</span>
    </div>

    <div class="list">
      {#if activity.error}
        <p class="state err">Could not read the activity file: {activity.error}</p>
      {:else if !activity.loaded}
        <p class="state">Reading…</p>
      {:else if rows.length === 0 && groups.length === 0}
        <div class="state empty">
          <span class="glyph big"><Icon name="pulse" size={22} /></span>
          <p class="state-title">Nothing recorded yet</p>
          <p class="muted">
            When a script, a notebook or an AI assistant sends a command through ZapTerm, it shows
            up here — with what was sent, where, and whether ZapTerm let it through.
          </p>
        </div>
      {:else if limited.days.length === 0}
        <div class="state empty">
          <p class="state-title">Nothing matches these filters</p>
          <button class="link" onclick={() => (filter = { ...NO_FILTER })}>Clear filters</button>
        </div>
      {:else}
        {#each limited.days as d (d.day)}
          <h4 class="day">
            {dayLabel(d.day, today)}
            {#if dayLabel(d.day, today) === 'Today' || dayLabel(d.day, today) === 'Yesterday'}
              <span class="day-date">{shortDay(d.day)}</span>
            {/if}
          </h4>
          {#each d.groups as g (g.key)}
            <article class="visit {g.source}">
              <header class="visit-head">
                <span class="who {g.source}"><Icon name={SOURCE_ICON[g.source]} size={13} /></span>
                <span class="visit-title" title={groupTitle(g)}>{groupTitle(g)}</span>
                {#if groupKind(g)}<span class="kind">{groupKind(g)}</span>{/if}
                <span class="span">{groupSpan(g)}</span>
                {#if g.note}<span class="note" title={g.note}>{g.note}</span>{/if}
                {#if g.key === liveKey}<span class="live"><span class="pulse"></span>now</span>{/if}
                <span class="spacer"></span>
                {#each groupCounts(g) as c (c.outcome)}
                  <span class="tally" title={TILES.find((t) => t.id === c.outcome)?.hint}>
                    <span class="mark {c.outcome}"
                      ><Icon name={OUTCOME_ICON[c.outcome]} size={11} /></span
                    >{c.n}
                  </span>
                {/each}
              </header>
              {#if g.rows.length === 0}
                <p class="none">Nothing was sent.</p>
              {:else}
                <ul>
                  {#each g.rows as r, i (i)}
                    <li class={outcomeOf(r)}>
                      <button
                        class="row"
                        onclick={() => copyRow(r)}
                        title="Click to copy the command"
                      >
                        <span class="time">{clockOf(r.time)}</span>
                        <span class="device" title={r.address || r.device}
                          >{r.device || '—'}{#if r.address && r.address !== r.device}<span
                              class="addr">{r.address}</span
                            >{/if}</span
                        >
                        <span class="cmd" title={r.detail}>
                          {#if actionTag(r)}<span class="tag">{actionTag(r)}</span>{/if}{r.detail}
                        </span>
                        <span class="result">
                          <span class="mark {outcomeOf(r)}"
                            ><Icon name={OUTCOME_ICON[outcomeOf(r)]} size={11} /></span
                          >{RESULT_LABEL[r.result] ?? r.result}
                        </span>
                        <span class="took">{durationText(r.seconds)}</span>
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </article>
          {/each}
        {/each}
        {#if limited.hidden > 0}
          <button class="more" onclick={() => (maxRows += 500)}
            >Show older ({limited.hidden} more)</button
          >
        {/if}
      {/if}
    </div>

    <footer>
      <span class="path" title={activity.currentFile}>{activity.currentFile}</span>
      <span class="spacer"></span>
      {#if activity.olderFiles > 0}
        <button
          class="link"
          disabled={activity.loading}
          onclick={() => loadActivity(activity.months + 6)}
          >Load older months ({activity.olderFiles} file{activity.olderFiles === 1
            ? ''
            : 's'})</button
        >
      {/if}
    </footer>
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
    width: 66rem;
    max-width: 94vw;
    height: 86vh;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
    overflow: hidden;
    /* Status colours, each with its icon and label wherever it is used. */
    --ok: var(--zx-ok);
    --warn: var(--zx-warn);
    --err: var(--zx-err);
    --line: color-mix(in srgb, var(--zx-text) 9%, transparent);
    --fill: color-mix(in srgb, var(--zx-text) 4%, transparent);
  }
  .spacer {
    flex: 1;
  }
  h3,
  h4,
  p,
  ul {
    margin: 0;
  }
  ul {
    list-style: none;
    padding: 0;
  }
  button {
    font-family: inherit;
    color: inherit;
  }
  .muted {
    color: var(--zx-text-muted);
    font-size: 0.78rem;
    line-height: 1.45;
  }

  /* ── header ─────────────────────────────────────────────────────────── */
  .top {
    display: flex;
    align-items: center;
    gap: var(--zx-space-2);
    padding: var(--zx-space-4) var(--zx-space-4) var(--zx-space-3);
  }
  .glyph {
    width: 2rem;
    height: 2rem;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 9px;
    color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 14%, transparent);
  }
  .glyph.big {
    width: 3rem;
    height: 3rem;
    border-radius: 14px;
  }
  .titles {
    min-width: 0;
    margin-left: var(--zx-space-1);
  }
  h3 {
    font-size: 0.98rem;
    font-weight: 600;
    letter-spacing: -0.005em;
  }
  .ghost,
  .icon {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.76rem;
    color: var(--zx-text-muted);
    padding: 0.32rem 0.6rem;
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    white-space: nowrap;
  }
  .icon {
    padding: 0.35rem;
    border-color: transparent;
  }
  .ghost:hover:not(:disabled),
  .icon:hover {
    color: var(--zx-text);
    background: var(--fill);
  }
  .ghost:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .ghost:focus-visible,
  .icon:focus-visible,
  .tile:focus-visible,
  .col:focus-visible,
  .seg button:focus-visible,
  .row:focus-visible,
  .chip:focus-visible,
  .link:focus-visible,
  .more:focus-visible {
    outline: none;
    box-shadow: var(--zx-ring);
  }

  /* ── counters and the 14-day strip ──────────────────────────────────── */
  .summary {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 17rem;
    gap: var(--zx-space-3);
    padding: 0 var(--zx-space-4) var(--zx-space-3);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--zx-space-2);
  }
  .tile {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.65rem 0.75rem;
    min-height: 4.6rem;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--fill);
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .tile:hover {
    border-color: color-mix(in srgb, var(--zx-text) 22%, transparent);
  }
  .tile.on {
    border-color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 9%, transparent);
  }
  .tile-label {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.72rem;
    color: var(--zx-text-muted);
  }
  .tile-value {
    font-size: 1.55rem;
    font-weight: 600;
    line-height: 1;
    letter-spacing: -0.02em;
  }
  /* The status mark: a small tinted disc carrying the icon; the text beside
     it stays in text colours. */
  .mark {
    display: inline-grid;
    place-items: center;
    width: 1.05rem;
    height: 1.05rem;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .mark.sent {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 16%, transparent);
  }
  .mark.blocked {
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 18%, transparent);
  }
  .mark.declined,
  .mark.failed {
    color: var(--err);
    background: color-mix(in srgb, var(--err) 16%, transparent);
  }

  .strip {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.6rem 0.75rem 0.5rem;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--fill);
    min-width: 0;
  }
  .strip-head {
    display: flex;
    align-items: baseline;
    gap: var(--zx-space-2);
    font-size: 0.72rem;
  }
  .strip-title {
    color: var(--zx-text-muted);
  }
  .strip-read {
    margin-left: auto;
    color: var(--zx-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cols {
    flex: 1;
    display: flex;
    align-items: stretch;
    gap: 2px;
    min-height: 2.2rem;
    border-bottom: 1px solid var(--line);
  }
  /* The whole column is the target; the bar inside is the mark. */
  .col {
    all: unset;
    cursor: pointer;
    flex: 1;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    border-radius: 4px 4px 0 0;
  }
  .col:hover {
    background: color-mix(in srgb, var(--zx-text) 7%, transparent);
  }
  .bar {
    width: 100%;
    max-width: 14px;
    border-radius: 4px 4px 0 0;
    background: color-mix(in srgb, var(--zx-accent) 55%, transparent);
  }
  .col.empty .bar {
    background: color-mix(in srgb, var(--zx-text) 16%, transparent);
    border-radius: 0;
  }
  .col:hover .bar,
  .col.on .bar {
    background: var(--zx-accent);
  }
  .strip-axis {
    display: flex;
    justify-content: space-between;
    font-size: 0.66rem;
    color: var(--zx-text-muted);
  }

  /* ── filters ────────────────────────────────────────────────────────── */
  .filters {
    display: flex;
    align-items: center;
    gap: var(--zx-space-2);
    padding: var(--zx-space-2) var(--zx-space-4);
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    font-size: 0.76rem;
  }
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--zx-text) 7%, transparent);
  }
  .seg button {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.25rem 0.6rem;
    border-radius: 7px;
    color: var(--zx-text-muted);
    white-space: nowrap;
  }
  .seg button:hover {
    color: var(--zx-text);
  }
  .seg button.on {
    color: var(--zx-text);
    background: var(--zx-surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  }
  select,
  .search {
    height: 1.9rem;
    box-sizing: border-box;
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    background: transparent;
    color: var(--zx-text-muted);
    font-family: inherit;
    font-size: 0.76rem;
  }
  select {
    padding: 0 0.45rem;
    max-width: 12rem;
  }
  select.set {
    color: var(--zx-text);
    border-color: var(--zx-accent);
  }
  .search {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0 0.5rem;
    width: 15rem;
  }
  .search:focus-within {
    border-color: var(--zx-accent);
    color: var(--zx-text);
  }
  .search input {
    all: unset;
    flex: 1;
    min-width: 0;
    color: var(--zx-text);
  }
  .search input::placeholder {
    color: var(--zx-text-muted);
  }
  .chip {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0.55rem;
    border-radius: 999px;
    color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 13%, transparent);
  }
  .link {
    all: unset;
    cursor: pointer;
    color: var(--zx-accent);
    font-size: 0.76rem;
  }
  .link:hover {
    text-decoration: underline;
  }
  .link:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .count {
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }

  /* ── the list ───────────────────────────────────────────────────────── */
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--zx-space-2) var(--zx-space-4) var(--zx-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-2);
  }
  .day {
    position: sticky;
    top: calc(-1 * var(--zx-space-2));
    z-index: 1;
    display: flex;
    align-items: baseline;
    gap: var(--zx-space-2);
    padding: var(--zx-space-3) 0 var(--zx-space-1);
    background: var(--zx-surface);
    font-size: 0.74rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--zx-text-muted);
  }
  .day-date {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
  }
  .visit {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--fill);
    overflow: hidden;
    flex-shrink: 0;
  }
  .visit-head {
    display: flex;
    align-items: center;
    gap: var(--zx-space-2);
    padding: 0.5rem 0.75rem;
    font-size: 0.78rem;
    min-width: 0;
  }
  .who {
    width: 1.45rem;
    height: 1.45rem;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: var(--zx-text-muted);
    background: color-mix(in srgb, var(--zx-text) 9%, transparent);
  }
  .who.mcp {
    color: var(--zx-accent);
    background: color-mix(in srgb, var(--zx-accent) 16%, transparent);
  }
  .who.notebook {
    color: var(--zx-accent-2);
    background: color-mix(in srgb, var(--zx-accent-2) 16%, transparent);
  }
  .visit-title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind,
  .span,
  .note {
    color: var(--zx-text-muted);
    white-space: nowrap;
  }
  .kind {
    font-size: 0.7rem;
    padding: 0.08rem 0.45rem;
    border-radius: 999px;
    border: 1px solid var(--line);
  }
  .span {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
  }
  .note {
    font-size: 0.72rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.7rem;
    color: var(--zx-text);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 0 color-mix(in srgb, var(--ok) 60%, transparent);
    animation: pulse 1.8s ease-out infinite;
  }
  @keyframes pulse {
    to {
      box-shadow: 0 0 0 7px transparent;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse {
      animation: none;
    }
  }
  .tally {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .none {
    padding: 0 0.75rem 0.6rem 2.8rem;
    font-size: 0.74rem;
    color: var(--zx-text-muted);
  }
  ul {
    border-top: 1px solid var(--line);
  }
  li + li {
    border-top: 1px solid color-mix(in srgb, var(--zx-text) 5%, transparent);
  }
  .row {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    width: 100%;
    display: grid;
    grid-template-columns: 4.4rem minmax(7rem, 17rem) minmax(0, 1fr) 10.5rem 4.2rem;
    align-items: center;
    gap: var(--zx-space-3);
    padding: 0.36rem 0.75rem;
    font-size: 0.76rem;
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: color-mix(in srgb, var(--zx-text) 5%, transparent);
  }
  li.blocked .row {
    border-left-color: color-mix(in srgb, var(--warn) 70%, transparent);
  }
  li.declined .row,
  li.failed .row {
    border-left-color: color-mix(in srgb, var(--err) 70%, transparent);
  }
  .time,
  .took {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .took {
    text-align: right;
  }
  .device {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .addr {
    margin-left: 0.5rem;
    font-family: var(--zx-font-mono);
    font-size: 0.68rem;
    font-weight: 400;
    color: var(--zx-text-muted);
  }
  .cmd {
    font-family: var(--zx-font-mono);
    font-size: 0.76rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  li.blocked .cmd,
  li.declined .cmd {
    color: var(--zx-text-muted);
  }
  .tag {
    font-family: var(--zx-font-ui);
    font-size: 0.66rem;
    color: var(--zx-text-muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 0.02rem 0.35rem;
    margin-right: 0.45rem;
  }
  .result {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    white-space: nowrap;
  }
  .more {
    all: unset;
    cursor: pointer;
    align-self: center;
    margin-top: var(--zx-space-2);
    padding: 0.35rem 0.9rem;
    font-size: 0.76rem;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
    border-radius: 999px;
  }
  .more:hover {
    color: var(--zx-text);
  }

  .state {
    margin: auto;
    text-align: center;
    color: var(--zx-text-muted);
    font-size: 0.8rem;
  }
  .state.err {
    color: var(--err);
  }
  .state.empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--zx-space-2);
    max-width: 26rem;
  }
  .state-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--zx-text);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--zx-space-2);
    padding: var(--zx-space-2) var(--zx-space-4);
    border-top: 1px solid var(--line);
    min-height: 1.4rem;
  }
  .path {
    font-family: var(--zx-font-mono);
    font-size: 0.68rem;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
