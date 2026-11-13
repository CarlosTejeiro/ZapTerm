// Shared UI flow for the sync folder: turns a `SyncStatus` into the right
// prompt (merge remote? resolve conflict?) and runs the chosen action. Used by
// the Backup settings panel (after "Sync now" / configure) and by the global
// listener in App.svelte that reacts to the background checker's events.

import { ask } from '@tauri-apps/plugin-dialog'
import {
  backupSyncPull,
  backupSyncPush,
  restartApp,
  type RestoreSummary,
  type SyncStatus,
} from '$lib/bridge/commands'
import { showToast } from '$lib/ui/toast-store.svelte'
import { errorText } from '$lib/bridge/errors'

export const SYNC_STATE_LABELS: Record<SyncStatus['state'], string> = {
  disabled: 'Off',
  no_passphrase: 'Passphrase missing',
  in_sync: 'In sync',
  local_changes: 'Local changes pending',
  remote_changes: 'Changes available from another device',
  conflict: 'Both sides changed',
  error: 'Error',
}

function when(ts: string | null | undefined): string {
  if (!ts) return 'unknown date'
  const d = new Date(ts)
  return isNaN(d.getTime()) ? ts : d.toLocaleString()
}

function remoteLine(st: SyncStatus): string {
  const r = st.remote
  return r ? `"${r.device_name}" wrote the sync bundle on ${when(r.written_at)}.` : ''
}

function reportRestore(s: RestoreSummary) {
  const env = s.environment
  showToast({
    kind: 'success',
    title: 'Sync applied',
    detail:
      `${env.sessions_added} new sessions, ${env.items_updated} items updated, ` +
      `${env.folders_added} folders, ${env.groups_added} groups, ` +
      `${env.snippets_added} snippets, ${env.rules_added} rules · vault: ${s.vault_added} added, ` +
      `${s.vault_updated} updated · ${s.credentials_linked} credentials · ${s.settings_applied} settings · ` +
      `${s.known_hosts_added} known hosts`,
    ttl: 0,
  })
  const warnings = [...env.warnings, ...s.warnings]
  if (warnings.length > 0) {
    showToast({
      kind: 'warning',
      title: `Sync: ${warnings.length} warning(s)`,
      detail: warnings.slice(0, 3).join(' · ') + (warnings.length > 3 ? ' …' : ''),
      ttl: 0,
    })
  }
}

/// Merge the remote bundle in (then publish the merged result), report, and
/// offer a restart so the new sessions/settings are loaded.
export async function pullAndReport(): Promise<SyncStatus | null> {
  try {
    const r = await backupSyncPull()
    reportRestore(r.restore)
    const yes = await ask(
      'Restart ZapTerm now so the synced sessions and settings are loaded?\n\nOpen sessions will be closed.',
      { title: 'Restart ZapTerm', kind: 'info' },
    )
    if (yes) await restartApp()
    return r.status
  } catch (e) {
    showToast({ kind: 'error', title: 'Sync failed', detail: errorText(e) })
    return null
  }
}

/// Act on a status: prompt for remote changes / conflicts, surface errors.
/// Returns the status after any action (or the input when nothing was done).
export async function handleSyncStatus(st: SyncStatus): Promise<SyncStatus> {
  switch (st.state) {
    case 'remote_changes': {
      const yes = await ask(
        `${remoteLine(st)}\n\nApply it now? Sessions that exist here are kept, vault entries with ` +
          'the same name are updated, settings are replaced with the synced ones. Nothing is deleted.',
        { title: 'Sync: changes from another device', kind: 'info' },
      )
      if (!yes) return st
      return (await pullAndReport()) ?? st
    }
    case 'conflict': {
      const mergeFirst = await ask(
        `${remoteLine(st)} This device also has changes that were not published yet.\n\n` +
          "Merge the other device's changes first, then publish the result? " +
          '(Its settings win.)\n\nChoose "No" to publish this device\'s environment over it instead.',
        { title: 'Sync: both sides changed', kind: 'warning' },
      )
      if (mergeFirst) return (await pullAndReport()) ?? st
      const overwrite = await ask(
        "Overwrite the sync bundle with this device's environment? The other device will be " +
          'offered to merge it on its next check.',
        { title: 'Sync: publish mine', kind: 'warning' },
      )
      if (!overwrite) return st
      try {
        const after = await backupSyncPush()
        showToast({
          kind: 'success',
          title: 'Sync',
          detail: "Published this device's environment.",
        })
        return after
      } catch (e) {
        showToast({ kind: 'error', title: 'Sync failed', detail: errorText(e) })
        return st
      }
    }
    case 'error':
      showToast({ kind: 'error', title: 'Sync', detail: st.detail ?? 'unknown error', ttl: 0 })
      return st
    default:
      return st
  }
}
