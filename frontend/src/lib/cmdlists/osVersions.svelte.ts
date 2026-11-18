import { getSessionOsVersion } from '$lib/bridge/commands'

/** The OS version each saved session's device last reported ("7.4.3"), by
 *  saved-session id. The bar uses it to pick the lists for that release. */
export const osVersions = $state<{ bySaved: Record<number, string> }>({ bySaved: {} })

export function setOsVersion(savedSessionId: number, version: string): void {
  osVersions.bySaved[savedSessionId] = version
}

/** Read the stored version of a saved session (none stored: forget ours). */
export async function loadOsVersion(savedSessionId: number): Promise<void> {
  try {
    const v = await getSessionOsVersion(savedSessionId)
    if (v) osVersions.bySaved[savedSessionId] = v
    else delete osVersions.bySaved[savedSessionId]
  } catch {
    /* non-fatal: the platform's default lists are used */
  }
}
