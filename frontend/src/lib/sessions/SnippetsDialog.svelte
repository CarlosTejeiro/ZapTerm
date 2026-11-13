<script lang="ts">
  import { ask } from '@tauri-apps/plugin-dialog'
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import { resolveSnippet } from '$lib/snippets/variables.svelte'
  import {
    createSnippet,
    updateSnippet,
    deleteSnippet,
    sendInputText,
    listPlatforms,
  } from '$lib/bridge/commands'
  import type { Snippet, PlatformInfo } from '$lib/bridge/types'
  import {
    getFocusedSessionId,
    broadcast,
    broadcastTargets,
  } from '$lib/stores/sessionRuntime.svelte'
  import {
    snippets,
    loadSnippets,
    loadVisibleSnippets,
    loadRecents,
  } from '$lib/stores/snippets.svelte'

  interface Props {
    onClose: () => void
  }

  let { onClose }: Props = $props()

  let loading = $state(true)
  let status = $state<{ kind: 'ok' | 'err'; text: string } | null>(null)
  let platforms = $state<PlatformInfo[]>([])

  /** Button-bar accent palette (mirrors ButtonEditor). `null` = theme tint. */
  const SWATCHES = ['#5eb3b2', '#5b8fc9', '#9a91e8', '#3e8f60', '#b88528', '#c2410c', '#b13a3a']

  // Add form
  let showAdd = $state(false)
  let newName = $state('')
  let newContent = $state('')
  /** Empty string = global; otherwise a Platform.as_str() value. */
  let newPlatform = $state('')
  let newColor = $state<string | null>(null)

  // Edit form
  let editingId = $state<number | null>(null)
  let editName = $state('')
  let editContent = $state('')
  let editPlatform = $state('')
  let editColor = $state<string | null>(null)

  onMount(async () => {
    // Pull platforms once; the list is bundled, doesn't change at runtime.
    try {
      platforms = await listPlatforms()
    } catch {
      /* stay empty */
    }
    await refresh()
  })

  async function refresh() {
    loading = true
    try {
      await loadSnippets()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    } finally {
      loading = false
    }
  }

  /// Tell the bottom bar to re-fetch so user edits propagate live. The bar's
  /// view is platform-filtered, so a single full reload of `snippets` isn't
  /// enough.
  async function refreshBar() {
    await Promise.all([loadVisibleSnippets(), loadRecents()])
  }

  /// Look up the user-facing display name for a platform key, falling back
  /// to the key itself if it's not in the platforms list (e.g. a stale tag
  /// from a previous schema).
  function platformLabel(key: string | null | undefined): string {
    if (!key) return ''
    const p = platforms.find((p) => p.id === key)
    return p?.name ?? key
  }

  async function add() {
    if (!newName.trim()) {
      status = { kind: 'err', text: 'Name is required.' }
      return
    }
    try {
      await createSnippet(newName.trim(), newContent, newPlatform || null, newColor)
      newName = ''
      newContent = ''
      newPlatform = ''
      newColor = null
      showAdd = false
      status = { kind: 'ok', text: 'Snippet added.' }
      await refresh()
      await refreshBar()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  function beginEdit(s: Snippet) {
    editingId = s.id
    editName = s.name
    editContent = s.content
    editPlatform = s.platform ?? ''
    editColor = s.color ?? null
  }

  async function commitEdit() {
    if (editingId == null) return
    if (!editName.trim()) {
      status = { kind: 'err', text: 'Name is required.' }
      return
    }
    try {
      await updateSnippet(editingId, editName.trim(), editContent, editPlatform || null, editColor)
      editingId = null
      status = { kind: 'ok', text: 'Snippet updated.' }
      await refresh()
      await refreshBar()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  function cancelEdit() {
    editingId = null
  }

  async function remove(s: Snippet) {
    if (!(await ask(`Delete snippet "${s.name}"?`, { title: 'Snippets', kind: 'warning' }))) return
    try {
      await deleteSnippet(s.id)
      status = { kind: 'ok', text: 'Deleted.' }
      await refresh()
      await refreshBar()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  /// Send the snippet to the focused session (and to the other sessions of
  /// the active tab if MultiExec broadcast is on).
  async function send(s: Snippet) {
    const focused = getFocusedSessionId()
    if (!focused) {
      status = { kind: 'err', text: 'No session is focused. Click a terminal first.' }
      return
    }
    const content = await resolveSnippet(s.content)
    if (content === null) return
    try {
      await sendInputText(focused, content)
      if (broadcast.enabled) {
        const others = broadcastTargets(focused)
        for (const other of others) {
          await sendInputText(other, content).catch(() => {})
        }
        status = { kind: 'ok', text: `Sent to ${1 + others.length} sessions.` }
      } else {
        status = { kind: 'ok', text: 'Sent.' }
      }
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  function preview(content: string): string {
    const first = content.split(/\r?\n/)[0] ?? ''
    return first.length > 80 ? first.slice(0, 80) + '…' : first
  }

  function fmt(e: unknown): string {
    return errorText(e)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (editingId != null) cancelEdit()
      else if (showAdd) showAdd = false
      else onClose()
    }
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog">
    <div class="header">
      <h2>Snippets</h2>
      <div class="header-right">
        <button class="btn" onclick={() => (showAdd = !showAdd)}>+ New</button>
        <button class="btn" onclick={onClose} title="Close"><Icon name="x" size={12} /></button>
      </div>
    </div>

    {#if broadcast.enabled}
      <p class="hint broadcast">
        <Icon name="alert" size={12} /> MultiExec broadcast is ON — sending will dispatch to
        <strong>every session in the active tab</strong>.
      </p>
    {:else}
      <p class="hint">Click ▶ to send the snippet to the focused terminal.</p>
    {/if}

    {#if showAdd}
      <form
        class="form"
        onsubmit={(e) => {
          e.preventDefault()
          add()
        }}
      >
        <label>
          <span>Name</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={newName} placeholder="e.g. show interface description" autofocus />
        </label>
        <label>
          <span>Content</span>
          <textarea
            bind:value={newContent}
            rows="4"
            placeholder="show interface description"
            spellcheck="false"
          ></textarea>
        </label>
        <label>
          <span>Applies to</span>
          <select bind:value={newPlatform}>
            <option value="">Every session (global)</option>
            {#each platforms.filter((p) => p.id !== 'generic') as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>Color</span>
          <div class="swatch-row">
            <button
              type="button"
              class="swatch none"
              class:sel={newColor === null}
              onclick={() => (newColor = null)}
              title="Sin color">✕</button
            >
            {#each SWATCHES as c (c)}
              <button
                type="button"
                class="swatch"
                class:sel={newColor === c}
                style:background={c}
                onclick={() => (newColor = c)}
                aria-label={c}
              ></button>
            {/each}
          </div>
        </label>
        <div class="form-actions">
          <button type="button" class="btn" onclick={() => (showAdd = false)}>Cancel</button>
          <button type="submit" class="btn primary" disabled={!newName.trim()}>Save</button>
        </div>
      </form>
    {/if}

    {#if status}
      <p class="msg" class:err={status.kind === 'err'}>{status.text}</p>
    {/if}

    <div class="listing">
      {#if loading}
        <p class="muted center">Loading…</p>
      {:else if snippets.length === 0}
        <p class="muted center">No snippets yet.</p>
      {:else}
        <ul>
          {#each snippets as s (s.id)}
            <li>
              {#if editingId === s.id}
                <form
                  class="form inline"
                  onsubmit={(e) => {
                    e.preventDefault()
                    commitEdit()
                  }}
                >
                  <input bind:value={editName} />
                  <textarea bind:value={editContent} rows="3" spellcheck="false"></textarea>
                  <select bind:value={editPlatform}>
                    <option value="">Every session (global)</option>
                    {#each platforms.filter((p) => p.id !== 'generic') as p (p.id)}
                      <option value={p.id}>{p.name}</option>
                    {/each}
                  </select>
                  <div class="swatch-row">
                    <button
                      type="button"
                      class="swatch none"
                      class:sel={editColor === null}
                      onclick={() => (editColor = null)}
                      title="Sin color">✕</button
                    >
                    {#each SWATCHES as c (c)}
                      <button
                        type="button"
                        class="swatch"
                        class:sel={editColor === c}
                        style:background={c}
                        onclick={() => (editColor = c)}
                        aria-label={c}
                      ></button>
                    {/each}
                  </div>
                  <div class="form-actions">
                    <button type="button" class="btn" onclick={cancelEdit}>Cancel</button>
                    <button type="submit" class="btn primary" disabled={!editName.trim()}
                      >Save</button
                    >
                  </div>
                </form>
              {:else}
                <button class="send" onclick={() => send(s)} title="Send to focused session"
                  >▶</button
                >
                <div class="info">
                  <span class="name">
                    {s.name}
                    {#if s.platform}
                      <span class="platform-tag">{platformLabel(s.platform)}</span>
                    {/if}
                  </span>
                  <span class="preview">{preview(s.content)}</span>
                </div>
                <button class="btn icon" onclick={() => beginEdit(s)} title="Edit"
                  ><Icon name="pencil" size={12} /></button
                >
                <button class="btn icon danger" onclick={() => remove(s)} title="Delete"
                  ><Icon name="x" size={12} /></button
                >
              {/if}
            </li>
          {/each}
        </ul>
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
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 115;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    color: var(--zx-text);
    box-shadow: var(--zx-shadow);
    font-family: var(--zx-font-ui);
    padding: 1rem 1.2rem 1.2rem;
    width: 36rem;
    max-width: 95vw;
    height: 32rem;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .header-right {
    display: flex;
    gap: 0.35rem;
  }

  h2 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--zx-text);
  }

  .hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--zx-text-muted);
  }

  .hint.broadcast {
    color: var(--zx-warn);
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.7rem 0.9rem;
  }

  .form label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--zx-text-muted);
  }

  input,
  textarea,
  select {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.85rem;
    padding: 0.35rem 0.55rem;
    outline: none;
    font-family: inherit;
  }

  textarea {
    font-family: var(--zx-font-mono);
    resize: vertical;
  }

  input:focus-visible,
  textarea:focus-visible,
  select:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }

  .platform-tag {
    display: inline-block;
    margin-left: 0.4rem;
    padding: 0 0.4rem;
    font-size: 0.65rem;
    line-height: 1.5;
    color: var(--zx-accent);
    background: var(--zx-active-bg);
    border: 1px solid var(--zx-active-border);
    border-radius: 0.25rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    vertical-align: 0.05em;
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .btn {
    padding: 0.35rem 0.7rem;
    font-size: 0.8rem;
    border-radius: 0.25rem;
    border: 1px solid var(--zx-border);
    background: var(--zx-surface-2);
    color: var(--zx-text);
    cursor: pointer;
    font-family: inherit;
  }

  .btn:hover:not(:disabled) {
    background: var(--zx-hover-bg);
  }

  .btn.primary {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
  }

  .btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-accent) 85%, black);
  }

  .btn.primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn.icon {
    padding: 0.2rem 0.45rem;
  }

  .btn.danger {
    color: var(--zx-err);
  }

  .btn.danger:hover {
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
  }

  .swatch-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .swatch {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid var(--zx-border);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: transform 0.1s;
  }
  .swatch:hover {
    transform: scale(1.15);
  }
  .swatch.sel {
    box-shadow: 0 0 0 2px var(--zx-accent);
  }
  .swatch.none {
    background: transparent;
    color: var(--zx-text-dim);
    font-size: 10px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .msg {
    margin: 0;
    font-size: 0.8rem;
    color: var(--zx-ok);
  }

  .msg.err {
    color: var(--zx-err);
  }

  .listing {
    flex: 1;
    overflow: auto;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.7rem;
    border-bottom: 1px solid var(--zx-border);
  }

  li:last-child {
    border-bottom: none;
  }

  .send {
    background: var(--zx-ok);
    color: var(--zx-on-accent);
    border: none;
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 0.25rem;
    cursor: pointer;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.7rem;
  }

  .send:hover {
    background: color-mix(in srgb, var(--zx-ok) 85%, black);
  }

  .info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    overflow: hidden;
  }

  .name {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--zx-text);
  }

  .preview {
    font-size: 0.74rem;
    color: var(--zx-text-muted);
    font-family: var(--zx-font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .form.inline {
    flex: 1;
    padding: 0.5rem;
  }

  .muted {
    color: var(--zx-text-muted);
    font-size: 0.82rem;
  }

  .center {
    text-align: center;
    padding: 1.5rem 0;
  }
</style>
