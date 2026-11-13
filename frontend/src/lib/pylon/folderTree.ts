import type { Folder } from '$lib/bridge/types'

/**
 * Top-level folders of the sidebar tree. A folder is a root when it has no
 * parent or its parent no longer exists; folders caught in a parent loop
 * (never reachable from a root) are shown at the root too, so none is lost.
 */
export function rootFolders(folders: Folder[]): Folder[] {
  const ids = new Set(folders.map((f) => f.id))
  const roots = folders.filter((f) => f.parent_id == null || !ids.has(f.parent_id))
  const reached = new Set<number>()
  const visit = (id: number) => {
    if (reached.has(id)) return
    reached.add(id)
    for (const c of folders) if (c.parent_id === id) visit(c.id)
  }
  for (const r of roots) visit(r.id)
  return [...roots, ...folders.filter((f) => !reached.has(f.id))]
}
