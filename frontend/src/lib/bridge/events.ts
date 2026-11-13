// Typed event subscribers — populated in Bloque 1
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

export interface TerminalDataPayload {
  session_id: string
  data: number[]
}

export async function onTerminalData(
  handler: (payload: TerminalDataPayload) => void,
): Promise<UnlistenFn> {
  return listen<TerminalDataPayload>('terminal-data', (event) => {
    handler(event.payload)
  })
}
