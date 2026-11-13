// Broadcast-group state (multi-host orchestration).
//
// A broadcast group is a named, ordered set of SAVED sessions the user opens
// together as a grid. This store holds the persisted groups (loaded from
// SQLite via the bridge) and thin CRUD wrappers that keep the in-memory list
// in sync after each mutation. Distinct from the ephemeral live-target set in
// `sessionRuntime.svelte.ts` (`broadcastGroup.activeSessionIds`), which tracks
// the UUIDs of whatever grid is currently open.

import {
  listBroadcastGroups,
  createBroadcastGroup,
  renameBroadcastGroup,
  deleteBroadcastGroup,
  setBroadcastGroupMembers,
} from '$lib/bridge/commands'
import type { BroadcastGroup } from '$lib/bridge/types'

export const groups = $state<BroadcastGroup[]>([])

/** Reload the full group list from disk. */
export async function loadGroups(): Promise<void> {
  try {
    const fresh = await listBroadcastGroups()
    groups.splice(0, groups.length, ...fresh)
  } catch (e) {
    console.error('loadGroups failed:', e)
  }
}

/** Create a group and reload. Returns the new group id. */
export async function addGroup(name: string, sessionIds: number[] = []): Promise<number> {
  const id = await createBroadcastGroup(name)
  if (sessionIds.length) await setBroadcastGroupMembers(id, sessionIds)
  await loadGroups()
  return id
}

export async function renameGroup(id: number, name: string): Promise<void> {
  await renameBroadcastGroup(id, name)
  await loadGroups()
}

export async function removeGroup(id: number): Promise<void> {
  await deleteBroadcastGroup(id)
  await loadGroups()
}

export async function setGroupMembers(id: number, sessionIds: number[]): Promise<void> {
  await setBroadcastGroupMembers(id, sessionIds)
  await loadGroups()
}
