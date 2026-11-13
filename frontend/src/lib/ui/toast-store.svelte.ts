export type ToastKind = 'info' | 'success' | 'warning' | 'error'

export interface Toast {
  id: number
  kind: ToastKind
  title: string
  detail?: string
  /** Auto-dismiss after this many ms. 0 = sticky. */
  ttl: number
}

const DEFAULT_TTL = 4000
let nextId = 1

export const toasts = $state<Toast[]>([])

export function showToast(t: Omit<Toast, 'id' | 'ttl'> & { ttl?: number }): number {
  const id = nextId++
  const ttl = t.ttl ?? DEFAULT_TTL
  toasts.push({ ...t, id, ttl })
  if (ttl > 0) {
    setTimeout(() => dismissToast(id), ttl)
  }
  return id
}

export function dismissToast(id: number): void {
  const i = toasts.findIndex((t) => t.id === id)
  if (i >= 0) toasts.splice(i, 1)
}
