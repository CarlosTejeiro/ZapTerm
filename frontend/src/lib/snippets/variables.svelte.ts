// Snippet variables: `{{name}}` placeholders that prompt for a value when the
// snippet runs. Optional default with `{{name=Default}}` or `{{name|Default}}`.
//
// `resolveSnippet(content)` returns the substituted text, opening a modal to
// collect values when the snippet has any variables. A single shared request
// drives `VariablesDialog.svelte` (mounted once in App.svelte). Returns null
// if the user cancels.

export interface SnippetVar {
  name: string
  def: string
}

const VAR_RE = /\{\{\s*([^}|=]+?)\s*(?:[|=]\s*([^}]*?))?\s*\}\}/g

/** Unique variables in `content`, in first-seen order (first default wins). */
export function parseVariables(content: string): SnippetVar[] {
  const out: SnippetVar[] = []
  const seen = new Set<string>()
  for (const m of content.matchAll(VAR_RE)) {
    const name = (m[1] ?? '').trim()
    if (!name || seen.has(name)) continue
    seen.add(name)
    out.push({ name, def: (m[2] ?? '').trim() })
  }
  return out
}

/** Replace every `{{name...}}` with `values[name]` (missing → empty). */
export function substitute(content: string, values: Record<string, string>): string {
  return content.replace(VAR_RE, (_full, rawName) => {
    const name = String(rawName).trim()
    return values[name] ?? ''
  })
}

interface PendingRequest {
  vars: SnippetVar[]
  values: Record<string, string>
  content: string
  resolve: (text: string | null) => void
}

// The active request the dialog renders; null when idle.
export const variablesRequest = $state<{ current: PendingRequest | null }>({ current: null })

/**
 * Resolve a snippet's text, prompting for `{{var}}` values when present.
 * No variables → returns the content unchanged (no dialog).
 */
export function resolveSnippet(content: string): Promise<string | null> {
  const vars = parseVariables(content)
  if (vars.length === 0) return Promise.resolve(content)
  return new Promise((resolve) => {
    const values: Record<string, string> = {}
    for (const v of vars) values[v.name] = v.def
    variablesRequest.current = { vars, values, content, resolve }
  })
}

/** Called by the dialog: substitute and resolve, or cancel (null). */
export function commitVariables(cancelled: boolean): void {
  const req = variablesRequest.current
  if (!req) return
  variablesRequest.current = null
  req.resolve(cancelled ? null : substitute(req.content, req.values))
}
