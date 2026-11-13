<script lang="ts">
  import { ask } from '@tauri-apps/plugin-dialog'
  import { errorText } from '$lib/bridge/errors'
  import type { SavedSession, BroadcastGroup } from '$lib/bridge/types'
  import Icon from '$lib/icons/Icon.svelte'
  import {
    groups,
    loadGroups,
    addGroup,
    renameGroup,
    removeGroup,
    setGroupMembers,
  } from '$lib/stores/groups.svelte'

  interface Props {
    sessions: SavedSession[]
    onClose: () => void
    onOpenGrid: (group: BroadcastGroup) => void
  }

  let { sessions, onClose, onOpenGrid }: Props = $props()

  let status = $state<{ kind: 'ok' | 'err'; text: string } | null>(null)
  let newName = $state('')

  // Membership editor: which group is being edited + its working set.
  let editingId = $state<number | null>(null)
  let editName = $state('')
  let editMembers = $state<Set<number>>(new Set())

  function fmt(e: unknown): string {
    return errorText(e)
  }

  async function create() {
    const name = newName.trim()
    if (!name) return
    try {
      await addGroup(name)
      newName = ''
      status = { kind: 'ok', text: `Group "${name}" created.` }
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  function startEdit(g: BroadcastGroup) {
    editingId = g.id
    editName = g.name
    editMembers = new Set(g.session_ids)
  }

  function cancelEdit() {
    editingId = null
    editMembers = new Set()
  }

  function toggleMember(id: number) {
    const next = new Set(editMembers)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    editMembers = next
  }

  async function saveEdit() {
    if (editingId == null) return
    try {
      const name = editName.trim()
      if (name) await renameGroup(editingId, name)
      // Preserve saved-session order from the sessions list for stable grids.
      const ordered = sessions.map((s) => s.id).filter((id) => editMembers.has(id))
      await setGroupMembers(editingId, ordered)
      status = { kind: 'ok', text: 'Group saved.' }
      cancelEdit()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  async function destroy(g: BroadcastGroup) {
    if (!(await ask(`Delete group "${g.name}"?`, { title: 'Groups', kind: 'warning' }))) return
    try {
      await removeGroup(g.id)
      if (editingId === g.id) cancelEdit()
    } catch (e) {
      status = { kind: 'err', text: fmt(e) }
    }
  }

  function memberNames(g: BroadcastGroup): string {
    const names = g.session_ids
      .map((id) => sessions.find((s) => s.id === id)?.name)
      .filter((n): n is string => !!n)
    return names.length ? names.join(', ') : 'no sessions'
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (editingId != null) cancelEdit()
      else onClose()
    }
  }

  // Refresh on open so we reflect any external changes.
  loadGroups().catch(() => {})
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog">
    <div class="header">
      <h2>Broadcast groups</h2>
      <button class="btn" onclick={onClose} title="Close"><Icon name="x" size={12} /></button>
    </div>

    <p class="hint">
      A group opens N sessions in a grid with a master bar that drives them all at once.
    </p>

    <form
      class="newform"
      onsubmit={(e) => {
        e.preventDefault()
        create()
      }}
    >
      <input bind:value={newName} placeholder="New group name (e.g. core-switches)" />
      <button class="btn primary" type="submit">+ Create</button>
    </form>

    {#if status}
      <p class="status" class:err={status.kind === 'err'}>{status.text}</p>
    {/if}

    <div class="list">
      {#each groups as g (g.id)}
        <div class="group">
          {#if editingId === g.id}
            <div class="edit">
              <input class="edit-name" bind:value={editName} placeholder="Name" />
              <div class="members">
                {#if sessions.length === 0}
                  <p class="empty">No saved sessions yet.</p>
                {/if}
                {#each sessions as s (s.id)}
                  <label class="member">
                    <input
                      type="checkbox"
                      checked={editMembers.has(s.id)}
                      onchange={() => toggleMember(s.id)}
                    />
                    <span class="m-name">{s.name}</span>
                    <span class="m-host">{s.protocol.toUpperCase()} · {s.host ?? ''}</span>
                  </label>
                {/each}
              </div>
              <div class="edit-actions">
                <button class="btn" onclick={cancelEdit}>Cancel</button>
                <button class="btn primary" onclick={saveEdit}>Save</button>
              </div>
            </div>
          {:else}
            <div class="row">
              <div class="info">
                <span class="g-name">{g.name}</span>
                <span class="g-members">{memberNames(g)}</span>
              </div>
              <div class="row-actions">
                <button
                  class="btn primary"
                  disabled={g.session_ids.length === 0}
                  onclick={() => onOpenGrid(g)}
                  title="Open as grid">▦ Open</button
                >
                <button class="btn" onclick={() => startEdit(g)} title="Edit members"
                  ><Icon name="pencil" size={12} /></button
                >
                <button class="btn danger" onclick={() => destroy(g)} title="Delete"
                  ><Icon name="x" size={12} /></button
                >
              </div>
            </div>
          {/if}
        </div>
      {/each}
      {#if groups.length === 0}
        <p class="empty">No groups yet. Create one above.</p>
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
    width: 38rem;
    max-width: 95vw;
    height: 34rem;
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

  .newform {
    display: flex;
    gap: 0.4rem;
  }

  .newform input {
    flex: 1;
  }

  input {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.85rem;
    padding: 0.35rem 0.55rem;
    outline: none;
    font-family: inherit;
  }

  input:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }

  .status {
    margin: 0;
    font-size: 0.78rem;
    color: var(--zx-ok);
  }
  .status.err {
    color: var(--zx-err);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-height: 0;
  }

  .group {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.5rem 0.7rem;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .g-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--zx-text);
  }

  .g-members {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 22rem;
  }

  .row-actions,
  .edit-actions {
    display: flex;
    gap: 0.3rem;
    flex-shrink: 0;
  }

  .edit {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .members {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    max-height: 12rem;
    overflow-y: auto;
  }

  .member {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--zx-text);
    padding: 0.15rem 0;
  }

  .m-name {
    flex-shrink: 0;
  }

  .m-host {
    font-size: 0.7rem;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    font-size: 0.78rem;
    color: var(--zx-text-dim);
    margin: 0.3rem 0;
  }

  .btn {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.78rem;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
    font-family: inherit;
  }

  .btn:hover {
    background: var(--zx-hover-bg);
  }

  .btn.primary {
    background: var(--zx-accent);
    border-color: var(--zx-accent);
    color: var(--zx-on-accent);
  }
  .btn.primary:hover {
    background: color-mix(in srgb, var(--zx-accent) 85%, black);
  }
  .btn.primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn.danger:hover {
    background: color-mix(in srgb, var(--zx-err) 20%, transparent);
    color: var(--zx-err);
  }
</style>
