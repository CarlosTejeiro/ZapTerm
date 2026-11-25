<script lang="ts">
  /**
   * File → Add devices in bulk…: paste a list or open a CSV file, set the
   * values the list leaves out, check the preview and add them all as saved
   * sessions. The backend checks every row (duplicates, folders, vault
   * credential, jump host) and creates nothing while any row has an error.
   */
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
  import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager'
  import {
    bulkAddSessions,
    listVaultEntries,
    readDeviceListFile,
    type BulkResult,
  } from '$lib/bridge/commands'
  import { errorText } from '$lib/bridge/errors'
  import type { Folder, SavedSession, VaultEntry } from '$lib/bridge/types'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import {
    CSV_TEMPLATE,
    DEFAULT_PORT,
    parseDeviceList,
    type BulkDefaults,
    type ParseResult,
    type Protocol,
  } from './bulkParse'

  interface Props {
    sessions: SavedSession[]
    folders: Folder[]
    onClose: () => void
    /** Sessions were added: refresh the sidebar. */
    onAdded: () => void
  }

  const { sessions, folders, onClose, onAdded }: Props = $props()

  let text = $state('')
  let source = $state('')
  let protocol = $state<Protocol>('ssh')
  let portText = $state('')
  let username = $state('')
  let folder = $state('')
  let tagsText = $state('')
  let credential = $state('')
  let jumpHost = $state('')
  let vault = $state<VaultEntry[]>([])
  let check = $state<BulkResult | null>(null)
  let checkError = $state('')
  let checking = $state(false)
  let adding = $state(false)

  $effect(() => {
    listVaultEntries()
      .then((v) => (vault = v))
      .catch(() => (vault = []))
  })

  /** Every folder as a `/` path, for the folder suggestions. */
  const folderPaths = $derived.by(() => {
    const byId = new Map(folders.map((f) => [f.id, f]))
    const path = (f: Folder): string => {
      const parent = f.parent_id != null ? byId.get(f.parent_id) : undefined
      return parent ? `${path(parent)}/${f.name}` : f.name
    }
    return folders.map(path).sort((a, b) => a.localeCompare(b))
  })
  const sshSessions = $derived(
    sessions.filter((s) => s.protocol === 'ssh').sort((a, b) => a.name.localeCompare(b.name)),
  )

  const portProblem = $derived.by(() => {
    const t = portText.trim()
    if (!t) return ''
    const n = Number(t)
    return /^\d+$/.test(t) && n >= 1 && n <= 65535 ? '' : 'Port must be 1–65535'
  })

  const defaults = $derived<BulkDefaults>({
    protocol,
    port: portText.trim() && !portProblem ? Number(portText.trim()) : null,
    username,
    folder,
    tags: tagsText
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean),
    credential,
    jumpHost,
  })

  const parsed = $derived<ParseResult>(parseDeviceList(text, defaults))
  /** Rows the parser could read — the ones sent to the backend. */
  const readable = $derived(parsed.rows.filter((r) => !r.problem))

  // Ask the backend about the readable rows whenever they change (debounced).
  $effect(() => {
    const rows = readable.map((r) => r.row)
    checkError = ''
    if (rows.length === 0) {
      check = null
      return
    }
    checking = true
    const t = setTimeout(() => {
      bulkAddSessions(rows, false)
        .then((r) => (check = r))
        .catch((e) => {
          check = null
          checkError = errorText(e)
        })
        .finally(() => (checking = false))
    }, 250)
    return () => clearTimeout(t)
  })

  interface PreviewRow {
    line: number
    name: string
    host: string
    port: number
    protocol: string
    folder: string
    status: 'new' | 'exists' | 'error' | 'pending'
    message: string
  }

  const preview = $derived.by<PreviewRow[]>(() => {
    let k = 0
    return parsed.rows.map((r) => {
      const base = {
        line: r.line,
        name: r.row.name,
        host: r.row.host,
        port: r.row.port,
        protocol: r.row.protocol,
        folder: r.row.folder ?? '',
      }
      if (r.problem) return { ...base, status: 'error', message: r.problem }
      const res = check?.rows[k++]
      if (!res) return { ...base, status: 'pending', message: '' }
      return { ...base, status: res.status, message: res.message ?? '' }
    })
  })

  const counts = $derived.by(() => {
    const c = { new: 0, exists: 0, error: 0, pending: 0 }
    for (const r of preview) c[r.status]++
    return c
  })
  const newFolderPaths = $derived.by(() => {
    if (!check) return []
    const set = new Set<string>()
    let k = 0
    for (const r of parsed.rows) {
      if (r.problem) continue
      const res = check.rows[k++]
      if (res?.status === 'new' && res.newFolders.length) {
        const all = (r.row.folder ?? '')
          .split('/')
          .map((s) => s.trim())
          .filter(Boolean)
        set.add(all.join('/'))
      }
    }
    return [...set]
  })
  const canAdd = $derived(
    !adding &&
      !checking &&
      !portProblem &&
      check !== null &&
      counts.error === 0 &&
      counts.pending === 0 &&
      counts.new > 0,
  )

  async function openCsv() {
    const picked = await openFileDialog({
      title: 'Open a device list',
      multiple: false,
      directory: false,
      filters: [{ name: 'Device list', extensions: ['csv', 'txt', 'tsv'] }],
    })
    if (typeof picked !== 'string' || !picked) return
    try {
      text = await readDeviceListFile(picked)
      source = picked.split(/[\\/]/).pop() ?? picked
    } catch (e) {
      showToast({ kind: 'error', title: 'Could not read the file', detail: errorText(e) })
    }
  }

  async function copyTemplate() {
    try {
      await clipboardWriteText(CSV_TEMPLATE)
    } catch {
      await navigator.clipboard.writeText(CSV_TEMPLATE).catch(() => {})
    }
    showToast({
      kind: 'info',
      title: 'CSV template copied',
      detail: 'name, host, port, protocol, username, folder, tags, notes, credential, jump_host',
    })
  }

  async function add() {
    if (!canAdd) return
    adding = true
    try {
      const r = await bulkAddSessions(
        readable.map((x) => x.row),
        true,
      )
      if (!r.applied) {
        check = r
        showToast({
          kind: 'warning',
          title: 'Nothing was added',
          detail: 'The inventory changed meanwhile — check the rows marked in the list.',
        })
        return
      }
      showToast({
        kind: 'success',
        title: `Added ${r.added} device${r.added === 1 ? '' : 's'}`,
        detail:
          `${r.skipped} already in the inventory` +
          (r.foldersAdded
            ? ` · ${r.foldersAdded} new folder${r.foldersAdded === 1 ? '' : 's'}`
            : ''),
      })
      onAdded()
      onClose()
    } catch (e) {
      showToast({ kind: 'error', title: 'Adding devices failed', detail: errorText(e) })
    } finally {
      adding = false
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
    }
  }

  const statusLabel = { new: 'New', exists: 'Exists', error: 'Error', pending: '…' }
</script>

<div class="overlay" role="presentation" onkeydown={onKeydown}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Add devices in bulk"
    tabindex="-1"
  >
    <header>
      <h3>Add devices in bulk</h3>
      <p class="muted">
        Paste one device per line — <code>host</code>, <code>host:port</code>,
        <code>name host</code> or <code>name host port</code> — or a CSV whose first line names the
        columns. Lines starting with <code>#</code> are skipped.
      </p>
    </header>

    <div class="body">
      <section class="input">
        <div class="row">
          <button type="button" class="ghost" onclick={openCsv}>Open CSV…</button>
          <button type="button" class="ghost" onclick={copyTemplate}>Copy CSV template</button>
          {#if source}<span class="source" title={source}>{source}</span>{/if}
        </div>
        <!-- svelte-ignore a11y_autofocus -->
        <textarea
          bind:value={text}
          oninput={() => (source = '')}
          spellcheck="false"
          autofocus
          aria-label="Device list"
          placeholder={'core-sw-01 10.10.0.2\ncore-sw-02 10.10.0.3\n10.20.0.1:2222\n\n# or CSV:\nname,host,folder,tags\nedge-01,192.0.2.1,WAN,"edge,mad"'}
        ></textarea>
        {#if parsed.kind === 'csv' && parsed.ignoredColumns.length}
          <p class="muted small">
            Columns not used: {parsed.ignoredColumns.join(', ')}
          </p>
        {/if}
      </section>

      <section class="defaults" aria-label="Defaults for values the list leaves out">
        <p class="label">For values the list leaves out</p>
        <label>
          <span>Protocol</span>
          <select bind:value={protocol}>
            <option value="ssh">SSH</option>
            <option value="telnet">Telnet</option>
          </select>
        </label>
        <label>
          <span>Port</span>
          <input
            bind:value={portText}
            placeholder={String(DEFAULT_PORT[protocol])}
            inputmode="numeric"
          />
        </label>
        {#if portProblem}<p class="err small">{portProblem}</p>{/if}
        <label>
          <span>Username</span>
          <input bind:value={username} placeholder="SSH only" spellcheck="false" />
        </label>
        <label>
          <span>Folder</span>
          <input bind:value={folder} list="bulk-folders" placeholder="Top level — or DC/Madrid" />
          <datalist id="bulk-folders">
            {#each folderPaths as p (p)}<option value={p}></option>{/each}
          </datalist>
        </label>
        <label>
          <span>Tags</span>
          <input bind:value={tagsText} placeholder="core, prod" />
        </label>
        <label>
          <span>Credential</span>
          <select bind:value={credential}>
            <option value="">Ask on first connect</option>
            {#each vault as v (v.id)}<option value={v.name}>{v.name}</option>{/each}
          </select>
        </label>
        <label>
          <span>Jump host</span>
          <select bind:value={jumpHost}>
            <option value="">None</option>
            {#each sshSessions as s (s.id)}<option value={s.name}>{s.name}</option>{/each}
          </select>
        </label>
        <p class="muted small">
          A CSV column wins over these. Credential and jump host apply to SSH only; the password
          stays in the vault.
        </p>
      </section>
    </div>

    <section class="preview" aria-label="Preview">
      <p class="summary" aria-live="polite">
        {#if parsed.kind === 'empty'}
          Nothing to add yet.
        {:else}
          <b>{counts.new}</b> new · {counts.exists} already in the inventory ·
          <span class:err={counts.error > 0}>{counts.error} with errors</span>
          {#if newFolderPaths.length}
            · new folders: {newFolderPaths.join(', ')}
          {/if}
          {#if checking}<span class="muted"> · checking…</span>{/if}
        {/if}
      </p>
      {#if checkError}<p class="err small">{checkError}</p>{/if}
      {#if preview.length}
        <div class="table">
          <table>
            <thead>
              <tr
                ><th>Line</th><th>Name</th><th>Host</th><th>Port</th><th>Protocol</th><th>Folder</th
                ><th>Status</th></tr
              >
            </thead>
            <tbody>
              {#each preview as r (r.line)}
                <tr class={r.status}>
                  <td class="num">{r.line}</td>
                  <td>{r.name}</td>
                  <td class="mono">{r.host}</td>
                  <td class="num">{r.port}</td>
                  <td>{r.protocol}</td>
                  <td>{r.folder}</td>
                  <td>
                    <span class="chip {r.status}">{statusLabel[r.status]}</span>
                    {#if r.message}<span class="msg">{r.message}</span>{/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>

    <div class="actions">
      <span class="muted small">
        {#if counts.error > 0}Fix the rows with errors — nothing is added while any row has one.{/if}
      </span>
      <button type="button" class="cancel" onclick={onClose}>Cancel</button>
      <button type="button" class="run" onclick={add} disabled={!canAdd}>
        {adding ? 'Adding…' : `Add ${counts.new} device${counts.new === 1 ? '' : 's'}`}
      </button>
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
    z-index: 1100;
  }
  .dialog {
    width: 60rem;
    max-width: 94vw;
    max-height: 90vh;
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 10px;
    padding: var(--zx-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-3);
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
    overflow: hidden;
  }
  h3 {
    margin: 0 0 var(--zx-space-1);
    font-size: 0.95rem;
    font-weight: 600;
  }
  code,
  .mono {
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
  }
  .muted {
    margin: 0;
    color: var(--zx-text-muted);
    font-size: 0.8rem;
    line-height: 1.4;
  }
  .small {
    font-size: 0.75rem;
  }
  .err {
    color: var(--zx-err);
    margin: 0;
  }
  .body {
    display: grid;
    grid-template-columns: 1fr 17rem;
    gap: var(--zx-space-3);
    min-height: 0;
  }
  .input {
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-2);
    min-width: 0;
  }
  .row {
    display: flex;
    gap: var(--zx-space-1);
    align-items: center;
  }
  .source {
    font-size: 0.75rem;
    color: var(--zx-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  textarea {
    min-height: 11rem;
    flex: 1;
    resize: vertical;
    background: color-mix(in srgb, var(--zx-text) 5%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 6px;
    padding: var(--zx-space-2);
    color: var(--zx-text);
    font-family: var(--zx-font-mono);
    font-size: 0.8rem;
    line-height: 1.45;
    outline: none;
  }
  textarea:focus-visible,
  input:focus-visible,
  select:focus-visible {
    border-color: var(--zx-accent);
  }
  .defaults {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: var(--zx-space-2);
    border: 1px solid var(--zx-border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--zx-text) 3%, transparent);
  }
  .label {
    margin: 0 0 0.15rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--zx-text-muted);
  }
  .defaults label {
    display: grid;
    grid-template-columns: 5.5rem 1fr;
    align-items: center;
    gap: var(--zx-space-1);
    font-size: 0.8rem;
    color: var(--zx-text-muted);
  }
  input,
  select {
    min-width: 0;
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 5px;
    padding: 0.3rem var(--zx-space-2);
    color: var(--zx-text);
    font-family: inherit;
    font-size: 0.8rem;
    outline: none;
  }
  .preview {
    display: flex;
    flex-direction: column;
    gap: var(--zx-space-1);
    min-height: 0;
  }
  .summary {
    margin: 0;
    font-size: 0.82rem;
  }
  .table {
    overflow: auto;
    max-height: 14rem;
    border: 1px solid var(--zx-border);
    border-radius: 6px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
  }
  th,
  td {
    text-align: left;
    padding: 0.3rem var(--zx-space-2);
    border-bottom: 1px solid var(--zx-border);
    white-space: nowrap;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--zx-surface);
    color: var(--zx-text-muted);
    font-weight: 600;
  }
  td.num {
    font-variant-numeric: tabular-nums;
    color: var(--zx-text-muted);
  }
  tr.exists td {
    color: var(--zx-text-muted);
  }
  .chip {
    display: inline-block;
    padding: 0 0.45rem;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 600;
    margin-right: 0.35rem;
  }
  .chip.new {
    background: color-mix(in srgb, var(--zx-ok, #22c55e) 18%, transparent);
    color: var(--zx-ok, #22c55e);
  }
  .chip.exists,
  .chip.pending {
    background: var(--zx-hover-bg);
    color: var(--zx-text-muted);
  }
  .chip.error {
    background: color-mix(in srgb, var(--zx-err) 18%, transparent);
    color: var(--zx-err);
  }
  .msg {
    color: var(--zx-text-muted);
    white-space: normal;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: var(--zx-space-1);
  }
  .actions > .muted {
    margin-right: auto;
  }
  button {
    border: 0;
    border-radius: 5px;
    padding: var(--zx-space-1) var(--zx-space-3);
    font-size: 0.8rem;
    font-family: inherit;
    cursor: pointer;
  }
  .ghost {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .cancel {
    background: var(--zx-hover-bg);
    color: var(--zx-text-muted);
  }
  .run {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    font-weight: 600;
  }
  .run:disabled {
    opacity: 0.5;
    cursor: default;
  }
  @media (max-width: 760px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
</style>
