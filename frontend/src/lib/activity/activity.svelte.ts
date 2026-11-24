// Automation activity: everything a script, a notebook or an AI assistant
// (MCP) sent — or tried to send — through ZapTerm. The backend appends each
// event to a text file in the logs folder; this store keeps what the Activity
// view shows and adds new events to it as they happen.

import { invoke } from '@tauri-apps/api/core'
import type { ActivityEntry, NewActivity } from './activityView'

export const activity = $state({
  /** Oldest first. Empty until the view loads it. */
  entries: [] as ActivityEntry[],
  loaded: false,
  loading: false,
  error: '',
  /** Folder with the monthly files, and this month's file. */
  dir: '',
  currentFile: '',
  /** Monthly files not read yet (older than the months loaded). */
  olderFiles: 0,
  /** Months of files loaded. */
  months: 2,
  visible: false,
  /** Events recorded since ZapTerm started (the newest RECENT_MAX), each
   *  with its number: what the connection panel shows live. */
  recent: [] as { seq: number; entry: ActivityEntry }[],
  /** Number of the last event recorded. */
  seq: 0,
})

const RECENT_MAX = 400

interface ActivityLog {
  entries: ActivityEntry[]
  dir: string
  current_file: string
  older_files: number
}

/** Writes go one after another, so events reach the file in the order they
 *  happened. */
let queue: Promise<void> = Promise.resolve()

/** Append one event to the activity file. Never throws: a failure to record
 *  is logged and must not break the command being recorded. */
export function recordActivity(entry: NewActivity): Promise<void> {
  queue = queue.then(async () => {
    try {
      const stored = await invoke<ActivityEntry>('activity_record', { entry })
      if (activity.loaded) activity.entries.push(stored)
      activity.recent.push({ seq: ++activity.seq, entry: stored })
      if (activity.recent.length > RECENT_MAX) {
        activity.recent.splice(0, activity.recent.length - RECENT_MAX)
      }
    } catch (e) {
      console.error('activity_record failed', e)
    }
  })
  return queue
}

/** (Re)read the newest `months` monthly files. */
export async function loadActivity(months = activity.months): Promise<void> {
  activity.loading = true
  activity.error = ''
  try {
    const log = await invoke<ActivityLog>('activity_read', { months })
    activity.entries = log.entries
    activity.dir = log.dir
    activity.currentFile = log.current_file
    activity.olderFiles = log.older_files
    activity.months = months
    activity.loaded = true
  } catch (e) {
    activity.error = String(e instanceof Error ? e.message : e)
  } finally {
    activity.loading = false
  }
}

/** Open this month's file (or the folder) with the system's own application. */
export function openActivity(file: boolean): Promise<string> {
  return invoke<string>('activity_open', { file })
}

export function showActivity(): void {
  activity.visible = true
}
