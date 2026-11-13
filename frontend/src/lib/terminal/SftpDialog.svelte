<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount, onDestroy } from 'svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { open as openDialog, save as saveDialog, ask } from '@tauri-apps/plugin-dialog'
  import {
    sftpCanonicalize,
    sftpListDir,
    sftpMkdir,
    sftpRemoveDir,
    sftpRemoveFile,
    sftpRename,
    sftpDownloadFile,
    sftpUploadFile,
    sftpCancelTransfer,
    sftpEditFile,
    type SftpProgressEvent,
  } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import type { SftpEntry } from '$lib/bridge/types'

  interface Props {
    sessionId: string
    onClose: () => void
  }

  let { sessionId, onClose }: Props = $props()

  let path = $state('/')
  let entries = $state<SftpEntry[]>([])
  let selected = $state<string | null>(null)
  let loading = $state(true)
  let error = $state('')
  let busy = $state('') // 'mkdir' | 'download' | 'upload' | ...

  // Inline-form state
  let mkdirName = $state('')
  let renameTo = $state('')
  let downloadTo = $state('')
  let uploadFrom = $state('')
  let uploadAs = $state('')

  let showMkdir = $state(false)
  let showRename = $state(false)
  let showDownload = $state(false)
  let showUpload = $state(false)

  // ── Transfer progress ────────────────────────────────────────────────────
  // Tracks ONE active transfer at a time (the SFTP browser has serial UI).
  // The Rust side emits `sftp-progress` every ~100 ms; we mirror it into
  // reactive state so the progress bar / cancel button can render.
  interface ActiveTransfer {
    id: string
    direction: 'download' | 'upload'
    name: string
    done: number
    total: number | null
    cancelling: boolean
  }
  let transfer = $state<ActiveTransfer | null>(null)
  let unlistenProgress: UnlistenFn | null = null

  const selectedEntry = $derived(entries.find((e) => e.name === selected) ?? null)

  function newTransferId(): string {
    // Browser crypto.randomUUID() is available in Tauri's webview.
    return globalThis.crypto?.randomUUID?.() ?? `t-${Date.now()}-${Math.random()}`
  }

  function fmtBytes(b: number): string {
    if (b < 1024) return `${b} B`
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
    if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`
    return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`
  }

  async function cancelTransfer() {
    if (!transfer || transfer.cancelling) return
    transfer.cancelling = true
    try {
      await sftpCancelTransfer(transfer.id)
    } catch {
      /* best-effort */
    }
  }

  onMount(async () => {
    // Subscribe to streaming-transfer progress events. The Rust side rate-
    // limits to ~10 Hz so updating state on every payload is cheap.
    const off = await listen<SftpProgressEvent>('sftp-progress', (e) => {
      if (!transfer || e.payload.transfer_id !== transfer.id) return
      transfer.done = e.payload.done
      transfer.total = e.payload.total
    })
    // Closed while `listen` was pending: onDestroy already ran.
    if (destroyed) {
      off()
      return
    }
    unlistenProgress = off
    try {
      path = await sftpCanonicalize(sessionId, '.')
    } catch (e) {
      error = fmt(e)
    }
    await refresh()
  })

  let destroyed = false
  onDestroy(() => {
    destroyed = true
    unlistenProgress?.()
  })

  /** List `path`; false (with the error shown) when it can't be listed. */
  async function refresh(): Promise<boolean> {
    loading = true
    error = ''
    try {
      entries = await sftpListDir(sessionId, path)
      selected = null
      return true
    } catch (e) {
      error = fmt(e)
      entries = []
      return false
    } finally {
      loading = false
    }
  }

  function joinPath(base: string, name: string): string {
    if (base === '/') return `/${name}`
    return `${base.replace(/\/+$/, '')}/${name}`
  }

  function parentOf(p: string): string {
    if (p === '/' || p === '') return '/'
    const trimmed = p.replace(/\/+$/, '')
    const i = trimmed.lastIndexOf('/')
    if (i <= 0) return '/'
    return trimmed.slice(0, i)
  }

  async function goTo(newPath: string) {
    path = newPath || '/'
    await refresh()
  }

  function onEntryClick(entry: SftpEntry) {
    selected = entry.name
  }

  async function onEntryActivate(entry: SftpEntry) {
    const full = joinPath(path, entry.name)
    if (entry.kind === 'dir') {
      await goTo(full)
    } else if (entry.kind === 'symlink') {
      // Symlinks could point anywhere: try to enter it as a directory; if it
      // can't be listed (a link to a file), stay here and offer the download.
      // (`refresh` reports failure instead of throwing, so a `.catch` never
      // ran and the browser was left inside an unlistable path.)
      const here = path
      path = full
      if (!(await refresh())) {
        path = here
        await refresh()
        selected = entry.name
        downloadTo = ''
        showDownload = true
      }
    } else {
      selected = entry.name
      downloadTo = ''
      showDownload = true
    }
  }

  async function doMkdir() {
    busy = 'mkdir'
    error = ''
    try {
      await sftpMkdir(sessionId, joinPath(path, mkdirName.trim()))
      mkdirName = ''
      showMkdir = false
      await refresh()
    } catch (e) {
      error = fmt(e)
    } finally {
      busy = ''
    }
  }

  async function doRename() {
    if (!selectedEntry) return
    busy = 'rename'
    error = ''
    try {
      const from = joinPath(path, selectedEntry.name)
      const to = joinPath(path, renameTo.trim())
      await sftpRename(sessionId, from, to)
      renameTo = ''
      showRename = false
      await refresh()
    } catch (e) {
      error = fmt(e)
    } finally {
      busy = ''
    }
  }

  /// Open the selected file in the OS editor; saves re-upload automatically
  /// (the backend watches the temp copy until the session closes).
  async function editSelected() {
    if (!selectedEntry || selectedEntry.kind === 'dir') return
    const full = joinPath(path, selectedEntry.name)
    try {
      await sftpEditFile(sessionId, full)
      showToast({
        kind: 'info',
        title: 'Editing remote file',
        detail: `${selectedEntry.name} — saves upload back automatically`,
      })
    } catch (e) {
      error = errorText(e)
    }
  }

  async function doDelete() {
    if (!selectedEntry) return
    const full = joinPath(path, selectedEntry.name)
    const isDir = selectedEntry.kind === 'dir'
    if (
      !(await ask(`Delete ${isDir ? 'directory' : 'file'} "${selectedEntry.name}"?`, {
        title: 'SFTP',
        kind: 'warning',
      }))
    )
      return
    busy = 'delete'
    error = ''
    try {
      if (isDir) {
        await sftpRemoveDir(sessionId, full)
      } else {
        await sftpRemoveFile(sessionId, full)
      }
      await refresh()
    } catch (e) {
      error = fmt(e)
    } finally {
      busy = ''
    }
  }

  async function doDownload() {
    if (!selectedEntry || !downloadTo.trim()) return
    busy = 'download'
    error = ''
    const id = newTransferId()
    transfer = {
      id,
      direction: 'download',
      name: selectedEntry.name,
      done: 0,
      total: selectedEntry.size ?? null,
      cancelling: false,
    }
    try {
      const n = await sftpDownloadFile(
        sessionId,
        joinPath(path, selectedEntry.name),
        downloadTo.trim(),
        id,
      )
      showDownload = false
      downloadTo = ''
      error = `Downloaded ${fmtBytes(n)}.`
    } catch (e) {
      const msg = fmt(e)
      error = msg.includes('cancelled') ? 'Download cancelled.' : msg
    } finally {
      busy = ''
      transfer = null
    }
  }

  async function doUpload() {
    if (!uploadFrom.trim()) return
    const remoteName = uploadAs.trim() || baseName(uploadFrom.trim())
    if (!remoteName) {
      error = 'Remote name required.'
      return
    }
    // The upload writes the remote file in place: an existing one is replaced
    // (and left truncated if the upload is cancelled), so ask first.
    const existing = entries.find((e) => e.name === remoteName)
    if (existing?.kind === 'dir') {
      error = `"${remoteName}" is a folder here — choose another name.`
      return
    }
    if (
      existing &&
      !(await ask(`"${remoteName}" already exists here. Replace it?`, {
        title: 'SFTP',
        kind: 'warning',
      }))
    )
      return
    busy = 'upload'
    error = ''
    const id = newTransferId()
    transfer = {
      id,
      direction: 'upload',
      name: remoteName,
      done: 0,
      // Local size is known to the OS but not to JS without a `stat` call;
      // the Rust streaming side reports it back on the first progress event.
      total: null,
      cancelling: false,
    }
    try {
      const n = await sftpUploadFile(sessionId, uploadFrom.trim(), joinPath(path, remoteName), id)
      showUpload = false
      uploadFrom = ''
      uploadAs = ''
      error = `Uploaded ${fmtBytes(n)}.`
      await refresh()
    } catch (e) {
      const msg = fmt(e)
      error = msg.includes('cancelled') ? 'Upload cancelled.' : msg
    } finally {
      busy = ''
      transfer = null
    }
  }

  function baseName(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? ''
  }

  function fmt(e: unknown): string {
    return errorText(e)
  }

  /// Native picker for "where do I save this remote file locally?".
  async function pickDownloadDestination() {
    if (!selectedEntry) return
    try {
      const target = await saveDialog({
        title: 'Save remote file as…',
        defaultPath: selectedEntry.name,
      })
      if (target) downloadTo = target
    } catch (e) {
      error = fmt(e)
    }
  }

  /// Native picker for "which local file should I upload?".
  async function pickUploadSource() {
    try {
      const selected = await openDialog({
        title: 'Choose file to upload',
        multiple: false,
        directory: false,
      })
      if (typeof selected === 'string' && selected.length > 0) {
        uploadFrom = selected
        if (!uploadAs.trim()) uploadAs = baseName(selected)
      }
    } catch (e) {
      error = fmt(e)
    }
  }

  function fmtSize(n: number | null): string {
    if (n == null) return '–'
    if (n < 1024) return `${n} B`
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
    if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
    return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
  }

  function fmtMode(perm: number | null, kind: string): string {
    const prefix = kind === 'dir' ? 'd' : kind === 'symlink' ? 'l' : '-'
    if (perm == null) return `${prefix}---------`
    const bit = (b: number) => (perm & b ? 1 : 0)
    const triplet = (r: number, w: number, x: number) =>
      `${bit(r) ? 'r' : '-'}${bit(w) ? 'w' : '-'}${bit(x) ? 'x' : '-'}`
    return (
      prefix +
      triplet(0o400, 0o200, 0o100) +
      triplet(0o040, 0o020, 0o010) +
      triplet(0o004, 0o002, 0o001)
    )
  }

  function fmtMtime(s: number | null): string {
    if (s == null) return '–'
    const d = new Date(s * 1000)
    return d.toISOString().slice(0, 16).replace('T', ' ')
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog">
    <div class="header">
      <h2>SFTP browser</h2>
      <div class="header-right">
        <button class="btn" onclick={refresh} disabled={loading} title="Refresh"
          ><Icon name="refresh" size={12} /></button
        >
        <button class="btn" onclick={onClose} title="Close"><Icon name="x" size={12} /></button>
      </div>
    </div>

    <div class="pathbar">
      <button class="btn" onclick={() => goTo(parentOf(path))} title="Up">↑</button>
      <input
        class="path"
        bind:value={path}
        onkeydown={(e) => {
          if (e.key === 'Enter') refresh()
        }}
        spellcheck="false"
      />
      <button class="btn primary" onclick={refresh} disabled={loading}>Go</button>
    </div>

    <div class="actions">
      <button
        class="btn"
        onclick={() => {
          showMkdir = true
          showRename = false
          showDownload = false
          showUpload = false
        }}
      >
        + Folder
      </button>
      <button
        class="btn"
        onclick={() => {
          showUpload = true
          showMkdir = false
          showRename = false
          showDownload = false
          uploadAs = ''
        }}
      >
        ⬆ Upload
      </button>
      {#if selectedEntry}
        {#if selectedEntry.kind === 'file' || selectedEntry.kind === 'symlink'}
          <button
            class="btn"
            onclick={() => {
              showDownload = true
              showMkdir = false
              showRename = false
              showUpload = false
              downloadTo = ''
            }}
          >
            ⬇ Download
          </button>
          <button
            class="btn"
            onclick={editSelected}
            title="Open in your editor; saves upload back automatically"
          >
            <Icon name="pencil" size={12} /> Edit
          </button>
        {/if}
        <button
          class="btn"
          onclick={() => {
            showRename = true
            showMkdir = false
            showDownload = false
            showUpload = false
            renameTo = selectedEntry?.name ?? ''
          }}
        >
          <Icon name="pencil" size={12} /> Rename
        </button>
        <button class="btn danger" onclick={doDelete} disabled={busy === 'delete'}>
          <Icon name="x" size={12} /> Delete
        </button>
      {/if}
    </div>

    {#if showMkdir}
      <form
        class="inline-form"
        onsubmit={(e) => {
          e.preventDefault()
          doMkdir()
        }}
      >
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={mkdirName} placeholder="New folder name" spellcheck="false" autofocus />
        <button class="btn primary" type="submit" disabled={!mkdirName.trim() || busy === 'mkdir'}
          >Create</button
        >
        <button class="btn" type="button" onclick={() => (showMkdir = false)}>Cancel</button>
      </form>
    {/if}
    {#if showRename && selectedEntry}
      <form
        class="inline-form"
        onsubmit={(e) => {
          e.preventDefault()
          doRename()
        }}
      >
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={renameTo} placeholder="New name" spellcheck="false" autofocus />
        <button class="btn primary" type="submit" disabled={!renameTo.trim() || busy === 'rename'}
          >Rename</button
        >
        <button class="btn" type="button" onclick={() => (showRename = false)}>Cancel</button>
      </form>
    {/if}
    {#if showDownload && selectedEntry}
      <form
        class="inline-form"
        onsubmit={(e) => {
          e.preventDefault()
          doDownload()
        }}
      >
        <span class="form-label">Local path:</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:value={downloadTo}
          placeholder="/abs/path/to/save/{selectedEntry.name}"
          spellcheck="false"
          autofocus
        />
        <button class="btn" type="button" onclick={pickDownloadDestination}>Browse…</button>
        <button
          class="btn primary"
          type="submit"
          disabled={!downloadTo.trim() || busy === 'download'}
        >
          {busy === 'download' ? 'Downloading…' : 'Download'}
        </button>
        <button class="btn" type="button" onclick={() => (showDownload = false)}>Cancel</button>
      </form>
    {/if}
    {#if showUpload}
      <form
        class="inline-form col"
        onsubmit={(e) => {
          e.preventDefault()
          doUpload()
        }}
      >
        <div class="form-row">
          <span class="form-label">Local file:</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            bind:value={uploadFrom}
            placeholder="/abs/path/to/local/file"
            spellcheck="false"
            autofocus
          />
          <button class="btn" type="button" onclick={pickUploadSource}>Browse…</button>
        </div>
        <div class="form-row">
          <span class="form-label">Save as:</span>
          <input
            bind:value={uploadAs}
            placeholder="(defaults to local file name)"
            spellcheck="false"
          />
          <button
            class="btn primary"
            type="submit"
            disabled={!uploadFrom.trim() || busy === 'upload'}
          >
            {busy === 'upload' ? 'Uploading…' : 'Upload'}
          </button>
          <button class="btn" type="button" onclick={() => (showUpload = false)}>Cancel</button>
        </div>
      </form>
    {/if}

    {#if error}<p class="msg">{error}</p>{/if}

    {#if transfer}
      <div class="xfer">
        <div class="xfer-row">
          <span class="xfer-tag" class:up={transfer.direction === 'upload'}>
            {transfer.direction === 'upload' ? '↑' : '↓'}
          </span>
          <span class="xfer-name" title={transfer.name}>{transfer.name}</span>
          <span class="xfer-stats">
            {fmtBytes(transfer.done)}{#if transfer.total}
              / {fmtBytes(transfer.total)}{/if}
          </span>
          <button
            class="xfer-cancel"
            type="button"
            onclick={cancelTransfer}
            disabled={transfer.cancelling}
            title="Cancel transfer"
          >
            {#if transfer.cancelling}…{:else}<Icon name="x" size={12} />{/if}
          </button>
        </div>
        <div class="xfer-bar">
          <div
            class="xfer-bar-fill"
            class:indeterminate={transfer.total === null}
            style:width={transfer.total
              ? `${Math.min(100, (transfer.done / transfer.total) * 100)}%`
              : '100%'}
          ></div>
        </div>
      </div>
    {/if}

    <div class="listing">
      {#if loading}
        <p class="muted center">Loading…</p>
      {:else if entries.length === 0}
        <p class="muted center">Empty directory.</p>
      {:else}
        <table>
          <thead>
            <tr>
              <th class="col-name">Name</th>
              <th class="col-size">Size</th>
              <th class="col-mode">Mode</th>
              <th class="col-mtime">Modified</th>
            </tr>
          </thead>
          <tbody>
            {#each entries as e (e.name)}
              <tr
                class:selected={selected === e.name}
                onclick={() => onEntryClick(e)}
                ondblclick={() => onEntryActivate(e)}
              >
                <td class="col-name">
                  <span class="icon">
                    {#if e.kind === 'dir'}<Icon
                        name="folder"
                        size={13}
                      />{:else if e.kind === 'symlink'}<Icon name="link" size={13} />{:else}<Icon
                        name="file"
                        size={13}
                      />{/if}
                  </span>
                  {e.name}
                </td>
                <td class="col-size num">{e.kind === 'dir' ? '–' : fmtSize(e.size)}</td>
                <td class="col-mode num">{fmtMode(e.permissions, e.kind)}</td>
                <td class="col-mtime num">{fmtMtime(e.mtime)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
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
    width: 48rem;
    max-width: 95vw;
    height: 36rem;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
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

  .pathbar,
  .actions {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .actions {
    flex-wrap: wrap;
  }

  .path {
    flex: 1;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.85rem;
    padding: 0.35rem 0.55rem;
    outline: none;
    font-family: var(--zx-font-mono);
  }

  .path:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
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
    flex-shrink: 0;
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
    filter: brightness(1.1);
  }

  .btn.danger {
    border-color: var(--zx-err);
    color: var(--zx-err);
  }

  .btn.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .inline-form {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.5rem 0.6rem;
  }

  .inline-form.col {
    flex-direction: column;
    align-items: stretch;
  }

  .form-row {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .form-label {
    font-size: 0.75rem;
    color: var(--zx-text-muted);
    min-width: 5rem;
    flex-shrink: 0;
  }

  .inline-form input {
    flex: 1;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.82rem;
    padding: 0.3rem 0.5rem;
    outline: none;
    font-family: var(--zx-font-mono);
  }

  .inline-form input:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }

  .msg {
    font-size: 0.8rem;
    color: var(--zx-err);
    margin: 0;
  }

  .listing {
    flex: 1;
    overflow: auto;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--zx-surface-2);
    color: var(--zx-text-muted);
    text-align: left;
    font-weight: 500;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.4rem 0.55rem;
    border-bottom: 1px solid var(--zx-border);
  }

  td {
    padding: 0.35rem 0.55rem;
    color: var(--zx-text);
    border-bottom: 1px solid var(--zx-border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  tr {
    cursor: pointer;
  }

  tr:hover td {
    background: var(--zx-hover-bg);
  }

  tr.selected td {
    background: var(--zx-active-bg);
    color: var(--zx-text);
  }

  .col-name {
    width: 45%;
  }
  .col-size,
  .col-mode,
  .col-mtime {
    width: 18%;
  }
  .icon {
    margin-right: 0.35rem;
  }
  .num {
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
  }

  .muted {
    color: var(--zx-text-muted);
    font-size: 0.82rem;
  }

  .center {
    text-align: center;
    padding: 1.5rem 0;
  }

  .xfer {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.45rem 0.6rem;
  }

  .xfer-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.78rem;
  }

  .xfer-tag {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 700;
    border-radius: 0.2rem;
    padding: 0 0.4rem;
    font-size: 0.78rem;
    line-height: 1.4;
  }

  .xfer-tag.up {
    background: var(--zx-ok);
  }

  .xfer-name {
    flex: 1;
    color: var(--zx-text);
    font-family: var(--zx-font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .xfer-stats {
    color: var(--zx-text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .xfer-cancel {
    background: transparent;
    border: none;
    color: var(--zx-text-muted);
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    padding: 0.15rem 0.35rem;
    border-radius: 0.2rem;
    font-family: inherit;
  }

  .xfer-cancel:hover:not(:disabled) {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
  }

  .xfer-bar {
    width: 100%;
    height: 4px;
    background: color-mix(in srgb, var(--zx-text) 10%, transparent);
    border-radius: 2px;
    overflow: hidden;
  }

  .xfer-bar-fill {
    height: 100%;
    background: var(--zx-accent);
    transition: width 0.15s ease-out;
  }

  .xfer-bar-fill.indeterminate {
    animation: xfer-indet 1.3s linear infinite;
    background: linear-gradient(90deg, transparent 0%, var(--zx-accent) 50%, transparent 100%);
    background-size: 40% 100%;
    background-repeat: no-repeat;
  }

  @keyframes xfer-indet {
    0% {
      background-position: -40% 0;
    }
    100% {
      background-position: 140% 0;
    }
  }
</style>
