import { getSetting, setSetting } from '$lib/bridge/commands'

// Reactive hint preferences — mirrored from the backend settings table.
export const hintsSettings = $state({
  ghostEnabled: true,
  popupEnabled: true,
  defaultPlatform: 'generic',
})

export async function loadHintsSettings(): Promise<void> {
  try {
    const [ghost, popup, platform] = await Promise.all([
      getSetting('hints.ghost_enabled'),
      getSetting('hints.popup_enabled'),
      getSetting('hints.default_platform'),
    ])
    if (ghost !== null) hintsSettings.ghostEnabled = ghost !== 'false'
    if (popup !== null) hintsSettings.popupEnabled = popup !== 'false'
    if (platform) hintsSettings.defaultPlatform = platform
  } catch {
    // keep defaults
  }
}

export async function setGhostEnabled(v: boolean): Promise<void> {
  hintsSettings.ghostEnabled = v
  await setSetting('hints.ghost_enabled', String(v))
}

export async function setPopupEnabled(v: boolean): Promise<void> {
  hintsSettings.popupEnabled = v
  await setSetting('hints.popup_enabled', String(v))
}

export async function setDefaultPlatform(platform: string): Promise<void> {
  hintsSettings.defaultPlatform = platform
  await setSetting('hints.default_platform', platform)
}
