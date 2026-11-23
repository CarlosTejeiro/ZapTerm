// Run & compare options shared by every SEND TO ALL bar (tabs and groups).

export const compareSettings = $state({
  /** Press Space for the user when a device stops at a pager prompt
   *  (`--More--`), so the whole output is compared. On by default. */
  autoPage: true,
  /** How Run & compare shows the differences: side by side (`split`, the
   *  default) or in one column (`unified`). */
  diffLayout: 'split' as 'split' | 'unified',
})
