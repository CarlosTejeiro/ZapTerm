// Runtime-only credential cache: session_id → password.
// Lives only while the app is running — never written to disk.
const cache = new Map<number, string>()

export function getCachedPassword(sessionId: number): string | null {
  return cache.get(sessionId) ?? null
}

export function setCachedPassword(sessionId: number, password: string): void {
  cache.set(sessionId, password)
}

export function clearCachedPassword(sessionId: number): void {
  cache.delete(sessionId)
}
