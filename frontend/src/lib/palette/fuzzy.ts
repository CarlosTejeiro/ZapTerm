// Small subsequence fuzzy matcher inspired by VS Code's algorithm but
// trimmed to the essentials: subsequence match, bonuses for word starts,
// uppercase boundaries, and consecutive runs. Returns a score (higher is
// better) and the matched character indices for highlighting; or null if
// no match.

export interface FuzzyResult {
  score: number
  indices: number[]
}

export function fuzzyMatch(query: string, target: string): FuzzyResult | null {
  if (!query) return { score: 0, indices: [] }
  const q = query.toLowerCase()
  const t = target.toLowerCase()
  if (q.length > t.length) return null

  const indices: number[] = []
  let ti = 0
  let score = 0
  let run = 0

  for (let qi = 0; qi < q.length; qi++) {
    const qc = q[qi]
    let found = -1
    while (ti < t.length) {
      if (t[ti] === qc) {
        found = ti
        break
      }
      ti++
      run = 0
    }
    if (found < 0) return null
    indices.push(found)

    // Position-based bonuses.
    let charScore = 1
    if (found === 0) charScore += 4 // first char
    const prev = target[found - 1]
    if (prev !== undefined && /[\s/_.-]/.test(prev)) charScore += 3 // word boundary
    if (target[found] !== qc && target[found] === qc?.toUpperCase()) charScore += 2 // CamelCase
    if (run > 0) charScore += run // consecutive run

    score += charScore
    run += 1
    ti += 1
  }

  // Penalty for unmatched tail length.
  score -= Math.max(0, target.length - q.length) * 0.05
  return { score, indices }
}
