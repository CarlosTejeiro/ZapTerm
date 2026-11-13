<script lang="ts">
  /**
   * SecureCRT-style "command window": paste a list of commands and dispatch
   * it line-by-line to any set of open sessions, with a configurable pause
   * between lines and mid-run cancellation. Lines starting with `#` and
   * blank lines are skipped, so runbooks can carry comments.
   */
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon from '$lib/icons/Icon.svelte'
  import { sendInputText } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'

  export interface CommandTarget {
    sessionId: string
    label: string
    tabLabel: string
    /** Belongs to the active tab — preselected by default. */
    active: boolean
  }

  interface Props {
    targets: CommandTarget[]
    onClose: () => void
  }

  const { targets, onClose }: Props = $props()

  let text = $state('')
  let delayMs = $state(500)
  // Intentional one-shot capture: the dialog is remounted on every open, so
  // the preselection (active tab's sessions, else all) is a snapshot of the
  // moment it opens — not a live binding.
  // svelte-ignore state_referenced_locally
  let selected = $state<Set<string>>(
    new Set(
      (targets.some((t) => t.active) ? targets.filter((t) => t.active) : targets).map(
        (t) => t.sessionId,
      ),
    ),
  )
  let sending = $state(false)
  let progress = $state({ line: 0, total: 0 })
  let cancelled = false

  const lines = $derived(
    text
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith('#')),
  )

  function toggleTarget(id: string) {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selected = next
  }

  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

  async function send() {
    if (sending || lines.length === 0 || selected.size === 0) return
    sending = true
    cancelled = false
    progress = { line: 0, total: lines.length }
    const ids = [...selected]
    let sent = 0
    for (const line of lines) {
      if (cancelled) break
      progress = { line: sent + 1, total: lines.length }
      await Promise.all(ids.map((sid) => sendInputText(sid, line + '\r').catch(() => {})))
      sent++
      if (sent < lines.length && delayMs > 0) await sleep(delayMs)
    }
    sending = false
    showToast({
      kind: cancelled ? 'warning' : 'success',
      title: cancelled ? 'Send cancelled' : 'List sent',
      detail: `${sent}/${lines.length} commands to ${ids.length} session${ids.length === 1 ? '' : 's'}`,
    })
    if (!cancelled) onClose()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !sending) onClose()
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Send command list"
  tabindex="-1"
  onkeydown={onKeydown}
  transition:fade={{ duration: 140 }}
>
  <div class="dialog" transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}>
    <div class="header">
      <h2>Send command list</h2>
      <button class="close-btn" onclick={onClose} disabled={sending} aria-label="Close">
        <Icon name="x" size={13} />
      </button>
    </div>

    <div class="body">
      <div class="targets">
        <span class="section-label">Target sessions ({selected.size}/{targets.length})</span>
        {#if targets.length === 0}
          <p class="hint">No open sessions.</p>
        {:else}
          <ul class="target-list">
            {#each targets as t (t.sessionId)}
              <li>
                <label class="target-row">
                  <input
                    type="checkbox"
                    checked={selected.has(t.sessionId)}
                    disabled={sending}
                    onchange={() => toggleTarget(t.sessionId)}
                  />
                  <span class="target-name">{t.label}</span>
                  <span class="target-tab">{t.tabLabel}</span>
                </label>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="editor">
        <span class="section-label">Commands — one per line («#» lines are ignored)</span>
        <textarea
          bind:value={text}
          placeholder={'show clock\nterminal length 0\nshow ip interface brief\n# comment'}
          spellcheck="false"
          disabled={sending}
        ></textarea>
        <div class="options">
          <label class="delay">
            Pause between lines
            <input type="number" min="0" step="100" bind:value={delayMs} disabled={sending} />
            ms
          </label>
          <span class="count">{lines.length} command{lines.length === 1 ? '' : 's'}</span>
        </div>
      </div>
    </div>

    <div class="footer">
      {#if sending}
        <span class="progress">Sending line {progress.line}/{progress.total}…</span>
        <button class="btn" onclick={() => (cancelled = true)}>Cancel</button>
      {:else}
        <button class="btn" onclick={onClose}>Close</button>
        <button
          class="btn primary"
          disabled={lines.length === 0 || selected.size === 0}
          onclick={send}>Send to {selected.size} session{selected.size === 1 ? '' : 's'}</button
        >
      {/if}
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
    z-index: 300;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .dialog {
    width: 640px;
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 96px);
    display: flex;
    flex-direction: column;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    box-shadow: var(--zx-shadow);
    font-family: var(--zx-font-ui);
    color: var(--zx-text);
    overflow: hidden;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--zx-border);
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--zx-text-dim);
    padding: 4px;
    border-radius: 4px;
    display: flex;
  }
  .close-btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .close-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .body {
    display: flex;
    gap: 14px;
    padding: 12px 16px;
    min-height: 0;
    overflow: hidden;
  }

  .section-label {
    display: block;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--zx-text-dim);
    margin-bottom: 6px;
  }

  .targets {
    width: 220px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .target-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    background: var(--zx-body-bg);
  }

  .target-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    font-size: 12.5px;
    cursor: pointer;
  }
  .target-row:hover {
    background: var(--zx-hover-bg);
  }
  .target-row input {
    accent-color: var(--zx-accent);
  }

  .target-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .target-tab {
    font-size: 10px;
    color: var(--zx-text-dim);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    padding: 0 4px;
    flex-shrink: 0;
    max-width: 70px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  textarea {
    flex: 1;
    min-height: 220px;
    resize: vertical;
    background: var(--zx-body-bg);
    color: var(--zx-text);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    padding: 8px 10px;
    font-family: var(--zx-font-mono);
    font-size: 12.5px;
    line-height: 1.55;
    outline: none;
  }
  textarea:focus {
    border-color: var(--zx-accent);
  }

  .options {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 12px;
    color: var(--zx-text-muted);
  }

  .delay {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .delay input {
    width: 70px;
    background: var(--zx-body-bg);
    color: var(--zx-text);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    padding: 3px 6px;
    font-family: var(--zx-font-mono);
    font-size: 12px;
  }

  .count {
    color: var(--zx-text-dim);
  }

  .footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 16px 14px;
    border-top: 1px solid var(--zx-border);
  }

  .progress {
    margin-right: auto;
    font-size: 12px;
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .btn {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    font-size: 12.5px;
    font-family: inherit;
    padding: 6px 14px;
    cursor: pointer;
  }
  .btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }

  .btn.primary {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
  .btn.primary:hover {
    filter: brightness(1.1);
  }
  .btn.primary:disabled {
    opacity: 0.45;
    cursor: default;
    filter: none;
  }
</style>
