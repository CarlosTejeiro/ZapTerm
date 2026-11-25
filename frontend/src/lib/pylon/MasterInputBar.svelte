<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import type { PylonTheme } from '$lib/themes/index'
  import Icon from '$lib/icons/Icon.svelte'
  import { liveDelta } from './liveInput'
  import { compareSettings } from '$lib/orchestrator/compareSettings.svelte'
  import { MAX_PAGES } from '$lib/orchestrator/compare'
  import { PagerWatch } from '$lib/orchestrator/pagerWatch'

  interface Props {
    theme: PylonTheme
    /** Live session UUIDs this bar broadcasts to. */
    sessionIds: string[]
    /** When provided, shows a "Run & compare" action that captures + diffs
     *  per-host output instead of just fanning the command out. `typed`: the
     *  command is already on the devices' line ("live" mode) — only Enter is
     *  left to send. */
    onRunCompare?: (command: string, typed: boolean) => void
    /** What the sessions belong to, for the label's tooltip. */
    scopeLabel?: 'tab' | 'group'
  }

  const { theme, sessionIds, onRunCompare, scopeLabel = 'tab' }: Props = $props()

  let line = $state('')
  // "live" = every keystroke is fanned out as you type (SecureCRT-style chat
  // mode). Off = nothing is sent until you press Enter (safer default — you
  // compose the whole command, review it, then dispatch).
  let live = $state(false)
  const LIVE_HELP =
    'Off: nothing is sent until you press Enter, so you can check the command first. ' +
    'On: every key goes to all the sessions as you type it — for what a device answers ' +
    'before Enter, such as ? for context help on Cisco IOS. Backspace is sent too.'
  const PAGE_HELP =
    'Presses Space for you when a device stops at a pager prompt (--More--, ' +
    '---(more)---), so you get the whole output. After a line sent from this bar ' +
    `(Enter or Run & compare), only in the session that is waiting, up to ${MAX_PAGES} pages.`

  // Pages through the output of a line sent with Enter (Run & compare pages
  // through its own).
  const pager = new PagerWatch()
  onDestroy(() => pager.stop())
  $effect(() => {
    if (!compareSettings.autoPage) pager.stop()
  })
  let flash = $state<string | null>(null)
  let flashTimer: ReturnType<typeof setTimeout> | null = null

  function notify(text: string) {
    flash = text
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = setTimeout(() => (flash = null), 1800)
  }

  function fanout(data: string, ids: string[] = sessionIds) {
    const bytes = Array.from(new TextEncoder().encode(data))
    for (const sid of ids) {
      invoke('send_input', { sessionId: sid, data: bytes }).catch(console.error)
    }
  }

  /** Send a finished line; with "Page through --More--" on, start watching
   *  for pager prompts first so the first page isn't missed. */
  async function sendLine(data: string) {
    const ids = [...sessionIds]
    if (compareSettings.autoPage) await pager.start(ids)
    else pager.stop()
    fanout(data, ids)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      if (sessionIds.length === 0) {
        notify('No live sessions in this grid yet')
        return
      }
      // In live mode the characters were already streamed; only the newline
      // is left to send. Otherwise send the whole composed line + newline.
      void sendLine(live ? liveDelta(sentLive, line) + '\r' : line + '\r')
      notify(`Sent to ${sessionIds.length} host${sessionIds.length === 1 ? '' : 's'}`)
      line = ''
      sentLive = ''
    }
  }

  /** In live mode: what the devices' command line holds now. */
  let sentLive = ''

  function onInput(e: Event) {
    const value = (e.target as HTMLInputElement).value
    if (live) {
      // Keep the devices' line equal to the box: new characters are sent,
      // deleted ones are erased with Backspace.
      const delta = liveDelta(sentLive, value)
      if (delta) fanout(delta)
      sentLive = value
    }
    line = value
  }

  // Switching live on or off starts from an empty line on the devices'
  // side: what was composed before isn't on their line.
  $effect(() => {
    void live
    sentLive = ''
  })

  function runCompare() {
    if (!onRunCompare) return
    const cmd = line.trim()
    if (!cmd) {
      notify('Type a command first')
      return
    }
    if (sessionIds.length === 0) {
      notify('No live sessions in this grid yet')
      return
    }
    // Live mode already typed the line on every device: sending it again
    // would double it (`show versionshow version`).
    if (live) {
      const delta = liveDelta(sentLive, line)
      if (delta) fanout(delta)
    }
    pager.stop()
    onRunCompare(cmd, live)
    line = ''
    sentLive = ''
  }
</script>

<div
  class="master-bar"
  style:background={theme.titlebarBg}
  style:border-top="1px solid {theme.border}"
  style:font-family={theme.fontUi}
  style:--mb-fill="color-mix(in srgb, {theme.textPrimary} 7%, transparent)"
  style:--mb-fill-focus="color-mix(in srgb, {theme.textPrimary} 12%, transparent)"
  style:--mb-border={theme.border}
  style:--mb-accent={theme.accent}
>
  <span
    class="mb-label"
    style:color={theme.accent}
    title={`What you type in this box goes to all ${sessionIds.length} session${sessionIds.length === 1 ? '' : 's'} of this ${scopeLabel}`}
  >
    <Icon name="cast" size={13} />
    SEND TO ALL
  </span>
  <input
    class="mb-input"
    placeholder={!sessionIds.length
      ? 'Waiting for sessions to connect…'
      : live
        ? `Each key goes to ${sessionIds.length} host${sessionIds.length === 1 ? '' : 's'} as you type`
        : `Broadcast to ${sessionIds.length} host${sessionIds.length === 1 ? '' : 's'}… (Enter to send)`}
    bind:value={line}
    oninput={onInput}
    onkeydown={onKeydown}
    autocomplete="off"
    autocapitalize="off"
    autocorrect="off"
    spellcheck="false"
    style:color={theme.textPrimary}
  />
  {#if flash}
    <span class="mb-flash" style:color={theme.textMuted}>{flash}</span>
  {/if}
  {#if onRunCompare}
    <button
      class="mb-run"
      style:border="1px solid {theme.accent}"
      style:color={theme.accent}
      onclick={runCompare}
      title="Run on all and compare the output">▶ Run &amp; compare</button
    >
  {/if}
  {#if onRunCompare}
    <label class="mb-live" style:color={theme.textMuted} title={PAGE_HELP}>
      <input type="checkbox" bind:checked={compareSettings.autoPage} />
      Page through --More--
    </label>
  {/if}
  <label class="mb-live" style:color={theme.textMuted} title={LIVE_HELP}>
    <input type="checkbox" bind:checked={live} />
    Send keys as I type
  </label>
</div>

<style>
  .master-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    flex-shrink: 0;
  }

  .mb-label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  .mb-input {
    flex: 1;
    background: var(--mb-fill);
    border: 1px solid var(--mb-border);
    border-radius: 5px;
    padding: 6px 10px;
    font-size: 13px;
    font-family: inherit;
    outline: none;
    min-width: 0;
    transition:
      background 0.1s,
      border-color 0.1s;
  }

  .mb-input:focus {
    background: var(--mb-fill-focus);
    border-color: var(--mb-accent);
  }

  .mb-flash {
    font-size: 10.5px;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .mb-run {
    background: transparent;
    border-radius: 5px;
    padding: 5px 10px;
    font-size: 11px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .mb-run:hover {
    filter: brightness(1.25);
  }

  .mb-live {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    flex-shrink: 0;
    cursor: pointer;
    user-select: none;
  }
</style>
