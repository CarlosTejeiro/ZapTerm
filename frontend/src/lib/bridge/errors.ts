// Readable text for anything a `catch` can receive.
//
// Tauri command errors arrive as the serialized `AppError`, i.e. an object
// like `{ "Internal": "keyring error: …" }` — not an `Error` and not a
// string — so `String(e)` turned every backend error into "[object Object]".

export function errorText(e: unknown): string {
  if (e instanceof Error) return e.message
  if (typeof e === 'string') return unwrapInternal(e)
  if (e && typeof e === 'object') {
    const internal = (e as Record<string, unknown>).Internal
    if (typeof internal === 'string') return internal
    const message = (e as Record<string, unknown>).message
    if (typeof message === 'string') return message
    try {
      return JSON.stringify(e)
    } catch {
      /* cyclic — fall through */
    }
  }
  return String(e)
}

/** A JSON-encoded `{"Internal": "…"}` string → its message. */
function unwrapInternal(s: string): string {
  if (!s.startsWith('{')) return s
  try {
    const o = JSON.parse(s) as Record<string, unknown>
    return typeof o.Internal === 'string' ? o.Internal : s
  } catch {
    return s
  }
}
