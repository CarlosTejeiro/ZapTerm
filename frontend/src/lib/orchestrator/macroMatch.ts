// Pure expect-pattern matching for the macro runner. Kept in its own module
// (free of Svelte runes / store imports) so it can be unit-tested directly.

import type { LoginStep } from '$lib/bridge/types'

/**
 * Index in `haystack` just PAST the first match of `step.expect`, or -1 if it
 * doesn't match. Returning the end offset lets the runner consume matched text
 * from the streaming buffer so a later `expect` can't re-match earlier output.
 * An empty pattern never matches.
 */
export function matchEnd(step: LoginStep, haystack: string): number {
  if (!step.expect) return -1
  if (step.is_regex) {
    try {
      const m = new RegExp(step.expect).exec(haystack)
      return m ? m.index + m[0].length : -1
    } catch {
      const idx = haystack.indexOf(step.expect) // invalid regex → literal
      return idx < 0 ? -1 : idx + step.expect.length
    }
  }
  const idx = haystack.indexOf(step.expect)
  return idx < 0 ? -1 : idx + step.expect.length
}
