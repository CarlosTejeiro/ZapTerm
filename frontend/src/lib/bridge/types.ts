// Mirror of core-persistence public model types

export interface SavedSession {
  id: number
  folder_id: number | null
  name: string
  protocol: string
  host: string | null
  port: number | null
  username: string | null
  credential_id: number | null
  options_json: string
  last_used_at: string | null
  auth_method: string | null
  /** Optional ProxyJump: id of the saved SSH session used as the bastion. */
  via_session_id: number | null
  /** Sort order within the parent (folder or root). Set by drag-reorder. */
  sort_order: number
}

// Discriminated union mirroring the backend `AuthMethodArg` (serde tag = "type").
export type AuthMethod =
  | { type: 'password'; password: string }
  | { type: 'key'; keyPath: string; passphrase: string | null }
  | { type: 'keyboard-interactive' }
  | { type: 'agent' }
  // Reuse an existing vault credential by id (no secret carried).
  | { type: 'vaultentry'; credentialId: number }

// One server-driven keyboard-interactive prompt.
export interface KiPrompt {
  prompt: string
  echo: boolean
}

// Payload of the `ssh-ki-prompt` Tauri event.
export interface KiPromptEvent {
  interactionId: string
  name: string
  instructions: string
  prompts: KiPrompt[]
}

// One remote SFTP directory entry, mirroring core_transport::SftpEntry.
export interface SftpEntry {
  name: string
  /** "dir" | "file" | "symlink" | "other" */
  kind: string
  size: number | null
  permissions: number | null
  /** Unix seconds since epoch */
  mtime: number | null
}

// One step of an automated login script, mirroring app::login_script::LoginStep.
export type StepKind = 'expect' | 'send' | 'wait'

export interface LoginStep {
  /** 'expect' = wait for output then send; 'send' = send now; 'wait' = pause. */
  kind: StepKind
  expect: string
  is_regex: boolean
  send: string
  timeout_ms: number
}

// One output trigger, mirroring app::triggers::Trigger. When `pattern` matches
// a line of output, `action` fires: "notify" (toast), "send" (type text+Enter),
// or "bell".
export interface Trigger {
  pattern: string
  is_regex: boolean
  action: 'notify' | 'send' | 'bell'
  text: string
  enabled: boolean
}

// Payload of the `trigger-fired` Tauri event (notify/bell actions).
export interface TriggerFiredEvent {
  session_id: string
  /** "notify" | "bell" */
  kind: string
  text: string
}

// Payload of `login-script-progress` Tauri event, mirroring LoginProgress.
export interface LoginProgressEvent {
  session_id: string
  current: number
  total: number
  /** "running" | "complete" | "timeout" */
  status: string
}

// A command snippet mirroring core_persistence::Snippet.
export interface Snippet {
  id: number
  name: string
  content: string
  sort_order: number
  created_at: string
  /** Platform key (matching PlatformInfo.id) or null for global. */
  platform: string | null
  /** Optional accent color (hex) for the button-bar tile; null = theme tint. */
  color: string | null
  /** When set, this snippet is a macro: a JSON expect/send/wait step array run
   *  on the focused session instead of sending `content`. null = plain text. */
  steps_json: string | null
  /** Optional folder name for the sidebar Macros library; null = ungrouped. */
  folder: string | null
  /** Optional rank within the macro's sidebar folder group (nulls last).
   *  Reordered independently of `sort_order`; null = unranked. */
  position: number | null
  /** Optional keyboard shortcut (e.g. "Ctrl+Alt+P") that fires this snippet
   *  or macro on the focused session; null = none. */
  shortcut: string | null
}

/** A reusable credential ("vault") entry. NEVER carries a secret — only the
 *  metadata needed to list and reference it. The password lives in the OS
 *  keyring and is only ever written straight to a session's PTY backend-side. */
export interface VaultEntry {
  id: number
  name: string
  username: string | null
}

/** Item in the "Recents" zone of the snippet bar — a frequently-typed command
 *  promoted to a one-click button. Shape kept tiny because there's no id, no
 *  user-facing name, and no editable state — they're a read-only view of the
 *  command_history table.
 */
export interface RecentCommand {
  text: string
  freq: number
}

/** TCP MSS snapshot for an SSH session, captured right after connect. Each
 *  direction may be `null` either because the OS doesn't expose it (macOS
 *  doesn't surface receive; Windows neither) or because the session is
 *  tunnelled through a bastion (no direct TCP socket).
 */
export interface TcpMss {
  send: number | null
  recv: number | null
}

// Live port-forward metadata mirroring core_transport::ForwardInfo.
export interface ForwardInfo {
  id: string
  /** "local" or "dynamic" */
  kind: string
  bind_addr: string
  bind_port: number
  /** Present for "local"; null for "dynamic" (SOCKS5 chooses per connection). */
  target_host: string | null
  target_port: number | null
  /** The user's name for this tunnel, or null. */
  name: string | null
}

// A port-forward saved on a session (auto-started on connect), mirroring
// core_persistence::SavedForward. `target_*` are null for dynamic (SOCKS5).
export interface SavedForward {
  /** "local" (-L), "dynamic" (-D, SOCKS5) or "remote" (-R). */
  kind: 'local' | 'dynamic' | 'remote'
  bind_addr: string
  bind_port: number
  target_host: string | null
  target_port: number | null
  /** What the tunnel is for ("FortiGate GUI"…), or null. */
  name?: string | null
}

// Mirror of core-transport `HostKeyStatus` (serde tag = "status").
export type HostKeyStatus =
  | { status: 'known' }
  | { status: 'unknown'; fingerprint: string }
  | { status: 'changed'; fingerprint: string }

export interface Folder {
  id: number
  parent_id: number | null
  name: string
  sort_order: number
}

export interface BroadcastGroup {
  id: number
  name: string
  sort_order: number
  /** Ordered saved-session ids that make up the group. */
  session_ids: number[]
}

export interface HighlightRule {
  id: number
  name: string
  pattern: string
  is_regex: boolean
  fg_color: string | null
  bg_color: string | null
  bold: boolean
  underline: boolean
  enabled: boolean
  sort_order: number
}

export interface ColorScheme {
  id: number
  name: string
  palette_json: string
  is_builtin: boolean
}

export interface ColorPalette {
  background: string
  foreground: string
  cursor: string
  black: string
  red: string
  green: string
  yellow: string
  blue: string
  magenta: string
  cyan: string
  white: string
  brightBlack: string
  brightRed: string
  brightGreen: string
  brightYellow: string
  brightBlue: string
  brightMagenta: string
  brightCyan: string
  brightWhite: string
}

export interface SessionLog {
  id: number
  session_id: number | null
  started_at: string
  ended_at: string | null
  file_path: string
  bytes: number
  format: string
}

// ---------------------------------------------------------------------------
// Hints — command suggestion engine
// ---------------------------------------------------------------------------

export type HintSource =
  | { kind: 'history' }
  | { kind: 'catalog'; platform: string }
  | { kind: 'sequence' }

export interface Hint {
  text: string
  source: HintSource
  score: number
  label: string | null
}

export interface PlatformInfo {
  id: string
  name: string
}
