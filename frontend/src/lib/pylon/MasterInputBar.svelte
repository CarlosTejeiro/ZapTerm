<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import type { PylonTheme } from '$lib/themes/index'
  import Icon from '$lib/icons/Icon.svelte'

  interface Props {
    theme: PylonTheme
    /** Live session UUIDs this bar broadcasts to. */
    sessionIds: string[]
    /** When provided, shows a "Run & compare" action that captures + diffs
     *  per-host output instead of just fanning the command out. */
    onRunCompare?: (command: string) => void
  }

  const { theme, sessionIds, onRunCompare }: Props = $props()

  let line = $state('')
  // "live" = every keystroke is fanned out as you type (SecureCRT-style chat
  // mode). Off = nothing is sent until you press Enter (safer default — you
  // compose the whole command, review it, then dispatch).
  let live = $state(false)
  let flash = $state<string | null>(null)
  let flashTimer: ReturnType<typeof setTimeout> | null = null

  function notify(text: string) {
    flash = text
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = setTimeout(() => (flash = null), 1800)
  }

  function fanout(data: string) {
    const bytes = Array.from(new TextEncoder().encode(data))
    for (const sid of sessionIds) {
      invoke('send_input', { sessionId: sid, data: bytes }).catch(console.error)
    }
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
      fanout(live ? '\r' : line + '\r')
      notify(`Sent to ${sessionIds.length} host${sessionIds.length === 1 ? '' : 's'}`)
      line = ''
    }
  }

  function onInput(e: Event) {
    if (!live) return
    // Stream only the newly-typed tail so we don't re-send the whole buffer.
    const value = (e.target as HTMLInputElement).value
    if (value.length > line.length) {
      fanout(value.slice(line.length))
    }
    line = value
  }

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
    onRunCompare(cmd)
    line = ''
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
  <span class="mb-label" style:color={theme.accent}>
    <Icon name="cast" size={13} />
    MASTER
  </span>
  <input
    class="mb-input"
    placeholder={sessionIds.length
      ? `Broadcast to ${sessionIds.length} host${sessionIds.length === 1 ? '' : 's'}… (Enter to send)`
      : 'Waiting for sessions to connect…'}
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
  <label class="mb-live" style:color={theme.textMuted} title="Stream every keystroke as you type">
    <input type="checkbox" bind:checked={live} />
    live
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
