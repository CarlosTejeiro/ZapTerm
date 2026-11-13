<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  /// Manage reusable credential ("vault") entries: list, add, edit, delete.
  /// Secrets never come back over the bridge — editing leaves the password
  /// field blank and only rotates the secret when the user types a new one.
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import type { PylonTheme } from '$lib/themes/index'
  import type { VaultEntry } from '$lib/bridge/types'
  import {
    listVaultEntries,
    createVaultEntry,
    updateVaultEntry,
    deleteVaultEntry,
  } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'

  interface Props {
    theme: PylonTheme
    onClose: () => void
  }

  const { theme, onClose }: Props = $props()

  let entries = $state<VaultEntry[]>([])

  // Editor state. `editing` is null when adding, or the entry being edited.
  let editing = $state<VaultEntry | null>(null)
  let showForm = $state(false)
  let fName = $state('')
  let fUser = $state('')
  let fPass = $state('')

  async function refresh() {
    try {
      entries = await listVaultEntries()
    } catch {
      entries = []
    }
  }
  refresh()

  function openAdd() {
    editing = null
    fName = ''
    fUser = ''
    fPass = ''
    showForm = true
  }
  function openEdit(e: VaultEntry) {
    editing = e
    fName = e.name
    fUser = e.username ?? ''
    fPass = '' // blank = keep existing secret
    showForm = true
  }

  const canSave = $derived(!!fName.trim() && (editing !== null || !!fPass))

  async function save() {
    if (!canSave) return
    const name = fName.trim()
    const username = fUser.trim() || null
    try {
      if (editing) {
        await updateVaultEntry(editing.id, name, username, fPass ? fPass : null)
      } else {
        await createVaultEntry(name, username, fPass)
      }
      showForm = false
      await refresh()
    } catch (e) {
      showToast({ kind: 'error', title: 'Vault save failed', detail: errorText(e) })
    }
  }

  async function remove(e: VaultEntry) {
    try {
      await deleteVaultEntry(e.id)
      await refresh()
    } catch (err) {
      showToast({ kind: 'error', title: 'Vault delete failed', detail: errorText(err) })
    }
  }

  function onKeydown(ev: KeyboardEvent) {
    if (ev.key === 'Escape') {
      ev.stopPropagation()
      if (showForm) showForm = false
      else onClose()
    }
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Credential vault"
  tabindex="-1"
  onkeydown={onKeydown}
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose()
  }}
  transition:fade={{ duration: 120 }}
>
  <div
    class="dialog"
    style:background={theme.sidebarBg}
    style:border="1px solid {theme.border}"
    style:box-shadow={theme.windowShadow}
    style:font-family={theme.fontUi}
    transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}
  >
    <h3 style:color={theme.textPrimary}>Credential vault</h3>
    <p class="hint" style:color={theme.textDim}>
      Reusable credentials, referenced by sessions and macros. Secrets stay in the OS keyring.
    </p>

    {#if showForm}
      <div class="form">
        <input
          class="in"
          placeholder="Name"
          bind:value={fName}
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        <input
          class="in"
          placeholder="Username (optional)"
          bind:value={fUser}
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        <input
          class="in"
          type="password"
          placeholder={editing ? 'Password (leave blank to keep)' : 'Password'}
          bind:value={fPass}
          style:background={theme.bodyBg}
          style:color={theme.textPrimary}
          style:border="1px solid {theme.border}"
        />
        <div class="form-actions">
          <span class="spacer"></span>
          <button
            class="btn"
            style:color={theme.textMuted}
            style:border="1px solid {theme.border}"
            onclick={() => (showForm = false)}>Cancel</button
          >
          <button
            class="btn primary"
            disabled={!canSave}
            style:background={theme.accent}
            style:color={theme.onAccent}
            onclick={save}>{editing ? 'Save' : 'Add'}</button
          >
        </div>
      </div>
    {:else}
      <div class="list">
        {#if entries.length === 0}
          <p class="empty" style:color={theme.textMuted}>No vault entries yet.</p>
        {/if}
        {#each entries as e (e.id)}
          <div class="item" style:border="1px solid {theme.border}">
            <div class="meta">
              <span class="iname" style:color={theme.textPrimary}>{e.name}</span>
              {#if e.username}
                <span class="iuser" style:color={theme.textDim}>{e.username}</span>
              {/if}
            </div>
            <button
              class="iaction"
              style:color={theme.textMuted}
              title="Edit"
              onclick={() => openEdit(e)}>Edit</button
            >
            <button class="iaction" style:color={theme.err} title="Delete" onclick={() => remove(e)}
              >Delete</button
            >
          </div>
        {/each}
      </div>
    {/if}

    <div class="footer">
      {#if !showForm}
        <button
          class="btn"
          style:color={theme.textMuted}
          style:border="1px solid {theme.border}"
          onclick={openAdd}>+ Add entry</button
        >
      {/if}
      <span class="spacer"></span>
      <button
        class="btn"
        style:color={theme.textMuted}
        style:border="1px solid {theme.border}"
        onclick={onClose}>Close</button
      >
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
  }
  .dialog {
    width: 480px;
    max-width: 94vw;
    max-height: 88vh;
    overflow-y: auto;
    border-radius: 10px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.4;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .empty {
    font-size: 12px;
    margin: 4px 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 6px;
    padding: 6px 8px;
  }
  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .iname {
    font-size: 12.5px;
    font-weight: 500;
  }
  .iuser {
    font-size: 11px;
  }
  .iaction {
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 3px 8px;
    font-size: 11.5px;
    font-family: inherit;
    cursor: pointer;
    flex-shrink: 0;
  }
  .iaction:hover {
    filter: brightness(1.15);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .in {
    border-radius: 5px;
    padding: 5px 8px;
    font-size: 12.5px;
    font-family: inherit;
    outline: none;
  }
  .form-actions,
  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .spacer {
    flex: 1;
  }
  .btn {
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 5px 12px;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
  }
  .btn:hover {
    filter: brightness(1.1);
  }
  .btn.primary {
    font-weight: 600;
  }
  .btn.primary:disabled {
    opacity: 0.45;
    cursor: default;
    filter: none;
  }
</style>
