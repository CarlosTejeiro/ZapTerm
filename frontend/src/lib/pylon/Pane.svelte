<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import TerminalTab from '$lib/terminal/TerminalTab.svelte'
  import type { PylonTheme } from '$lib/themes/index'
  import type { SavedSession, ColorPalette, AuthMethod } from '$lib/bridge/types'
  import { clearCachedPassword, getCachedPassword, setCachedPassword } from '$lib/credentialCache'
  import {
    cacheSessionPassword,
    forgetSessionPassword,
    sessionPasswordSource,
    startSessionLogging,
    stopSessionLogging,
  } from '$lib/bridge/commands'
  import { paneToSession } from '$lib/stores/sessionRuntime.svelte'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { decodeEscapes, runMacro } from '$lib/orchestrator/macroRunner'
  import { snippets } from '$lib/stores/snippets.svelte'
  import type { LoginStep } from '$lib/bridge/types'
  import Icon from '$lib/icons/Icon.svelte'
  import SftpDialog from '$lib/terminal/SftpDialog.svelte'
  import TunnelsDialog from '$lib/terminal/TunnelsDialog.svelte'

  interface SshParams {
    host: string
    port: number
    user: string
    auth: AuthMethod
  }
  interface TelnetParams {
    host: string
    port: number
  }

  export interface PaneData {
    id: number
    label: string
    color: string
    savedSession?: SavedSession
    ssh?: SshParams
    telnet?: TelnetParams
    /** A freshly-split placeholder pane with no session yet — renders a
     *  "pick a session" prompt and is filled by clicking a sidebar session. */
    empty?: boolean
    /** Opened by a read-only script or notebook: don't run the session's
     *  on-connect macro (it would type commands the allowlist never saw). */
    skipOnConnectMacro?: boolean
  }

  type PaneStatus = 'connecting' | 'connected' | 'error' | 'lost' | 'closed'

  interface Props {
    theme: PylonTheme
    pane: PaneData
    focused: boolean
    /** Whether this pane shares the tab with other panes (split or grid). The
     *  focus ring only conveys information when there is more than one pane —
     *  on a solo pane it is pure visual noise — so it is drawn only when true. */
    hasSiblings?: boolean
    /** Whether closing this pane is allowed (false when the tab has a single leaf). */
    canClose?: boolean
    onFocus: () => void
    onSplitH?: () => void
    onSplitV?: () => void
    onClosePane?: () => void
    onStatusChange?: (status: PaneStatus) => void
    onGlobalShortcut?: (key: string, e: KeyboardEvent) => void
  }

  const {
    theme,
    pane,
    focused,
    hasSiblings = false,
    canClose = false,
    onFocus,
    onSplitH,
    onSplitV,
    onClosePane,
    onStatusChange,
    onGlobalShortcut,
  }: Props = $props()

  let status = $state<PaneStatus>('connecting')

  // ── Session logging (SecureCRT-style record button) ──────────────────
  // Toggled from the pane header. Writes raw terminal bytes to a rotating
  // file under the app's session_logs directory. Logging keeps recording
  // until the user stops it or the underlying session ends.
  let isLogging = $state(false)
  let logPath = $state<string | null>(null)
  let logBusy = $state(false)

  async function toggleLogging() {
    if (logBusy) return
    const sid = paneToSession.get(pane.id)
    if (!sid) {
      showToast({ kind: 'warning', title: 'Logging', detail: 'The session is not active.' })
      return
    }
    logBusy = true
    try {
      if (isLogging) {
        await stopSessionLogging(sid)
        isLogging = false
        showToast({ kind: 'info', title: 'Logging detenido', detail: logPath ?? undefined })
        logPath = null
      } else {
        const name = pane.savedSession?.name ?? pane.ssh?.host ?? pane.telnet?.host ?? pane.label
        const savedId = pane.savedSession?.id ?? null
        const p = await startSessionLogging(sid, savedId, name)
        logPath = p
        isLogging = true
        showToast({ kind: 'success', title: 'Logging started', detail: p })
      }
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Logging failed',
        detail: errorText(e),
      })
    } finally {
      logBusy = false
    }
  }

  // If the pane closes / disconnects while recording, the backend already
  // stops the writer in close_session; reset our local flag so the next
  // open doesn't think it's still recording.
  $effect(() => {
    if (status === 'closed' || status === 'error' || status === 'lost') {
      isLogging = false
      logPath = null
    }
  })

  function handleStatus(s: PaneStatus) {
    if (s === 'connected') retriedWithCache = false
    status = s
    onStatusChange?.(s)
  }

  // Password re-entry when keyring credential is missing
  let needsPassword = $state(false)
  let passwordInput = $state('')
  let sshOverride = $state<SshParams | null>(null)
  let termKey = $state(0)

  /// The saved session was retried with a just-cached password; a second
  /// "need password" right after means it didn't help → show the form.
  let retriedWithCache = false

  function handleNeedPassword() {
    const s = pane.savedSession
    const cached = s ? getCachedPassword(s.id) : null
    if (s && cached && !retriedWithCache) {
      // Use the password typed earlier in this run silently — no form shown.
      void reconnectSaved(s.id, cached)
      return
    }
    retriedWithCache = false
    needsPassword = true
    status = 'error'
    onStatusChange?.('error')
  }

  /// Reconnect as the SAVED session — jump host, saved port forwards, login
  /// automation, platform all apply — with `pw` standing in for the missing
  /// keyring secret. The backend reads it from its session-password cache
  /// (`cache_session_password` also keeps an encrypted copy for restarts).
  /// If caching fails, fall back to a plain SSH connection with `pw`.
  async function reconnectSaved(savedId: number, pw: string) {
    status = 'connecting'
    onStatusChange?.('connecting')
    // The form (or the failed terminal) stays until the password is cached:
    // mounting the terminal earlier would try the saved session without it.
    try {
      if (forgetting) {
        await forgetting
        forgetting = null
      }
      await cacheSessionPassword(savedId, pw)
      sshOverride = null
      retriedWithCache = true
    } catch (err) {
      console.error(err)
      const s = pane.savedSession
      sshOverride = {
        host: s?.host ?? '',
        port: s?.port ?? 22,
        user: s?.username ?? '',
        auth: { type: 'password', password: pw },
      }
    }
    needsPassword = false
    termKey++
  }

  /// Shown above the password form after a typed password was rejected.
  let passwordError = $state('')
  /// The backend forget of a rejected password, still running. A new password
  /// is only cached once it is done, or the forget could erase it.
  let forgetting: Promise<void> | null = null

  /// The server rejected the password. If it was one typed in this pane, it is
  /// wrong: forget it everywhere and ask again (instead of retrying it on
  /// every connect). A wrong password from the keyring stays a plain error —
  /// fix it in the session dialog.
  async function handleAuthFailed(): Promise<boolean> {
    const s = pane.savedSession
    if (!s) return false
    if (!retriedWithCache) {
      // Not just typed here: act only when the open used a password typed in
      // this pane earlier (kept in memory or encrypted), never on a wrong
      // keyring entry or a non-password session.
      const passwordAuth = s.auth_method == null || s.auth_method === 'password'
      if (!passwordAuth) return false
      const source = await sessionPasswordSource(s.id).catch(() => 'none')
      if (source !== 'typed') return false
    }
    retriedWithCache = false
    clearCachedPassword(s.id)
    forgetting = forgetSessionPassword(s.id).catch(console.error)
    passwordError = 'Wrong password — try again.'
    needsPassword = true
    status = 'error'
    onStatusChange?.('error')
    return true
  }

  function handlePasswordSubmit(e: Event) {
    e.preventDefault()
    if (!passwordInput.trim()) return
    const s = pane.savedSession
    if (!s) return
    const pw = passwordInput
    setCachedPassword(s.id, pw)
    passwordInput = ''
    passwordError = ''
    void reconnectSaved(s.id, pw)
  }

  const protocol = $derived(
    pane.savedSession?.protocol ?? (pane.ssh ? 'ssh' : pane.telnet ? 'telnet' : 'local'),
  )

  const hostLabel = $derived(pane.ssh?.host ?? pane.savedSession?.host ?? '')

  // SFTP + port-forward dialogs are SSH-only and need the live runtime
  // session id (the redesign hid TerminalTab's own toolbar, so the pane
  // header re-exposes these here).
  const isSsh = $derived(protocol === 'ssh')
  const runtimeSid = $derived(paneToSession.get(pane.id) ?? null)
  let showSftp = $state(false)
  let showTunnels = $state(false)

  // Anti-idle / keepalive: send a configured string on a timer while connected,
  // for servers whose idle timeout fires despite SSH keepalives. Configured per
  // saved session in options_json: { anti_idle: { interval_sec, send } }.
  const antiIdle = $derived.by<{ intervalSec: number; send: string } | null>(() => {
    const raw = pane.savedSession?.options_json
    if (!raw) return null
    try {
      const ai = (JSON.parse(raw) as { anti_idle?: { interval_sec?: unknown; send?: unknown } })
        .anti_idle
      if (
        ai &&
        typeof ai.interval_sec === 'number' &&
        ai.interval_sec > 0 &&
        typeof ai.send === 'string' &&
        ai.send.length > 0
      ) {
        return { intervalSec: ai.interval_sec, send: ai.send }
      }
    } catch {
      /* malformed options_json — no keepalive */
    }
    return null
  })

  $effect(() => {
    if (status !== 'connected' || !antiIdle || !runtimeSid) return
    const sid = runtimeSid
    const bytes = Array.from(new TextEncoder().encode(decodeEscapes(antiIdle.send)))
    if (bytes.length === 0) return
    const timer = setInterval(() => {
      invoke('send_input', { sessionId: sid, data: bytes }).catch(() => {})
    }, antiIdle.intervalSec * 1000)
    return () => clearInterval(timer)
  })

  // On-connect macro: when the saved session names an `on_connect_macro_id`,
  // run that macro once each time the session reaches `connected`. Driven from
  // the frontend (like the snippet macro runner) so `{{vault:…}}` steps resolve.
  // The guard resets on disconnect so a reconnect re-runs it.
  const onConnectMacroId = $derived.by<number | null>(() => {
    const raw = pane.savedSession?.options_json
    if (!raw) return null
    try {
      const id = (JSON.parse(raw) as { on_connect_macro_id?: unknown }).on_connect_macro_id
      return typeof id === 'number' ? id : null
    } catch {
      return null
    }
  })

  let onConnectRan = $state(false)
  $effect(() => {
    if (status !== 'connected') {
      // Allow a re-run after a reconnect.
      if (status === 'closed' || status === 'error' || status === 'lost') onConnectRan = false
      return
    }
    if (onConnectRan || onConnectMacroId === null || !runtimeSid) return
    if (pane.skipOnConnectMacro) return
    const macro = snippets.find((s) => s.id === onConnectMacroId)
    if (!macro?.steps_json) return
    let steps: LoginStep[]
    try {
      steps = JSON.parse(macro.steps_json)
    } catch {
      return
    }
    onConnectRan = true
    const sid = runtimeSid
    const macroName = macro.name
    runMacro(sid, steps)
      .then((res) => {
        if (!res.ok) {
          showToast({
            kind: 'error',
            title: `On-connect macro "${macroName}" failed`,
            detail: `step ${(res.failedStep ?? 0) + 1}: ${res.error ?? 'timed out'}`,
          })
        }
      })
      .catch(() => {})
  })

  // Map PylonTheme terminal tokens → xterm ColorPalette
  const terminalPalette = $derived<ColorPalette>({
    background: theme.terminal.bg,
    foreground: theme.terminal.fg,
    cursor: theme.terminal.cursor,
    black: theme.terminal.black,
    red: theme.terminal.red,
    green: theme.terminal.green,
    yellow: theme.terminal.yellow,
    blue: theme.terminal.blue,
    magenta: theme.terminal.magenta,
    cyan: theme.terminal.cyan,
    white: theme.terminal.white,
    brightBlack: theme.terminal.brightBlack,
    brightRed: theme.terminal.brightRed,
    brightGreen: theme.terminal.brightGreen,
    brightYellow: theme.terminal.brightYellow,
    brightBlue: theme.terminal.brightBlue,
    brightMagenta: theme.terminal.brightMagenta,
    brightCyan: theme.terminal.brightCyan,
    brightWhite: theme.terminal.brightWhite,
  })
</script>

<!-- The terminal floats as a card: rounded, clipped, soft shadow. When the
     tab holds several panes, the focused one adds a soft accent inset ring so
     the keyboard target is obvious; a solo pane keeps the neutral border, since
     a full-strength accent frame around the only terminal was the loudest
     element on screen and signalled nothing. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="pane"
  style:background={theme.terminal.bg}
  style:border-radius="calc({theme.radius} + 4px)"
  style:box-shadow={focused && hasSiblings
    ? `inset 0 0 0 1px color-mix(in srgb, ${theme.accent} 60%, transparent), 0 1px 2px rgba(0,0,0,0.14), 0 8px 26px rgba(0,0,0,0.18)`
    : `inset 0 0 0 1px ${theme.border}, 0 1px 2px rgba(0,0,0,0.12), 0 6px 20px rgba(0,0,0,0.14)`}
  onclick={onFocus}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') onFocus()
  }}
  role="region"
  tabindex="-1"
>
  <!-- Pane header (22px) — always dark, floats on terminal surface -->
  <div
    class="pane-header"
    style:background={theme.terminal.bg}
    style:border-bottom="1px solid rgba(255,255,255,0.07)"
    style:font-family={theme.fontUi}
  >
    <span class="ph-dot" style:background={pane.color}></span>
    <span class="ph-name" style:color={theme.terminal.fg}>{pane.label}</span>
    {#if hostLabel}
      <span class="ph-host" style:color={theme.terminal.dim}>{hostLabel}</span>
    {/if}
    {#if !pane.empty}
      <span class="ph-proto" style:color={theme.terminal.dim}>{protocol.toUpperCase()}</span>
    {/if}
    <span class="ph-actions">
      {#if !pane.empty}
        <button
          class="ph-btn ph-btn-rec"
          class:recording={isLogging}
          type="button"
          title={isLogging ? `Stop logging\n${logPath ?? ''}` : 'Start logging this session'}
          aria-label={isLogging ? 'Stop logging' : 'Start logging'}
          disabled={logBusy}
          style:color={isLogging ? theme.err : theme.terminal.fg}
          onclick={(e) => {
            e.stopPropagation()
            toggleLogging()
          }}><Icon name="record" size={10} /></button
        >
        {#if isSsh}
          <button
            class="ph-btn"
            type="button"
            title="SFTP file browser"
            aria-label="SFTP file browser"
            disabled={!runtimeSid}
            style:color={theme.terminal.fg}
            onclick={(e) => {
              e.stopPropagation()
              showSftp = true
            }}><Icon name="transfer" size={12} /></button
          >
          <button
            class="ph-btn"
            type="button"
            title="Port forwards (-L / -D / -R)"
            aria-label="Port forwards"
            disabled={!runtimeSid}
            style:color={theme.terminal.fg}
            onclick={(e) => {
              e.stopPropagation()
              showTunnels = true
            }}><Icon name="tunnel" size={12} /></button
          >
        {/if}
        {#if onSplitH}
          <button
            class="ph-btn"
            type="button"
            title="Split horizontally"
            aria-label="Split horizontally"
            style:color={theme.terminal.fg}
            onclick={(e) => {
              e.stopPropagation()
              onSplitH?.()
            }}><Icon name="split" size={12} /></button
          >
        {/if}
        {#if onSplitV}
          <button
            class="ph-btn"
            type="button"
            title="Split vertically"
            aria-label="Split vertically"
            style:color={theme.terminal.fg}
            onclick={(e) => {
              e.stopPropagation()
              onSplitV?.()
            }}><Icon name="splitV" size={12} /></button
          >
        {/if}
      {/if}
      {#if canClose && onClosePane}
        <button
          class="ph-btn ph-btn-close"
          type="button"
          title="Close pane"
          aria-label="Close pane"
          style:color={theme.terminal.fg}
          onclick={(e) => {
            e.stopPropagation()
            onClosePane?.()
          }}><Icon name="x" size={12} /></button
        >
      {/if}
    </span>
  </div>

  <!-- Terminal body -->
  <div class="pane-body" style:--term-bg={theme.terminal.bg}>
    {#if pane.empty}
      <!-- Freshly-split placeholder: filled by clicking a sidebar session. -->
      <div class="empty-overlay" style:background={theme.terminal.bg}>
        <svg
          class="empty-icon"
          width="30"
          height="30"
          viewBox="0 0 24 24"
          fill="none"
          stroke={theme.terminal.dim}
          stroke-width="1.6"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <line x1="12" y1="3" x2="12" y2="21" />
        </svg>
        <p class="empty-title" style:color={theme.terminal.fg}>Empty pane</p>
        <p class="empty-sub" style:color={theme.terminal.dim}>
          Click a session in the sidebar to open it here.
        </p>
      </div>
    {:else if needsPassword}
      <!-- Inline re-auth form when keyring credential is missing -->
      <div class="pw-overlay" style:background={theme.terminal.bg}>
        <form class="pw-box" onsubmit={handlePasswordSubmit} style:font-family={theme.fontUi}>
          <svg
            class="pw-icon"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke={theme.terminal.cursor}
            stroke-width="2"
          >
            <rect x="3" y="11" width="18" height="11" rx="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" />
          </svg>
          <p class="pw-title" style:color={theme.terminal.fg}>
            {pane.savedSession?.username ?? 'user'}@{pane.savedSession?.host ?? 'host'}
          </p>
          {#if passwordError}
            <p class="pw-error" role="alert">{passwordError}</p>
          {:else}
            <p class="pw-sub" style:color={theme.terminal.dim}>
              Credentials missing from keyring — enter password to reconnect
            </p>
          {/if}
          <input
            class="pw-input"
            type="password"
            placeholder="Password"
            bind:value={passwordInput}
            autocomplete="current-password"
            style:border="1px solid {theme.terminal.dim}"
            style:color={theme.terminal.fg}
          />
          <button class="pw-btn" type="submit" style:background={theme.terminal.cursor}
            >Connect</button
          >
        </form>
      </div>
    {:else}
      {#key termKey}
        <TerminalTab
          savedSession={sshOverride ? undefined : pane.savedSession}
          ssh={sshOverride ?? pane.ssh}
          telnet={pane.telnet}
          paneId={pane.id}
          hideToolbar={true}
          pylonPalette={terminalPalette}
          {onGlobalShortcut}
          onNeedPassword={handleNeedPassword}
          onAuthFailed={handleAuthFailed}
          onSessionOpen={() => handleStatus('connected')}
          onSessionError={() => handleStatus('error')}
          onSessionLost={() => handleStatus('lost')}
          onSessionClose={() => handleStatus('closed')}
        />
      {/key}
    {/if}
  </div>
</div>

{#if showSftp && runtimeSid}
  <SftpDialog sessionId={runtimeSid} onClose={() => (showSftp = false)} />
{/if}
{#if showTunnels && runtimeSid}
  <TunnelsDialog sessionId={runtimeSid} onClose={() => (showTunnels = false)} />
{/if}

<style>
  .pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    transition: box-shadow 0.1s;
    position: relative;
  }

  .pane-header {
    height: 26px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    flex-shrink: 0;
    user-select: none;
    cursor: default;
  }

  .ph-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ph-name {
    font-size: 11.5px;
    font-weight: 500;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ph-host {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 140px;
  }

  .ph-proto {
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.4px;
    flex-shrink: 0;
  }

  .ph-actions {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    margin-left: 4px;
  }

  /* Buttons paint on the FULL terminal foreground colour now (was the
     dim variant) — only the opacity dial steps them back so the active
     terminal still leads the eye. */
  .ph-btn {
    background: rgba(255, 255, 255, 0.06);
    border: none;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
    padding: 3px 7px;
    border-radius: 4px;
    opacity: 0.9;
    transition:
      opacity 0.1s,
      background 0.1s,
      color 0.1s;
    font-family: inherit;
  }

  .ph-btn:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.18);
  }

  .ph-btn-close:hover {
    background: rgba(239, 68, 68, 0.18);
    color: #ef4444 !important;
  }

  /* Recording state — red pulsing dot like SecureCRT's session log
     indicator. The button stays visible (not just on hover) while
     logging is active so the user knows. */
  .ph-btn-rec.recording {
    opacity: 1 !important;
    animation: ph-rec-pulse 1.4s ease-in-out infinite;
  }
  .ph-btn-rec.recording:hover {
    background: rgba(239, 68, 68, 0.18);
  }
  @keyframes ph-rec-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }

  /* Inner breathing room so terminal text doesn't kiss the card edge —
     the FitAddon measures the padded box, so cols/rows stay correct. */
  .pane-body {
    flex: 1;
    overflow: hidden;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 2px 6px 6px;
  }

  .pw-overlay {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-overlay {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    user-select: none;
  }
  .empty-icon {
    opacity: 0.7;
    margin-bottom: 4px;
  }
  .empty-title {
    font-size: 13px;
    font-weight: 600;
    margin: 0;
  }
  .empty-sub {
    font-size: 11.5px;
    margin: 0;
    text-align: center;
    max-width: 80%;
    line-height: 1.5;
  }

  .pw-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 280px;
    padding: 28px 24px;
  }

  .pw-icon {
    margin-bottom: 4px;
  }

  .pw-title {
    font-size: 13px;
    font-weight: 600;
    margin: 0;
    text-align: center;
  }

  .pw-error {
    margin: 0;
    font-size: 0.78rem;
    color: var(--zx-err);
    text-align: center;
  }
  .pw-sub {
    font-size: 11px;
    text-align: center;
    margin: 0;
    line-height: 1.5;
    opacity: 0.7;
  }

  .pw-input {
    width: 100%;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 5px;
    padding: 7px 10px;
    font-size: 13px;
    outline: none;
    font-family: inherit;
    margin-top: 4px;
  }

  .pw-input:focus {
    border-color: rgba(255, 255, 255, 0.35) !important;
  }

  .pw-btn {
    width: 100%;
    border: none;
    border-radius: 5px;
    padding: 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: #fff;
    cursor: pointer;
    font-family: inherit;
    opacity: 0.9;
    transition: opacity 0.1s;
  }

  .pw-btn:hover {
    opacity: 1;
  }
</style>
