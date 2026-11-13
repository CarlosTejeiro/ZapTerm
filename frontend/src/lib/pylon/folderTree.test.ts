import { describe, expect, it } from 'vitest'
import type { Folder } from '$lib/bridge/types'
import { rootFolders } from './folderTree'

const f = (id: number, parent_id: number | null): Folder =>
  ({ id, parent_id, name: `f${id}`, sort_order: 0 }) as Folder

describe('rootFolders', () => {
  it('keeps only top-level folders', () => {
    expect(rootFolders([f(1, null), f(2, 1), f(3, 2), f(4, null)]).map((x) => x.id)).toEqual([1, 4])
  })
  it('shows a folder whose parent is gone at the root', () => {
    expect(rootFolders([f(1, null), f(2, 99)]).map((x) => x.id)).toEqual([1, 2])
  })
  it('does not lose folders caught in a parent loop', () => {
    const ids = rootFolders([f(1, null), f(2, 3), f(3, 2)]).map((x) => x.id)
    expect(ids).toEqual([1, 2, 3])
  })
})
