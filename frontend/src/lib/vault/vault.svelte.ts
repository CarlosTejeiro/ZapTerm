// The vault's lock state, shared by the gate (set-up / unlock screen) and the
// Vault settings panel.

import { masterStatus, type VaultStatus } from '$lib/bridge/commands'

export const vault = $state<{ status: VaultStatus | null }>({ status: null })

export function setVaultStatus(status: VaultStatus) {
  vault.status = status
}

/** Ask the backend; at start this also opens a vault remembered on this device. */
export async function refreshVault(): Promise<void> {
  vault.status = await masterStatus()
}
