// Run & compare result per pane, shown as a small mark in the pane header so
// the result is visible next to each terminal, not only in the panel.
// Keyed by live session id; the comparison panel writes its panes' entries
// and removes them when it closes.

export interface CompareBadge {
  /** `running`: waiting for output; `same`: every pane gave this output;
   *  `variant`: one of several outputs (see `letter`); `noreply`. */
  kind: 'running' | 'same' | 'variant' | 'noreply'
  /** "A" (most common output; on a tie the left-most pane's), "B", "C"… */
  letter?: string
  /** The capture may be incomplete (stopped at a pager prompt, or no prompt
   *  before the time limit). */
  incomplete?: boolean
  /** Tooltip. */
  title: string
}

export const compareBadges = $state<{ bySession: Record<string, CompareBadge> }>({
  bySession: {},
})

export function setCompareBadges(entries: Record<string, CompareBadge>, previous: string[]): void {
  const next = { ...compareBadges.bySession }
  for (const id of previous) delete next[id]
  Object.assign(next, entries)
  compareBadges.bySession = next
}

export function clearCompareBadges(ids: string[]): void {
  if (!ids.some((id) => id in compareBadges.bySession)) return
  const next = { ...compareBadges.bySession }
  for (const id of ids) delete next[id]
  compareBadges.bySession = next
}
