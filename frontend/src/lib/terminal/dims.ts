// Terminal dimension sanitization.
//
// On some WebKitGTK builds (seen on Fedora) xterm's cell measurement can be
// invalid right after `term.open()` if fonts/layout aren't ready yet, leaving
// `term.cols`/`term.rows` at 1 (or 0/NaN). A PTY opened at 1×1 makes the remote
// shell's line editor redraw on a single column, producing garbage on input.
// These helpers guarantee we never hand a bad size to the backend.

export const FALLBACK_COLS = 80
export const FALLBACK_ROWS = 24

/** A terminal dimension is usable only when it's a finite number ≥ 2. */
export function isSaneDim(n: number): boolean {
  return Number.isFinite(n) && n >= 2
}

/** Coerce a column count to a usable value, falling back to 80. */
export function saneCols(n: number): number {
  return isSaneDim(n) ? Math.floor(n) : FALLBACK_COLS
}

/** Coerce a row count to a usable value, falling back to 24. */
export function saneRows(n: number): number {
  return isSaneDim(n) ? Math.floor(n) : FALLBACK_ROWS
}

/**
 * Sanitize a (cols, rows) pair. If BOTH are finite and ≥ 2 the measured values
 * pass through (floored); otherwise the whole pair falls back to 80×24 so a
 * failed measurement never opens the PTY at a degenerate size.
 */
export function sanitizeDims(cols: number, rows: number): { cols: number; rows: number } {
  if (isSaneDim(cols) && isSaneDim(rows)) {
    return { cols: Math.floor(cols), rows: Math.floor(rows) }
  }
  return { cols: FALLBACK_COLS, rows: FALLBACK_ROWS }
}
