<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { ask, open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'
  import {
    BACKUP_EXTENSION,
    BACKUP_MIN_PASSPHRASE,
    backupExport,
    backupInspect,
    backupRestore,
    backupSyncConfigure,
    backupSyncDisable,
    backupSyncNow,
    backupSyncStatus,
    restartApp,
    type BundleInfo,
    type SyncStatus,
  } from '$lib/bridge/commands'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import PassphraseDialog from '$lib/ui/PassphraseDialog.svelte'
  import { SYNC_STATE_LABELS, handleSyncStatus } from '$lib/backup/sync'

  let busy = $state(false)

  /// Which passphrase prompt is open, and the file/folder it applies to.
  type Prompt = { mode: 'export' | 'restore' | 'sync'; path: string }
  let prompt = $state<Prompt | null>(null)

  // ── sync folder ──────────────────────────────────────────────────────────

  let sync = $state<SyncStatus | null>(null)
  let syncBusy = $state(false)

  onMount(refreshSync)

  async function refreshSync() {
    try {
      sync = await backupSyncStatus()
    } catch (e) {
      showToast({ kind: 'error', title: 'Sync', detail: errorText(e) })
    }
  }

  function fmt(ts: string | null): string {
    if (!ts) return 'never'
    const d = new Date(ts)
    return isNaN(d.getTime()) ? ts : d.toLocaleString()
  }

  /// Pick the folder; ask for a passphrase unless one is already stored (a
  /// folder change keeps it).
  async function chooseSyncFolder() {
    if (syncBusy) return
    const dir = await openDialog({
      directory: true,
      title: 'Sync folder (inside your cloud folder)',
    })
    if (typeof dir !== 'string' || !dir) return
    if (sync?.has_passphrase) {
      await applySync(dir, null)
    } else {
      prompt = { mode: 'sync', path: dir }
    }
  }

  function changeSyncPassphrase() {
    if (syncBusy || !sync?.dir) return
    prompt = { mode: 'sync', path: sync.dir }
  }

  async function applySync(dir: string, passphrase: string | null) {
    syncBusy = true
    try {
      const st = await backupSyncConfigure(dir, passphrase)
      sync = st
      if (st.state === 'local_changes') {
        // Fresh folder: publish right away so the other devices have something to pull.
        sync = await backupSyncNow()
        showToast({ kind: 'success', title: 'Sync', detail: `Published to ${dir}` })
      } else {
        sync = await handleSyncStatus(st)
      }
    } catch (e) {
      showToast({ kind: 'error', title: 'Sync', detail: errorText(e) })
    } finally {
      syncBusy = false
    }
  }

  async function syncNow() {
    if (syncBusy) return
    syncBusy = true
    try {
      const st = await backupSyncNow()
      sync = await handleSyncStatus(st)
      if (sync.state === 'in_sync') {
        showToast({ kind: 'success', title: 'Sync', detail: 'Everything is up to date.' })
      }
    } catch (e) {
      showToast({ kind: 'error', title: 'Sync', detail: errorText(e) })
    } finally {
      syncBusy = false
    }
  }

  async function disableSync() {
    if (syncBusy) return
    const yes = await ask(
      'Stop syncing? The bundle already in the folder is left as it is; the stored passphrase is forgotten on this device.',
      { title: 'Disable sync', kind: 'warning' },
    )
    if (!yes) return
    syncBusy = true
    try {
      sync = await backupSyncDisable()
    } catch (e) {
      showToast({ kind: 'error', title: 'Sync', detail: errorText(e) })
    } finally {
      syncBusy = false
    }
  }

  function defaultFileName(): string {
    const d = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    return `zapterm-backup-${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}.${BACKUP_EXTENSION}`
  }

  function describe(i: BundleInfo): string {
    return (
      `${i.sessions} sessions, ${i.folders} folders, ${i.groups} groups, ` +
      `${i.snippets} snippets, ${i.rules} rules, ${i.vault_entries} vault entries, ` +
      `${i.session_credentials} session credentials, ${i.settings} settings, ` +
      `${i.known_hosts_lines} known hosts`
    )
  }

  function warn(title: string, warnings: string[]) {
    if (warnings.length === 0) return
    showToast({
      kind: 'warning',
      title: `${title}: ${warnings.length} warning(s)`,
      detail: warnings.slice(0, 3).join(' · ') + (warnings.length > 3 ? ' …' : ''),
      ttl: 0,
    })
  }

  async function chooseExportFile() {
    if (busy) return
    const path = await saveDialog({
      title: 'Save encrypted backup',
      defaultPath: defaultFileName(),
      filters: [{ name: 'ZapTerm backup', extensions: [BACKUP_EXTENSION] }],
    })
    if (!path) return
    prompt = { mode: 'export', path }
  }

  async function chooseRestoreFile() {
    if (busy) return
    const path = await openDialog({
      title: 'Open encrypted backup',
      multiple: false,
      directory: false,
      filters: [{ name: 'ZapTerm backup', extensions: [BACKUP_EXTENSION] }],
    })
    if (typeof path !== 'string' || !path) return
    prompt = { mode: 'restore', path }
  }

  async function onPassphrase(passphrase: string) {
    const p = prompt
    prompt = null
    if (!p) return
    if (p.mode === 'sync') {
      await applySync(p.path, passphrase)
      return
    }
    busy = true
    try {
      if (p.mode === 'export') await doExport(p.path, passphrase)
      else await doRestore(p.path, passphrase)
    } finally {
      busy = false
    }
  }

  async function doExport(path: string, passphrase: string) {
    try {
      const s = await backupExport(path, passphrase)
      showToast({
        kind: 'success',
        title: 'Backup saved',
        detail: `${describe(s.info)} → ${s.path}`,
      })
      warn('Backup', s.warnings)
    } catch (e) {
      showToast({ kind: 'error', title: 'Backup failed', detail: errorText(e) })
    }
  }

  async function doRestore(path: string, passphrase: string) {
    let info: BundleInfo
    try {
      info = await backupInspect(path, passphrase)
    } catch (e) {
      showToast({ kind: 'error', title: 'Cannot open backup', detail: errorText(e) })
      return
    }
    const when = info.created_at ? new Date(info.created_at).toLocaleString() : 'unknown date'
    const proceed = await ask(
      `Backup from "${info.device_name}" (ZapTerm ${info.app_version}, ${when}):\n\n` +
        `${describe(info)}.\n\n` +
        'Sessions that already exist here are kept as they are. Vault entries this device ' +
        'already has keep their password (only a missing one is filled in). Settings from the ' +
        'backup replace the current ones.\n\n' +
        'Restore now?',
      { title: 'Restore backup', kind: 'warning' },
    )
    if (!proceed) return
    try {
      const s = await backupRestore(path, passphrase)
      const env = s.environment
      showToast({
        kind: 'success',
        title: 'Backup restored',
        detail:
          `${env.sessions_added} new sessions (${env.sessions_skipped} already existed), ` +
          `${env.folders_added} folders, ${env.groups_added} groups, ${env.snippets_added} snippets, ` +
          `${env.rules_added} rules · vault: ${s.vault_added} added, ${s.vault_updated} updated · ` +
          `${s.credentials_linked} credentials attached · ${s.settings_applied} settings · ` +
          `${s.known_hosts_added} known hosts`,
        ttl: 0,
      })
      warn('Restore', [...env.warnings, ...s.warnings])
      const yes = await ask(
        'Restart ZapTerm now so the restored sessions and settings are loaded?\n\nOpen sessions will be closed.',
        { title: 'Restart ZapTerm', kind: 'info' },
      )
      if (yes) await restartApp()
    } catch (e) {
      showToast({ kind: 'error', title: 'Restore failed', detail: errorText(e) })
    }
  }
</script>

<div class="panel">
  <section>
    <h3>Encrypted backup</h3>
    <p class="hint">
      One file with <strong>everything</strong>: sessions, folders, groups, snippets and macros,
      highlight rules, settings, trusted host keys — and the passwords and vault entries that the
      plain sessions export leaves out. It is sealed with a passphrase you choose (Argon2id +
      AES-256-GCM), so it is safe to keep in any cloud-synced folder: the provider only ever sees
      ciphertext.
    </p>
    <div class="actions">
      <button type="button" class="ok-btn" disabled={busy} onclick={chooseExportFile}>
        Create backup…
      </button>
      <button type="button" class="ghost-btn" disabled={busy} onclick={chooseRestoreFile}>
        Restore from backup…
      </button>
    </div>
    <p class="hint">
      Restoring merges: sessions that already exist here are left untouched (including their
      passwords), vault entries this device already has keep their password, settings are replaced,
      host keys are added. Nothing is deleted.
    </p>
  </section>

  <section>
    <h3>Sync folder</h3>
    <p class="hint">
      Keep the same backup automatically up to date in a folder your cloud client already syncs
      (Dropbox, Drive, iCloud, OneDrive, Syncthing, a NAS…). ZapTerm publishes your changes there
      every few minutes and tells you when another device published theirs, so every install ends up
      with the same sessions, credentials and settings. Use the same passphrase on every device.
    </p>

    {#if !sync}
      <p class="hint">Loading…</p>
    {:else if sync.state === 'disabled'}
      <div class="actions">
        <button type="button" class="ok-btn" disabled={syncBusy} onclick={chooseSyncFolder}>
          Choose sync folder…
        </button>
      </div>
    {:else}
      <code class="path">{sync.dir}</code>
      <p class="hint">
        Status: <strong>{SYNC_STATE_LABELS[sync.state]}</strong>
        {#if sync.detail}
          · {sync.detail}{/if}
      </p>
      <p class="hint">
        Last published: {fmt(sync.last_push)} · last applied: {fmt(sync.last_pull)}
        {#if sync.remote}
          · in folder: from <strong>{sync.remote.device_name}</strong>, {fmt(
            sync.remote.written_at,
          )}
        {/if}
      </p>
      {#if !sync.has_passphrase}
        <p class="notice">
          The passphrase for this folder is not stored on this device. Set it to start syncing.
        </p>
      {/if}
      <div class="actions">
        <button
          type="button"
          class="ok-btn"
          disabled={syncBusy || !sync.has_passphrase}
          onclick={syncNow}
        >
          Sync now
        </button>
        <button type="button" class="ghost-btn" disabled={syncBusy} onclick={changeSyncPassphrase}>
          {sync.has_passphrase ? 'Change passphrase…' : 'Set passphrase…'}
        </button>
        <button type="button" class="ghost-btn" disabled={syncBusy} onclick={chooseSyncFolder}>
          Change folder…
        </button>
        <button type="button" class="ghost-btn" disabled={syncBusy} onclick={disableSync}>
          Disable
        </button>
      </div>
    {/if}
    <p class="hint">
      Sync adds and updates; it never deletes. A session removed or renamed on one device comes back
      from the other on the next merge. Applying another device's changes is always confirmed first
      and needs a restart.
    </p>
  </section>

  <section>
    <h3>Moving to another device</h3>
    <p class="hint">
      Either restore a backup file on the new device, or point it at the same sync folder with the
      same passphrase. The passphrase is never written to the folder — if you forget it, the backup
      cannot be opened.
    </p>
  </section>
</div>

{#if prompt}
  <PassphraseDialog
    title={prompt.mode === 'export'
      ? 'Protect the backup'
      : prompt.mode === 'sync'
        ? 'Sync passphrase'
        : 'Open the backup'}
    message={prompt.mode === 'export'
      ? `Choose a passphrase (at least ${BACKUP_MIN_PASSPHRASE} characters). You will need it to restore the backup — it cannot be recovered.`
      : prompt.mode === 'sync'
        ? `The passphrase that seals the bundle in the sync folder (at least ${BACKUP_MIN_PASSPHRASE} characters). Use the same one on every device. It is kept in this device's keyring.`
        : 'Enter the passphrase the backup was created with.'}
    confirm={prompt.mode !== 'restore'}
    minLength={prompt.mode === 'restore' ? 0 : BACKUP_MIN_PASSPHRASE}
    submitLabel={prompt.mode === 'export'
      ? 'Create backup'
      : prompt.mode === 'sync'
        ? 'Start syncing'
        : 'Open'}
    onSubmit={onPassphrase}
    onCancel={() => (prompt = null)}
  />
{/if}

<style>
  .panel {
    padding: 0.75rem 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0;
    line-height: 1.5;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .path {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    padding: 0.3rem 0.5rem;
    border-radius: 3px;
    word-break: break-all;
  }
  .notice {
    font-size: 0.72rem;
    line-height: 1.5;
    margin: 0;
    padding: 0.45rem 0.6rem;
    border: 1px solid color-mix(in srgb, var(--zx-warn) 45%, transparent);
    background: color-mix(in srgb, var(--zx-warn) 10%, transparent);
    border-radius: var(--zx-radius);
    color: var(--zx-text);
  }
  .ok-btn {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
    border: 0;
    border-radius: 4px;
    font-size: 0.78rem;
    font-family: inherit;
    font-weight: 600;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }
  .ok-btn:hover {
    filter: brightness(1.1);
  }
  .ok-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .ghost-btn {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
    border-radius: 4px;
    font-size: 0.78rem;
    font-family: inherit;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }
  .ghost-btn:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }
  .ghost-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
