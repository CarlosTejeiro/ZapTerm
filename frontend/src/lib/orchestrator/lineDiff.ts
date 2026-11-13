// Line diff for the run-and-compare panel.
//
// A plain LCS needs an (n+1)×(m+1) table: two 20 000-line configs (a full
// FortiGate `show`) is 400 M cells — gigabytes, enough to freeze or kill the
// window. Configs usually differ in a few places, so the common prefix and
// suffix are taken off first and the LCS only runs on the part in between,
// in one flat typed array. If even that is too big, the middle is shown as
// removed-then-added instead of hanging.

export interface DiffLine {
  kind: 'same' | 'a' | 'b'
  text: string
}

/** Largest middle section (cells) the exact LCS is run on (~16 MB). */
export const MAX_LCS_CELLS = 4_000_000

export function lineDiff(a: string, b: string, maxCells = MAX_LCS_CELLS): DiffLine[] {
  const A = a.split('\n')
  const B = b.split('\n')
  let start = 0
  while (start < A.length && start < B.length && A[start] === B[start]) start++
  let endA = A.length
  let endB = B.length
  while (endA > start && endB > start && A[endA - 1] === B[endB - 1]) {
    endA--
    endB--
  }

  const out: DiffLine[] = []
  for (let i = 0; i < start; i++) out.push({ kind: 'same', text: A[i]! })
  out.push(...middleDiff(A.slice(start, endA), B.slice(start, endB), maxCells))
  for (let i = endA; i < A.length; i++) out.push({ kind: 'same', text: A[i]! })
  return out
}

function middleDiff(A: string[], B: string[], maxCells: number): DiffLine[] {
  const n = A.length
  const m = B.length
  const out: DiffLine[] = []
  if ((n + 1) * (m + 1) > maxCells) {
    for (const t of A) out.push({ kind: 'a', text: t })
    for (const t of B) out.push({ kind: 'b', text: t })
    return out
  }
  const w = m + 1
  const lcs = new Uint32Array((n + 1) * w)
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i * w + j] =
        A[i] === B[j]
          ? lcs[(i + 1) * w + j + 1]! + 1
          : Math.max(lcs[(i + 1) * w + j]!, lcs[i * w + j + 1]!)
    }
  }
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      out.push({ kind: 'same', text: A[i]! })
      i++
      j++
    } else if (lcs[(i + 1) * w + j]! >= lcs[i * w + j + 1]!) {
      out.push({ kind: 'a', text: A[i]! })
      i++
    } else {
      out.push({ kind: 'b', text: B[j]! })
      j++
    }
  }
  while (i < n) out.push({ kind: 'a', text: A[i++]! })
  while (j < m) out.push({ kind: 'b', text: B[j++]! })
  return out
}
