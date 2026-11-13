/**
 * Unified SVG icon set — 16×16 viewBox, stroke 1.5, round caps/joins.
 * Paths come from the design handoff (`zapterm-shared.jsx`); rendered by
 * `Icon.svelte`. Each entry is the inner markup of the <svg> element so a
 * glyph can mix paths, circles and rects.
 */
export const ICON_PATHS = {
  search: '<circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5 L14 14" />',
  plus: '<path d="M8 3v10M3 8h10" />',
  folder: '<path d="M2 4.5h4l1.5 2H14v6.5H2z" />',
  chevron: '<path d="M6 3.5 L10.5 8 L6 12.5" />',
  pencil: '<path d="M3 13l.8-3L11 2.8a1.2 1.2 0 0 1 1.7 0l.5.5a1.2 1.2 0 0 1 0 1.7L6 12.2z" />',
  x: '<path d="M4 4l8 8M12 4l-8 8" />',
  split:
    '<rect x="2" y="2.5" width="5" height="11" rx="1.2" /><rect x="9" y="2.5" width="5" height="11" rx="1.2" />',
  splitV:
    '<rect x="2.5" y="2" width="11" height="5" rx="1.2" /><rect x="2.5" y="9" width="11" height="5" rx="1.2" />',
  cast: '<circle cx="8" cy="8" r="1.4" /><path d="M4.8 11.2a4.5 4.5 0 0 1 0-6.4M11.2 4.8a4.5 4.5 0 0 1 0 6.4" /><path d="M2.8 13.2a7.2 7.2 0 0 1 0-10.4M13.2 2.8a7.2 7.2 0 0 1 0 10.4" />',
  gear: '<circle cx="8" cy="8" r="2.2" /><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M12.4 3.6 11 5M5 11l-1.4 1.4" />',
  term: '<path d="M3 5l3.5 3L3 11" /><path d="M8.5 12H13" />',
  book: '<path d="M3 2.5h7.5a2 2 0 0 1 2 2V13.5H5a2 2 0 0 0-2 2z" /><path d="M3 13.5a2 2 0 0 1 2-2h7.5" />',
  clock: '<circle cx="8" cy="8" r="5.8" /><path d="M8 4.8V8l2.2 1.6" />',
  bolt: '<path d="M8.8 1.5 3.5 9h3.4l-.7 5.5L11.5 7H8.1z" />',
  key: '<circle cx="5.5" cy="10.5" r="3" /><path d="M7.8 8.2 13.5 2.5M11 5l2 2M9.5 6.5l1.5 1.5" />',
  min: '<path d="M3.5 8h9" />',
  max: '<rect x="3.5" y="3.5" width="9" height="9" rx="1" />',
  file: '<path d="M4 1.5h5.5L12 4v10.5H4z" /><path d="M9.5 1.5V4H12" />',
  /** Duplicate — a front document with a second peeking out behind it. */
  copy: '<rect x="5.5" y="5.5" width="8" height="8" rx="1.5" /><path d="M10.5 5.5V3.6A1.1 1.1 0 0 0 9.4 2.5H3.6A1.1 1.1 0 0 0 2.5 3.6v5.8a1.1 1.1 0 0 0 1.1 1.1h1.9" />',
  /** Import / load-from-file — a tray with an arrow pointing down into it. */
  download: '<path d="M8 2.5v7" /><path d="M5 6.5 8 9.5 11 6.5" /><path d="M3 11.5v1.5h10v-1.5" />',
  /** Export / save-to-file — a tray with an arrow pointing up out of it. */
  upload: '<path d="M8 9.5v-7" /><path d="M5 5.5 8 2.5 11 5.5" /><path d="M3 11.5v1.5h10v-1.5" />',
  /** SFTP / file transfer — up+down arrows between two endpoints. */
  transfer:
    '<path d="M5 2.5v9M5 11.5 2.8 9.3M5 11.5 7.2 9.3" /><path d="M11 13.5v-9M11 4.5 8.8 6.7M11 4.5 13.2 6.7" />',
  /** Tunnel / port forward — two brackets linked by a routed line. */
  tunnel:
    '<path d="M4.5 3.5h-2v9h2" /><path d="M11.5 3.5h2v9h-2" /><path d="M5.5 8h5" /><path d="M8.5 6 10.5 8 8.5 10" />',
  link: '<path d="M6.5 9.5 9.5 6.5" /><path d="M7.5 4.5 9 3a2.5 2.5 0 0 1 3.5 3.5L11 8" /><path d="M8.5 11.5 7 13a2.5 2.5 0 0 1-3.5-3.5L5 8" />',
  /** Filled record dot — `fill-current`, no stroke (session logging toggle). */
  record: '<circle cx="8" cy="8" r="4" fill="currentColor" stroke="none" />',
  /** Half-filled circle — theme cycle toggle. */
  contrast:
    '<circle cx="8" cy="8" r="5.8" /><path d="M8 2.2v11.6A5.8 5.8 0 0 0 8 2.2z" fill="currentColor" stroke="none" />',
  /** Warning — triangle with an exclamation mark. */
  alert:
    '<path d="M8 2.2 14.2 13H1.8z" /><path d="M8 6.5v3" /><circle cx="8" cy="11.2" r="0.4" fill="currentColor" />',
  /** Info — circle with an "i". */
  info: '<circle cx="8" cy="8" r="5.8" /><path d="M8 7.3v3.7" /><circle cx="8" cy="5.1" r="0.4" fill="currentColor" />',
  /** Success / selected — a single check stroke. */
  check: '<path d="M3.2 8.4 6.5 11.5 12.8 4.8" />',
  /** Refresh / reload — an open circular arrow. */
  refresh: '<path d="M13 8a5 5 0 1 1-1.5-3.6" /><path d="M13 2.5v3.2H9.8" />',
} as const

export type IconName = keyof typeof ICON_PATHS
