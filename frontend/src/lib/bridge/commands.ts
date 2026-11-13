import { invoke } from '@tauri-apps/api/core'
import type {
  SavedSession,
  Folder,
  BroadcastGroup,
  HighlightRule,
  SessionLog,
  ColorScheme,
  AuthMethod,
  HostKeyStatus,
  ForwardInfo,
  SavedForward,
  SftpEntry,
  Snippet,
  RecentCommand,
  LoginStep,
  Trigger,
  VaultEntry,
} from './types'

export async function listSessions(): Promise<SavedSession[]> {
  return invoke<SavedSession[]>('list_sessions')
}

export async function listFolders(): Promise<Folder[]> {
  return invoke<Folder[]>('list_folders')
}

export async function createFolder(name: string, parentId: number | null): Promise<number> {
  return invoke<number>('create_folder', { name, parentId })
}

export async function renameFolder(id: number, name: string): Promise<void> {
  return invoke<void>('rename_folder', { id, name })
}

export async function deleteFolder(id: number): Promise<void> {
  return invoke<void>('delete_folder', { id })
}

// ── Broadcast groups (multi-host orchestration) ──────────────────────────────

export async function listBroadcastGroups(): Promise<BroadcastGroup[]> {
  return invoke<BroadcastGroup[]>('list_broadcast_groups')
}

export async function createBroadcastGroup(name: string): Promise<number> {
  return invoke<number>('create_broadcast_group', { name })
}

export async function renameBroadcastGroup(id: number, name: string): Promise<void> {
  return invoke<void>('rename_broadcast_group', { id, name })
}

export async function deleteBroadcastGroup(id: number): Promise<void> {
  return invoke<void>('delete_broadcast_group', { id })
}

export async function setBroadcastGroupMembers(
  groupId: number,
  sessionIds: number[],
): Promise<void> {
  return invoke<void>('set_broadcast_group_members', { groupId, sessionIds })
}

/**
 * Whether `tail` ends at `platform`'s idle prompt — i.e. a command has
 * finished on that device. Used by the multi-host command runner to bound
 * captured output per host. Unknown platforms always return false (the runner
 * falls back to a time window).
 */
export async function promptReturned(platform: string, tail: string): Promise<boolean> {
  return invoke<boolean>('prompt_returned', { platform, tail })
}

/**
 * Reverse-i-search (Ctrl+R) over command history: substring match scoped to
 * this saved session (plus session-less entries), most-recent first. An empty
 * `query` returns the most recent commands.
 */
export async function searchCommandHistory(
  savedSessionId: number | null,
  query: string,
  limit = 50,
): Promise<string[]> {
  return invoke<string[]>('search_command_history', { savedSessionId, query, limit })
}

export async function createSavedSession(
  name: string,
  folderId: number | null,
  host: string,
  port: number,
  username: string,
  auth: AuthMethod,
  viaSessionId: number | null = null,
): Promise<number> {
  return invoke<number>('create_saved_session', {
    name,
    folderId,
    host,
    port,
    username,
    auth,
    viaSessionId,
  })
}

export async function openSshSession(
  host: string,
  port: number,
  user: string,
  auth: AuthMethod,
  cols: number,
  rows: number,
): Promise<string> {
  return invoke<string>('open_ssh_session', { host, port, user, auth, cols, rows })
}

/** Fetch+classify a host key against ~/.ssh/known_hosts (run before connecting). */
export async function sshPreflightHostKey(host: string, port: number): Promise<HostKeyStatus> {
  return invoke<HostKeyStatus>('ssh_preflight_host_key', { host, port })
}

/** Persist trust for a host key (writes ~/.ssh/known_hosts). `fingerprint` is
 * the value approved at preflight; the backend refuses to learn a key that no
 * longer matches it (TOCTOU guard). */
export async function sshTrustHostKey(
  host: string,
  port: number,
  fingerprint: string,
): Promise<void> {
  return invoke<void>('ssh_trust_host_key', { host, port, fingerprint })
}

/** Overwrite a CHANGED host key: forget the stale known_hosts entry and trust
 * the key the server currently presents (legitimate reinstall / factory reset).
 * `fingerprint` is the approved value; the backend refuses if the server now
 * presents a different key (TOCTOU guard). */
export async function sshOverwriteHostKey(
  host: string,
  port: number,
  fingerprint: string,
): Promise<void> {
  return invoke<void>('ssh_overwrite_host_key', { host, port, fingerprint })
}

/** Respond to a keyboard-interactive `InfoRequest` (one entry per prompt). */
export async function respondKeyboardInteractive(
  interactionId: string,
  responses: string[],
): Promise<void> {
  return invoke<void>('respond_keyboard_interactive', { interactionId, responses })
}

// ── port forwards ────────────────────────────────────────────────────────────

export async function addLocalForward(
  sessionId: string,
  bindAddr: string,
  bindPort: number,
  targetHost: string,
  targetPort: number,
  name?: string,
): Promise<ForwardInfo> {
  return invoke<ForwardInfo>('add_local_forward', {
    sessionId,
    bindAddr,
    bindPort,
    targetHost,
    targetPort,
    name: name || null,
  })
}

export async function addDynamicForward(
  sessionId: string,
  bindAddr: string,
  bindPort: number,
  name?: string,
): Promise<ForwardInfo> {
  return invoke<ForwardInfo>('add_dynamic_forward', {
    sessionId,
    bindAddr,
    bindPort,
    name: name || null,
  })
}

/// Ask the SSH server to listen on `bindAddr:bindPort` and forward every
/// inbound connection to `targetHost:targetPort` on this machine.
/// Equivalent to `ssh -R bindAddr:bindPort:targetHost:targetPort`.
export async function addRemoteForward(
  sessionId: string,
  bindAddr: string,
  bindPort: number,
  targetHost: string,
  targetPort: number,
  name?: string,
): Promise<ForwardInfo> {
  return invoke<ForwardInfo>('add_remote_forward', {
    sessionId,
    bindAddr,
    bindPort,
    targetHost,
    targetPort,
    name: name || null,
  })
}

export async function listForwards(sessionId: string): Promise<ForwardInfo[]> {
  return invoke<ForwardInfo[]>('list_forwards', { sessionId })
}

// Download a remote file, open it in the OS default editor, and re-upload on
// every save until the session closes. Returns the local temp path.
export async function sftpEditFile(sessionId: string, remotePath: string): Promise<string> {
  return invoke<string>('sftp_edit_file', { sessionId, remotePath })
}

// One active forward plus the live session it belongs to.
export interface SessionForwardInfo {
  session_id: string
  info: ForwardInfo
}

// Every active forward across all live sessions (global tunnels manager).
export async function listAllForwards(): Promise<SessionForwardInfo[]> {
  return invoke<SessionForwardInfo[]>('list_all_forwards')
}

export async function removeForward(sessionId: string, forwardId: string): Promise<void> {
  return invoke<void>('remove_forward', { sessionId, forwardId })
}

// ── SFTP ─────────────────────────────────────────────────────────────────────

export async function sftpListDir(sessionId: string, path: string): Promise<SftpEntry[]> {
  return invoke<SftpEntry[]>('sftp_list_dir', { sessionId, path })
}

export async function sftpStat(sessionId: string, path: string): Promise<SftpEntry> {
  return invoke<SftpEntry>('sftp_stat', { sessionId, path })
}

export async function sftpCanonicalize(sessionId: string, path: string): Promise<string> {
  return invoke<string>('sftp_canonicalize', { sessionId, path })
}

export async function sftpMkdir(sessionId: string, path: string): Promise<void> {
  return invoke<void>('sftp_mkdir', { sessionId, path })
}

export async function sftpRemoveDir(sessionId: string, path: string): Promise<void> {
  return invoke<void>('sftp_remove_dir', { sessionId, path })
}

export async function sftpRemoveFile(sessionId: string, path: string): Promise<void> {
  return invoke<void>('sftp_remove_file', { sessionId, path })
}

export async function sftpRename(sessionId: string, from: string, to: string): Promise<void> {
  return invoke<void>('sftp_rename', { sessionId, from, to })
}

/// Streaming download — emits `sftp-progress { transferId, done, total,
/// direction }` events while running. `transferId` is the same string the
/// caller passes here, so multiple concurrent transfers can be filtered.
export async function sftpDownloadFile(
  sessionId: string,
  remotePath: string,
  localPath: string,
  transferId: string,
): Promise<number> {
  return invoke<number>('sftp_download_file', { sessionId, remotePath, localPath, transferId })
}

export async function sftpUploadFile(
  sessionId: string,
  localPath: string,
  remotePath: string,
  transferId: string,
): Promise<number> {
  return invoke<number>('sftp_upload_file', { sessionId, localPath, remotePath, transferId })
}

/// Flip the cancel flag for a live transfer. Best-effort — the streaming
/// loop checks the flag after each chunk so the call returns immediately.
export async function sftpCancelTransfer(transferId: string): Promise<void> {
  return invoke<void>('sftp_cancel_transfer', { transferId })
}

/// TCP MSS snapshot for an SSH session (taken right after connect). Either
/// direction may come back as null on platforms / topologies where the OS
/// doesn't expose it.
export async function getSessionTcpMss(sessionId: string): Promise<import('./types').TcpMss> {
  return invoke<import('./types').TcpMss>('get_session_tcp_mss', { sessionId })
}

export interface SftpProgressEvent {
  transfer_id: string
  done: number
  total: number | null
  direction: 'download' | 'upload'
}

// ── snippets ─────────────────────────────────────────────────────────────────

export async function listSnippets(): Promise<Snippet[]> {
  return invoke<Snippet[]>('list_snippets')
}

/// Subset visible to a session of `platform` — globals (platform IS NULL)
/// plus matches. On first call for a never-seen platform the backend seeds a
/// curated default set so the bar isn't empty out of the box.
export async function listSnippetsForPlatform(platform: string): Promise<Snippet[]> {
  return invoke<Snippet[]>('list_snippets_for_platform', { platform })
}

export async function listRecentCommandSnippets(
  savedSessionId: number | null,
  limit?: number,
): Promise<RecentCommand[]> {
  return invoke<RecentCommand[]>('list_recent_command_snippets', { savedSessionId, limit })
}

export async function createSnippet(
  name: string,
  content: string,
  platform: string | null = null,
  color: string | null = null,
): Promise<number> {
  return invoke<number>('create_snippet', { name, content, platform, color })
}

export async function updateSnippet(
  id: number,
  name: string,
  content: string,
  platform: string | null = null,
  color: string | null = null,
): Promise<void> {
  return invoke<void>('update_snippet', { id, name, content, platform, color })
}

/** Set (JSON string) or clear (null) a snippet's macro steps. */
export async function setSnippetSteps(id: number, stepsJson: string | null): Promise<void> {
  return invoke<void>('set_snippet_steps', { id, stepsJson })
}

/** Set or clear (null) a snippet's folder for the sidebar Macros library. */
export async function setSnippetFolder(id: number, folder: string | null): Promise<void> {
  return invoke<void>('set_snippet_folder', { id, folder })
}

/** The Python script examples folder (created and filled on first use);
 *  `reveal` also opens it in the system file manager. */
export async function scriptExamplesDir(reveal: boolean): Promise<string> {
  return invoke<string>('script_examples_dir', { reveal })
}

/** Set or clear (null) the keyboard shortcut that fires a snippet or macro. */
export async function setSnippetShortcut(id: number, shortcut: string | null): Promise<void> {
  return invoke<void>('set_snippet_shortcut', { id, shortcut })
}

/** Persist a new per-folder ordering for macros (sidebar Macros ▲/▼ reorder).
 *  Each id's `position` becomes its index in `orderedIds`. */
export async function setSnippetPositions(orderedIds: number[]): Promise<void> {
  return invoke<void>('set_snippet_positions', { orderedIds })
}

// Persist a new display order (sort_order) for the listed snippet ids.
export async function setSnippetsOrder(orderedIds: number[]): Promise<void> {
  return invoke<void>('set_snippets_order', { orderedIds })
}

/** Summary returned by importMacros. */
export interface MacroImportSummary {
  added: number
  updated: number
}

/** Export macros to a JSON file. `ids = null` exports all macros. */
export async function exportMacros(path: string, ids: number[] | null = null): Promise<void> {
  return invoke<void>('export_macros', { path, ids })
}

/** Import macros from a JSON file (skip-by-name idempotent). */
export async function importMacros(path: string): Promise<MacroImportSummary> {
  return invoke<MacroImportSummary>('import_macros', { path })
}

// ── Credential vault ─────────────────────────────────────────────────────────

/** Create a reusable vault entry. The password goes to the OS keyring; the
 *  returned id is the credential reference. */
export async function createVaultEntry(
  name: string,
  username: string | null,
  password: string,
): Promise<number> {
  return invoke<number>('create_vault_entry', { name, username, password })
}

/** List all vault entries (no secrets). */
export async function listVaultEntries(): Promise<VaultEntry[]> {
  return invoke<VaultEntry[]>('list_vault_entries')
}

/** Update a vault entry's metadata, optionally rotating its secret. */
export async function updateVaultEntry(
  id: number,
  name: string,
  username: string | null,
  password: string | null = null,
): Promise<void> {
  return invoke<void>('update_vault_entry', { id, name, username, password })
}

/** Delete a vault entry (keyring secret + credential row). */
export async function deleteVaultEntry(id: number): Promise<void> {
  return invoke<void>('delete_vault_entry', { id })
}

/** Security-critical: send a vault entry's secret straight to a session's PTY.
 *  The plaintext never crosses back over the bridge. */
export async function sendVaultSecret(
  sessionId: string,
  vaultEntryId: number,
  enter: boolean,
): Promise<void> {
  return invoke<void>('send_vault_secret', { sessionId, vaultEntryId, enter })
}

/** Security-critical: send a vault entry's USERNAME or PASSWORD straight to a
 *  session's PTY, resolving the entry by its (unique) name. The plaintext /
 *  username never crosses back over the bridge. `field` is case-insensitive
 *  ("Username" | "Password"). Backs `{{vault:<Name>.<Field>}}`. */
export async function sendVaultField(
  sessionId: string,
  name: string,
  field: string,
  enter: boolean,
): Promise<void> {
  return invoke<void>('send_vault_field', { sessionId, name, field, enter })
}

export async function deleteSnippet(id: number): Promise<void> {
  return invoke<void>('delete_snippet', { id })
}

// ── login automation scripts ─────────────────────────────────────────────────

export async function getLoginScript(savedSessionId: number): Promise<LoginStep[]> {
  return invoke<LoginStep[]>('get_login_script', { savedSessionId })
}

export async function setLoginScript(savedSessionId: number, steps: LoginStep[]): Promise<void> {
  return invoke<void>('set_login_script', { savedSessionId, steps })
}

export async function getSessionTriggers(savedSessionId: number): Promise<Trigger[]> {
  return invoke<Trigger[]>('get_session_triggers', { savedSessionId })
}

export async function setSessionTriggers(
  savedSessionId: number,
  triggers: Trigger[],
): Promise<void> {
  return invoke<void>('set_session_triggers', { savedSessionId, triggers })
}

// ── saved port-forwards (auto-started on connect) ────────────────────────────

export async function getSessionForwards(savedSessionId: number): Promise<SavedForward[]> {
  return invoke<SavedForward[]>('get_session_forwards', { savedSessionId })
}

export async function setSessionForwards(
  savedSessionId: number,
  forwards: SavedForward[],
): Promise<void> {
  return invoke<void>('set_session_forwards', { savedSessionId, forwards })
}

/** Send raw text to a live session (uses the existing `send_input` command). */
export async function sendInputText(sessionId: string, text: string): Promise<void> {
  const data = Array.from(new TextEncoder().encode(text))
  return invoke<void>('send_input', { sessionId, data })
}

export async function deleteSavedSession(id: number): Promise<void> {
  return invoke<void>('delete_saved_session', { id })
}

/** Duplicate a saved session (independent copy). Returns the new session id. */
export async function cloneSavedSession(id: number): Promise<number> {
  return invoke<number>('clone_saved_session', { id })
}

/** Move a session to another folder (`null` = root). */
export async function moveSavedSession(id: number, folderId: number | null): Promise<void> {
  return invoke<void>('move_saved_session', { id, folderId })
}

/// Reorder a session inside its parent (folder or root). `targetIndex` is the
/// 0-based slot among the destination's children AFTER the move; the backend
/// renumbers every sibling so the next drag has predictable inputs.
export async function reorderSavedSession(
  id: number,
  folderId: number | null,
  targetIndex: number,
): Promise<void> {
  return invoke<void>('reorder_saved_session', { id, folderId, targetIndex })
}

/**
 * Edit a saved session in place. Credentials (auth method / password / key
 * passphrase) are intentionally NOT touched — recreate the session to change
 * those. `optionsJson` overwrites the protocol blob when provided.
 */
export async function updateSavedSession(
  id: number,
  name: string,
  folderId: number | null,
  host: string | null,
  port: number | null,
  username: string | null,
  viaSessionId: number | null,
  optionsJson: string | null = null,
): Promise<void> {
  return invoke<void>('update_saved_session', {
    id,
    name,
    folderId,
    host,
    port,
    username,
    viaSessionId,
    optionsJson,
  })
}

/**
 * Change an existing SSH session's credential in place: rotate its password,
 * switch auth method, or re-point it at a different vault entry. A typed
 * password/passphrase is written to a fresh private credential; a `vaultentry`
 * auth just references the shared row. The previous credential is purged only
 * when it was private — shared vault entries are left untouched. Call this only
 * when the credential actually changed; metadata edits go through
 * `updateSavedSession`.
 */
export async function setSessionCredential(id: number, auth: AuthMethod): Promise<void> {
  return invoke<void>('set_session_credential', { id, auth })
}

export async function openSavedSession(
  savedSessionId: number,
  cols: number,
  rows: number,
): Promise<string> {
  return invoke<string>('open_saved_session', { savedSessionId, cols, rows })
}

export async function createTelnetSession(
  name: string,
  folderId: number | null,
  host: string,
  port: number,
): Promise<number> {
  return invoke<number>('create_telnet_session', { name, folderId, host, port })
}

/** Save a WSL session: a local terminal running `wsl.exe --distribution
 *  <distro>` (and `--user <user>` when given). Windows only. */
export async function createWslSession(
  name: string,
  folderId: number | null,
  distro: string,
  user: string | null,
): Promise<number> {
  return invoke<number>('create_wsl_session', { name, folderId, distro, user })
}

/** Installed WSL distributions (`wsl --list --quiet`); empty off Windows. */
export async function listWslDistros(): Promise<string[]> {
  return invoke<string[]>('list_wsl_distros')
}

export async function createSerialSession(
  name: string,
  folderId: number | null,
  device: string,
  baudRate: number,
): Promise<number> {
  return invoke<number>('create_serial_session', { name, folderId, device, baudRate })
}

export async function listSerialPorts(): Promise<string[]> {
  return invoke<string[]>('list_serial_ports')
}

export async function getSettings(): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>('get_settings')
}

export async function listHighlightRules(): Promise<HighlightRule[]> {
  return invoke<HighlightRule[]>('list_highlight_rules')
}

export async function createHighlightRule(
  name: string,
  pattern: string,
  isRegex: boolean,
  fgColor: string | null,
  bgColor: string | null,
  bold: boolean,
  underline: boolean,
): Promise<number> {
  return invoke<number>('create_highlight_rule', {
    name,
    pattern,
    isRegex,
    fgColor,
    bgColor,
    bold,
    underline,
  })
}

export async function toggleHighlightRule(id: number, enabled: boolean): Promise<void> {
  return invoke<void>('toggle_highlight_rule', { id, enabled })
}

export async function deleteHighlightRule(id: number): Promise<void> {
  return invoke<void>('delete_highlight_rule', { id })
}

/** Change a rule's name and style. Its pattern and regex flag stay. */
export async function updateHighlightRule(
  id: number,
  name: string,
  fgColor: string | null,
  bgColor: string | null,
  bold: boolean,
  underline: boolean,
  enabled: boolean,
): Promise<void> {
  return invoke<void>('update_highlight_rule', {
    id,
    name,
    fgColor,
    bgColor,
    bold,
    underline,
    enabled,
  })
}

export interface RulesImportSummary {
  added: number
  /** Same pattern (and regex flag) as an existing rule. */
  skipped: number
  /** Rules left out, with the reason. */
  invalid: string[]
}

/** Write every highlight rule to `path` as JSON. Returns how many. */
export async function exportHighlightRules(path: string): Promise<number> {
  return invoke<number>('export_highlight_rules', { path })
}

/** Add the rules of an export file, skipping the ones already present. */
export async function importHighlightRules(path: string): Promise<RulesImportSummary> {
  return invoke<RulesImportSummary>('import_highlight_rules', { path })
}

export async function startSessionLogging(
  sessionId: string,
  savedSessionId: number | null,
  sessionName: string,
): Promise<string> {
  return invoke<string>('start_session_logging', { sessionId, savedSessionId, sessionName })
}

export async function stopSessionLogging(sessionId: string): Promise<void> {
  return invoke<void>('stop_session_logging', { sessionId })
}

export async function listSessionLogs(savedSessionId: number): Promise<SessionLog[]> {
  return invoke<SessionLog[]>('list_session_logs', { savedSessionId })
}

export async function listAllSessionLogs(): Promise<SessionLog[]> {
  return invoke<SessionLog[]>('list_all_session_logs')
}

export async function getSetting(key: string): Promise<string | null> {
  return invoke<string | null>('get_setting', { key })
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke<void>('set_setting', { key, value })
}

export async function listColorSchemes(): Promise<ColorScheme[]> {
  return invoke<ColorScheme[]>('list_color_schemes')
}

/** Persist and apply the SSH keepalive interval in seconds (0 disables). */
export async function setSshKeepalive(secs: number): Promise<void> {
  return invoke<void>('set_ssh_keepalive', { secs })
}

export async function openTelnetSession(
  host: string,
  port: number,
  cols: number,
  rows: number,
): Promise<string> {
  return invoke<string>('open_telnet_session', { host, port, cols, rows })
}

// ---------------------------------------------------------------------------
// Hints
// ---------------------------------------------------------------------------

import type { Hint, PlatformInfo } from './types'

export async function getHints(
  savedSessionId: number | null,
  prefix: string,
  limit: number = 5,
  // The last command run in this session — boosts its usual continuations.
  lastCommand: string | null = null,
): Promise<Hint[]> {
  return invoke<Hint[]>('get_hints', { savedSessionId, prefix, limit, lastCommand })
}

export async function recordCommand(
  savedSessionId: number | null,
  command: string,
  // The previously-run command — recorded as a prev→command transition.
  prevCommand: string | null = null,
): Promise<void> {
  return invoke<void>('record_command', { savedSessionId, command, prevCommand })
}

export async function setSessionPlatform(savedSessionId: number, platform: string): Promise<void> {
  return invoke<void>('set_session_platform', { savedSessionId, platform })
}

/** Forget the session's platform (detected or chosen): back to the default. */
export async function clearSessionPlatform(savedSessionId: number): Promise<void> {
  return invoke<void>('clear_session_platform', { savedSessionId })
}

export async function getSessionPlatform(savedSessionId: number): Promise<string | null> {
  return invoke<string | null>('get_session_platform', { savedSessionId })
}

export async function listPlatforms(): Promise<PlatformInfo[]> {
  return invoke<PlatformInfo[]>('list_platforms')
}

export async function clearCommandHistory(savedSessionId: number | null): Promise<void> {
  return invoke<void>('clear_command_history', { savedSessionId })
}

/// Re-scan the catalog override directory and return how many platforms
/// got their catalog replaced by a user file.
export async function reloadHintCatalogs(): Promise<number> {
  return invoke<number>('reload_hint_catalogs')
}

/// Absolute path of `<app_data>/catalogs/`. Creates the directory on first
/// call so the host file browser can open it cleanly. `null` when the host
/// did not configure a user-override dir (shouldn't happen in production).
export async function hintCatalogsDir(): Promise<string | null> {
  return invoke<string | null>('hint_catalogs_dir')
}

// Reveal the app's session-log directory in the host file explorer.
// Returns the absolute path (also useful for showing in a toast).
export async function openLogsDir(): Promise<string> {
  return invoke<string>('open_logs_dir')
}

// ── Export / import of the ZapTerm environment ────────────────────────────────

export interface ExportSummary {
  path: string
  sessions: number
  folders: number
  groups: number
  snippets: number
  rules: number
}

export interface ImportSummary {
  sessions_added: number
  sessions_skipped: number
  folders_added: number
  groups_added: number
  snippets_added: number
  rules_added: number
  /** Existing items updated from a sync pull (0 for imports and restores). */
  items_updated: number
  warnings: string[]
}

// Write the whole environment (sessions, folders, groups, snippets,
// highlight rules — never credentials) as versioned JSON at `path`.
export async function exportSessions(path: string): Promise<ExportSummary> {
  return invoke<ExportSummary>('export_sessions', { path })
}

// Merge a ZapTerm export file into the current DB. Idempotent: existing items
// are skipped and counted in the summary.
export async function importSessions(path: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_sessions', { path })
}

// Import SSH sessions from an OpenSSH client config. Omitting `path` uses
// `~/.ssh/config`. Same idempotent pipeline/summary as importSessions.
export async function importSshConfig(path?: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_ssh_config', { path: path ?? null })
}

// Import PuTTY sessions. With `path` (a .reg export) parses the file; without
// it, reads the Windows registry directly (errors on macOS/Linux).
export async function importPutty(path?: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_putty', { path: path ?? null })
}

// Import a MobaXterm.ini / .mxtsessions file.
export async function importMobaXterm(path: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_mobaxterm', { path })
}

// Import a SecureCRT "Sessions" directory (folder of per-session .ini files).
export async function importSecureCrt(path: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_securecrt', { path })
}

// ── Data directory (DB, logs, catalogs) ────────────────────────────────────

export interface DataDirInfo {
  path: string
  source: 'cli-flag' | 'env-var' | 'portable' | 'pointer' | 'default'
  portable: boolean
  default_path: string
  changeable: boolean
}

export async function getDataDirInfo(): Promise<DataDirInfo> {
  return invoke<DataDirInfo>('get_data_dir_info')
}

// ── Encrypted backup (.zapxb) ─────────────────────────────────────────────

/** File extension of a sealed bundle. */
export const BACKUP_EXTENSION = 'zapxb'
/** Minimum passphrase length the backend accepts (mirrors Rust). */
export const BACKUP_MIN_PASSPHRASE = 8

export interface BundleInfo {
  app_version: string
  created_at: string
  device_name: string
  sessions: number
  folders: number
  groups: number
  snippets: number
  rules: number
  vault_entries: number
  session_credentials: number
  settings: number
  known_hosts_lines: number
}

export interface BackupExportSummary {
  path: string
  info: BundleInfo
  warnings: string[]
}

export interface RestoreSummary {
  environment: ImportSummary
  vault_added: number
  vault_updated: number
  credentials_linked: number
  settings_applied: number
  known_hosts_added: number
  warnings: string[]
}

// Write the whole environment — including credentials, settings and trusted
// host keys — sealed under `passphrase` (Argon2id + AES-256-GCM) at `path`.
export async function backupExport(path: string, passphrase: string): Promise<BackupExportSummary> {
  return invoke<BackupExportSummary>('backup_export', { path, passphrase })
}

// Decrypt a bundle and describe its contents (counts only) without applying it.
export async function backupInspect(path: string, passphrase: string): Promise<BundleInfo> {
  return invoke<BundleInfo>('backup_inspect', { path, passphrase })
}

// Merge a bundle into this install. Idempotent for sessions; vault entries are
// matched by name; credentials attach only to sessions that have none here.
export async function backupRestore(path: string, passphrase: string): Promise<RestoreSummary> {
  return invoke<RestoreSummary>('backup_restore', { path, passphrase })
}

// ── Sync folder ───────────────────────────────────────────────────────────

/** Tauri event carrying a `SyncStatus` when the background check finds news. */
export const SYNC_EVENT = 'backup-sync'

export interface SyncMeta {
  zapx_sync: number
  device_id: string
  device_name: string
  written_at: string
  app_version: string
}

export type SyncState =
  | 'disabled'
  | 'no_passphrase'
  | 'in_sync'
  | 'local_changes'
  | 'remote_changes'
  | 'conflict'
  | 'error'

export interface SyncStatus {
  state: SyncState
  dir: string | null
  has_passphrase: boolean
  last_push: string | null
  last_pull: string | null
  remote: SyncMeta | null
  detail: string | null
  checked_at: string
}

export interface SyncPullResult {
  restore: RestoreSummary
  status: SyncStatus
}

export async function backupSyncStatus(): Promise<SyncStatus> {
  return invoke<SyncStatus>('backup_sync_status')
}

// Point sync at `dir` (created if missing); `passphrase` sets/replaces the
// stored one. Bookkeeping is reset when the folder changes.
export async function backupSyncConfigure(
  dir: string,
  passphrase: string | null,
): Promise<SyncStatus> {
  return invoke<SyncStatus>('backup_sync_configure', { dir, passphrase })
}

export async function backupSyncDisable(): Promise<SyncStatus> {
  return invoke<SyncStatus>('backup_sync_disable')
}

// Push if only the local side changed; otherwise just report.
export async function backupSyncNow(): Promise<SyncStatus> {
  return invoke<SyncStatus>('backup_sync_now')
}

// Overwrite the remote bundle with this device's environment.
export async function backupSyncPush(): Promise<SyncStatus> {
  return invoke<SyncStatus>('backup_sync_push')
}

// Merge the remote bundle in, then publish the merged result if it differs.
export async function backupSyncPull(): Promise<SyncPullResult> {
  return invoke<SyncPullResult>('backup_sync_pull')
}

// Migrate data to `newDir`, write the startup pointer and return a summary.
// Takes effect on next launch — pair with `restartApp()`.
export async function setDataDir(newDir: string): Promise<string> {
  return invoke<string>('set_data_dir', { newDir })
}

// Revert to the platform default location on next launch (data stays put).
export async function resetDataDir(): Promise<void> {
  return invoke<void>('reset_data_dir')
}

export async function restartApp(): Promise<void> {
  return invoke<void>('restart_app')
}

// Open an http(s) URL in the user's default browser.
export async function openExternal(url: string): Promise<void> {
  return invoke<void>('open_external', { url })
}

// Persist the user-typed password into the backend in-memory cache. Used by
// the reconnect dialog when the OS keychain denies access on dev builds.
export async function cacheSessionPassword(
  savedSessionId: number,
  password: string,
): Promise<void> {
  return invoke<void>('cache_session_password', { savedSessionId, password })
}

// Drop a typed password that turned out to be wrong (memory + encrypted copy).
/** Host and port an open of this saved session dials directly (its first
 *  jump host, or itself): the one whose host key is checked before connecting. */
export async function sshDirectEndpoint(savedSessionId: number): Promise<[string, number]> {
  return invoke('ssh_direct_endpoint', { savedSessionId })
}

/** Where an open of this saved session gets its password from now. */
export async function sessionPasswordSource(
  savedSessionId: number,
): Promise<'keyring' | 'typed' | 'none'> {
  return invoke('session_password_source', { savedSessionId })
}

export async function forgetSessionPassword(savedSessionId: number): Promise<void> {
  return invoke<void>('forget_session_password', { savedSessionId })
}

export type {
  SavedSession,
  Folder,
  BroadcastGroup,
  HighlightRule,
  SessionLog,
  ColorScheme,
  AuthMethod,
  HostKeyStatus,
}
