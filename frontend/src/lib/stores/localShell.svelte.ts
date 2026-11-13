import { getSetting, setSetting } from '$lib/bridge/commands'
export { isWindowsHost, platformForShell } from './localShellPlatform'

/** Settings key of the local shell (read by the backend when a local tab
 *  opens): '' = automatic, 'pwsh', 'powershell', 'cmd', or a shell path. */
export const LOCAL_SHELL_KEY = 'terminal.localShell'

export const localShellSetting = $state({ value: '' })

/** The shell each local tab actually started, by runtime session id (the
 *  preferred one may have fallen back to another). */
export const localShells = $state<Record<string, string>>({})

export function noteLocalShell(sessionId: string, shell: string): void {
  localShells[sessionId] = shell
}

/** Drop a closed tab's entry. */
export function forgetLocalShell(sessionId: string): void {
  delete localShells[sessionId]
}

export async function loadLocalShellSetting(): Promise<void> {
  try {
    localShellSetting.value = (await getSetting(LOCAL_SHELL_KEY)) ?? ''
  } catch {
    localShellSetting.value = ''
  }
}

export async function saveLocalShellSetting(value: string): Promise<void> {
  const v = value.trim()
  await setSetting(LOCAL_SHELL_KEY, v)
  // Only once stored, so a failed save doesn't show a choice that isn't kept.
  localShellSetting.value = v
}
