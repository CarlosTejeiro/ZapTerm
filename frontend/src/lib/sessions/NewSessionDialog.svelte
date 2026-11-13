<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon from '$lib/icons/Icon.svelte'
  import { open as openDialog } from '@tauri-apps/plugin-dialog'
  import {
    createSavedSession,
    createTelnetSession,
    createSerialSession,
    createWslSession,
    listWslDistros,
    listSerialPorts,
    listSessions,
    setLoginScript,
    updateSavedSession,
    getLoginScript,
    listPlatforms,
    setSessionPlatform,
    clearSessionPlatform,
    getSessionPlatform,
    getSessionForwards,
    setSessionForwards,
    getSessionTriggers,
    setSessionTriggers,
    listVaultEntries,
    setSessionCredential,
  } from '$lib/bridge/commands'
  import type {
    Folder,
    AuthMethod,
    SavedSession,
    LoginStep,
    Trigger,
    PlatformInfo,
    SavedForward,
    VaultEntry,
  } from '$lib/bridge/types'
  import { colorSchemes } from '$lib/stores/settings.svelte'
  import { setCachedPassword } from '$lib/credentialCache'
  import { isWindowsHost } from '$lib/stores/localShellPlatform'
  import { listSnippets } from '$lib/bridge/commands'
  import { escapeControls, unescapeControls } from './stepEscapes'
  import type { Snippet } from '$lib/bridge/types'

  // 'wsl' is UI-only: it's saved as protocol 'local' with options_json.wsl_distro.
  type Protocol = 'ssh' | 'telnet' | 'serial' | 'wsl'

  interface Props {
    folders: Folder[]
    /// When set, the dialog runs in "edit" mode: editable fields are pre-filled
    /// and Save calls `updateSavedSession`. Credentials are editable too — when
    /// the auth method or secret changes, Save additionally calls
    /// `setSessionCredential` (left untouched otherwise, so the stored secret is
    /// preserved).
    existing?: SavedSession
    onCreated: (id: number) => void
    onCancel: () => void
  }

  const { folders, existing, onCreated, onCancel }: Props = $props()
  const isEdit = $derived(existing != null)

  // SSH sessions available as a jump host (bastion).
  let bastionCandidates = $state<SavedSession[]>([])
  onMount(async () => {
    try {
      const all = await listSessions()
      // Exclude self from the bastion list in edit mode.
      bastionCandidates = all.filter(
        (s) => s.protocol === 'ssh' && (!existing || s.id !== existing.id),
      )
    } catch {
      bastionCandidates = []
    }
    try {
      vaultEntries = await listVaultEntries()
    } catch {
      vaultEntries = []
    }
    try {
      platforms = await listPlatforms()
    } catch {
      platforms = []
    }
    try {
      // Only macros (snippets with steps) can run on connect.
      macroSnippets = (await listSnippets()).filter((s) => s.steps_json)
    } catch {
      macroSnippets = []
    }
    if (existing) {
      try {
        platform = await getSessionPlatform(existing.id)
      } catch {
        platform = null
      }
    }
    if (existing) {
      protocol = (existing.protocol as Protocol) ?? 'ssh'
      if (existing.protocol === 'local') protocol = 'wsl'
      name = existing.name
      host = existing.host ?? ''
      port = existing.port ?? defaultPort[protocol]
      username = existing.username ?? ''
      authType = (existing.auth_method as typeof authType) ?? 'password'
      folderId = existing.folder_id
      viaSessionId = existing.via_session_id
      // If this password session already points at a vault entry, preselect it
      // so the picker shows the current choice and "unchanged" is detectable.
      // Private per-session credentials aren't in `vaultEntries` (reusable only),
      // so `vaultCredentialId` stays null and the password field shows blank
      // (= keep the current secret).
      if (
        authType === 'password' &&
        existing.credential_id != null &&
        vaultEntries.some((v) => v.id === existing.credential_id)
      ) {
        vaultCredentialId = existing.credential_id
      }
      // Snapshot the starting credential so Save can tell whether it changed and
      // only then re-point it (an untouched edit must preserve the stored secret).
      initialAuthType = authType
      initialVaultCredentialId = vaultCredentialId
      // Protocol-specific options.
      try {
        const opts = JSON.parse(existing.options_json || '{}')
        if (opts.key_path) keyPath = opts.key_path
        if (opts.device) device = opts.device
        if (opts.baud_rate) baudRate = opts.baud_rate
        if (typeof opts.wsl_distro === 'string') wslDistro = opts.wsl_distro
        if (typeof opts.wsl_user === 'string') wslUser = opts.wsl_user
        if (typeof opts.color_scheme === 'string' && opts.color_scheme) {
          colorScheme = opts.color_scheme
        }
        if (typeof opts.notes === 'string') notes = opts.notes
        if (Array.isArray(opts.tags)) tagsInput = opts.tags.join(', ')
        if (opts.anti_idle && typeof opts.anti_idle === 'object') {
          if (typeof opts.anti_idle.interval_sec === 'number')
            antiIdleSec = String(opts.anti_idle.interval_sec)
          if (typeof opts.anti_idle.send === 'string') antiIdleSend = opts.anti_idle.send
        }
        autoReconnect = opts.auto_reconnect !== false
        if (typeof opts.on_connect_macro_id === 'number') {
          onConnectMacroId = opts.on_connect_macro_id
        }
      } catch {
        // ignore malformed options_json
      }
      // Preload the login script.
      try {
        // Show stored control characters as the escapes Save turns back into
        // them (`\r`, `\n`, `\t`, `\\`). A one-line input drops CR/LF from
        // its value, so editing a step used to lose its Enter or newlines, and
        // a bare-Enter step showed up empty.
        loginSteps = (await getLoginScript(existing.id)).map((st) => ({
          ...st,
          send: escapeControls(st.send),
        }))
        if (loginSteps.length > 0) showLoginScript = true
      } catch {
        loginSteps = []
      }
      // Preload output triggers.
      try {
        triggers = (await getSessionTriggers(existing.id)).map((t) => ({
          ...t,
          text: escapeControls(t.text),
        }))
        if (triggers.length > 0) showTriggers = true
      } catch {
        triggers = []
      }
      // Preload saved port-forwards.
      try {
        forwards = await getSessionForwards(existing.id)
        if (forwards.length > 0) showForwards = true
      } catch {
        forwards = []
      }
    } else if (protocol === 'serial') {
      loadPorts()
    }
    if (protocol === 'wsl') loadDistros()
  })

  let protocol = $state<Protocol>('ssh')
  let name = $state('')
  let host = $state('')
  let port = $state(22)
  let username = $state('')
  let authType = $state<'password' | 'key' | 'keyboard-interactive' | 'agent'>('password')
  let password = $state('')
  // Reusable vault credentials available for password auth. `vaultCredentialId`
  // is the chosen entry (null = type a password as usual).
  let vaultEntries = $state<VaultEntry[]>([])
  let vaultCredentialId = $state<number | null>(null)
  /// Edit-mode snapshot of the starting credential, captured in onMount, so Save
  /// only re-points the credential when the user actually changed it.
  let initialAuthType = $state<typeof authType | null>(null)
  let initialVaultCredentialId = $state<number | null>(null)
  let keyPath = $state('')
  let passphrase = $state('')

  /// When a vault credential is chosen, prefill the Username field with that
  /// entry's username (empty when it has none). Done on the select's change
  /// rather than a broad $effect so it doesn't clobber manual edits.
  function onVaultCredentialChange() {
    if (vaultCredentialId === null) return
    const entry = vaultEntries.find((v) => v.id === vaultCredentialId)
    username = entry?.username ?? ''
  }
  let device = $state('')
  let baudRate = $state(9600)
  let folderId = $state<number | null>(null)
  let viaSessionId = $state<number | null>(null)
  /// Per-session color scheme override (null = use global default).
  let colorScheme = $state<string | null>(null)
  /// Per-session command-hint platform (null = use global default from Settings).
  let platform = $state<string | null>(null)
  let platforms = $state<PlatformInfo[]>([])
  /// Freeform notes + comma-separated tags, persisted inside options_json.
  let notes = $state('')
  let tagsInput = $state('')
  /// Anti-idle / keepalive: send `antiIdleSend` every `antiIdleSec` seconds
  /// while connected (for servers whose idle timeout fires despite SSH
  /// keepalives). Empty interval = disabled. Persisted in options_json.
  let antiIdleSec = $state('')
  let antiIdleSend = $state('')
  /// Auto-reconnect this saved session when the link drops (manual reconnect
  /// always works regardless). Default on; stored in options_json only when
  /// disabled, so it falls back to the global default otherwise.
  let autoReconnect = $state(true)
  /// Optional macro run automatically when the session reaches `connected`
  /// (frontend-driven, so `{{vault:…}}` steps resolve). Stored in options_json
  /// as `on_connect_macro_id`. `null` = none.
  let macroSnippets = $state<Snippet[]>([])
  let onConnectMacroId = $state<number | null>(null)
  let error = $state('')

  // Optional login automation: list of expect/send pairs run after connect.
  let showLoginScript = $state(false)
  let loginSteps = $state<LoginStep[]>([])
  function addStep() {
    loginSteps.push({ kind: 'expect', expect: '', is_regex: false, send: '', timeout_ms: 10000 })
  }
  function removeStep(idx: number) {
    loginSteps.splice(idx, 1)
  }

  // Optional output triggers: fire an action when a pattern appears in output.
  let showTriggers = $state(false)
  let triggers = $state<Trigger[]>([])
  function addTrigger() {
    triggers.push({ pattern: '', is_regex: false, action: 'notify', text: '', enabled: true })
  }
  function removeTrigger(idx: number) {
    triggers.splice(idx, 1)
  }

  // Optional saved port-forwards (SSH only), auto-started on connect.
  let showForwards = $state(false)
  let forwards = $state<SavedForward[]>([])
  function addForward() {
    forwards.push({
      kind: 'local',
      bind_addr: '127.0.0.1',
      bind_port: 0,
      target_host: '',
      target_port: 0,
      name: '',
    })
  }
  function removeForward(idx: number) {
    forwards.splice(idx, 1)
  }

  /// Native file picker for the SSH private key.
  async function pickKeyFile() {
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        title: 'Select SSH private key',
      })
      if (typeof selected === 'string' && selected.length > 0) {
        keyPath = selected
      }
    } catch (e) {
      error = errorText(e)
    }
  }
  let saving = $state(false)
  let availablePorts = $state<string[]>([])

  const defaultPort: Record<Protocol, number> = { ssh: 22, telnet: 23, serial: 0, wsl: 0 }
  /// Network protocols: the only ones with a host and port.
  const isNet = $derived(protocol === 'ssh' || protocol === 'telnet')
  let wslDistro = $state('')
  let wslUser = $state('')
  let wslDistros = $state<string[]>([])

  async function loadDistros() {
    try {
      wslDistros = await listWslDistros()
      if (wslDistros.length > 0 && !wslDistro) wslDistro = wslDistros[0] ?? ''
    } catch {
      wslDistros = []
    }
  }

  function onProtocolChange() {
    port = defaultPort[protocol]
    error = ''
    if (protocol === 'serial' && availablePorts.length === 0) {
      loadPorts()
    }
    if (protocol === 'wsl' && wslDistros.length === 0) loadDistros()
  }

  async function loadPorts() {
    try {
      availablePorts = await listSerialPorts()
      if (availablePorts.length > 0 && !device) {
        device = availablePorts[0] ?? ''
      }
    } catch {
      availablePorts = []
    }
  }

  async function submit() {
    if (!name.trim()) {
      error = 'Name is required.'
      return
    }
    if (isNet && !host.trim()) {
      error = 'Host is required.'
      return
    }
    if (isNet && (!Number.isInteger(port) || port < 1 || port > 65535)) {
      error = 'Port must be a number from 1 to 65535.'
      return
    }
    if (protocol === 'ssh' && !username.trim()) {
      error = 'Username is required.'
      return
    }
    if (protocol === 'ssh' && authType === 'key' && !keyPath.trim()) {
      error = 'Key file path is required.'
      return
    }
    if (protocol === 'serial' && !device.trim()) {
      error = 'Serial device is required.'
      return
    }
    if (protocol === 'wsl' && !wslDistro.trim()) {
      error = 'WSL distribution is required.'
      return
    }

    saving = true
    error = ''
    try {
      let id: number
      /// Build the protocol-specific options blob (used in both edit and the
      /// post-create touch-up when a color_scheme override is set).
      const buildOptsJson = (): string | null => {
        const opts: Record<string, unknown> = {}
        if (protocol === 'ssh' && authType === 'key') opts.key_path = keyPath.trim()
        if (protocol === 'serial') {
          opts.device = device.trim()
          opts.baud_rate = baudRate
        }
        if (protocol === 'wsl') {
          opts.wsl_distro = wslDistro.trim()
          if (wslUser.trim()) opts.wsl_user = wslUser.trim()
        }
        if (colorScheme) opts.color_scheme = colorScheme
        if (notes.trim()) opts.notes = notes.trim()
        const tagList = tagsInput
          .split(',')
          .map((t) => t.trim())
          .filter(Boolean)
        if (tagList.length) opts.tags = tagList
        const aiSec = parseInt(antiIdleSec, 10)
        if (aiSec > 0 && antiIdleSend.length > 0) {
          opts.anti_idle = { interval_sec: aiSec, send: antiIdleSend }
        }
        // Only persisted when disabled; absent = follow the global default.
        if (!autoReconnect) opts.auto_reconnect = false
        if (onConnectMacroId !== null) opts.on_connect_macro_id = onConnectMacroId
        return Object.keys(opts).length === 0 ? null : JSON.stringify(opts)
      }
      // Build the AuthMethod for the current form state. Shared by create and
      // the edit-mode credential update so the two never drift.
      const buildAuth = (): AuthMethod =>
        authType === 'key'
          ? { type: 'key', keyPath: keyPath.trim(), passphrase: passphrase || null }
          : authType === 'agent'
            ? { type: 'agent' }
            : authType === 'keyboard-interactive'
              ? { type: 'keyboard-interactive' }
              : vaultCredentialId !== null
                ? { type: 'vaultentry', credentialId: vaultCredentialId }
                : { type: 'password', password }
      if (existing) {
        // Edit mode — metadata (name/host/port/username/via + options_json,
        // where SSH key_path and serial device/baud live) goes through
        // updateSavedSession; the credential is updated separately, and only
        // when it actually changed, so an untouched edit preserves the secret.
        const optsJson = buildOptsJson()
        await updateSavedSession(
          existing.id,
          name.trim(),
          folderId,
          isNet ? host.trim() : null,
          isNet ? port : null,
          protocol === 'ssh' ? username.trim() : null,
          protocol === 'ssh' ? viaSessionId : null,
          optsJson,
        )
        id = existing.id
        if (protocol === 'ssh') {
          // A credential change is: switching auth method, picking a different
          // vault entry (or toggling between vault and a typed password), or
          // supplying a fresh secret. A blank password/passphrase = keep current.
          const methodChanged = authType !== initialAuthType
          const vaultChanged =
            authType === 'password' && vaultCredentialId !== initialVaultCredentialId
          const newPassword =
            authType === 'password' && vaultCredentialId === null && password.length > 0
          const newPassphrase = authType === 'key' && passphrase.length > 0
          if (methodChanged || vaultChanged || newPassword || newPassphrase) {
            await setSessionCredential(existing.id, buildAuth())
            if (newPassword) setCachedPassword(existing.id, password)
          }
        }
      } else if (protocol === 'ssh') {
        const auth: AuthMethod = buildAuth()
        id = await createSavedSession(
          name.trim(),
          folderId,
          host.trim(),
          port,
          username.trim(),
          auth,
          viaSessionId,
        )
        // Prime the in-memory cache so the next open in this app lifetime
        // doesn't re-prompt even if the OS Keychain denies the dev-built
        // binary access to its own entry (very common on macOS without
        // code signing).
        if (authType === 'password' && password) {
          setCachedPassword(id, password)
        }
      } else if (protocol === 'telnet') {
        id = await createTelnetSession(name.trim(), folderId, host.trim(), port)
      } else if (protocol === 'wsl') {
        id = await createWslSession(name.trim(), folderId, wslDistro.trim(), wslUser.trim() || null)
      } else {
        id = await createSerialSession(name.trim(), folderId, device.trim(), baudRate)
      }
      // Post-create touch-up: if a color-scheme override, notes or tags were
      // set, persist them into options_json. We rebuild the same blob the
      // backend would have written so nothing protocol-specific is lost.
      if (
        !existing &&
        (colorScheme ||
          notes.trim() ||
          tagsInput.trim() ||
          (parseInt(antiIdleSec, 10) > 0 && antiIdleSend.length > 0) ||
          !autoReconnect ||
          onConnectMacroId !== null)
      ) {
        await updateSavedSession(
          id,
          name.trim(),
          folderId,
          isNet ? host.trim() : null,
          isNet ? port : null,
          protocol === 'ssh' ? username.trim() : null,
          protocol === 'ssh' ? viaSessionId : null,
          buildOptsJson(),
        )
      }
      // Attach the optional login automation script (works for any protocol).
      // Interpret `\n`, `\r`, `\t` and `\\` in the send field so users can write
      // newlines without a multi-line input. Implemented as a small parser
      // (a chained string.replace would have to dodge the order-of-operations
      // trap for the backslash-escape itself).
      const unescape = unescapeControls
      const cleanSteps = loginSteps
        .filter((s) => (s.kind === 'wait' ? s.timeout_ms > 0 : s.expect.trim() || s.send))
        .map((s) => ({ ...s, send: unescape(s.send) }))
      // In edit mode push unconditionally so clearing all steps takes effect.
      if (existing || cleanSteps.length > 0) {
        await setLoginScript(id, cleanSteps)
      }
      // Persist output triggers (drop rows with no pattern). Push
      // unconditionally in edit mode so clearing them takes effect.
      const cleanTriggers = triggers
        .filter((t) => t.pattern.trim())
        .map((t) => ({ ...t, text: unescape(t.text) }))
      if (existing || cleanTriggers.length > 0) {
        await setSessionTriggers(id, cleanTriggers)
      }
      // Persist saved port-forwards (SSH only). Dynamic forwards carry no
      // target; drop empty/incomplete rows. Push unconditionally in SSH so
      // clearing them takes effect.
      if (protocol === 'ssh') {
        const cleanForwards = forwards
          .filter(
            (f) =>
              f.bind_port > 0 &&
              (f.kind === 'dynamic' || (f.target_host?.trim() && (f.target_port ?? 0) > 0)),
          )
          .map((f) => ({
            ...f,
            name: f.name?.trim() || null,
            ...(f.kind === 'dynamic'
              ? { target_host: null, target_port: null }
              : { target_host: f.target_host!.trim() }),
          }))
        if (existing || cleanForwards.length > 0) {
          await setSessionForwards(id, cleanForwards)
        }
      }
      // Persist the command-hint platform for this session. "Use global
      // default" clears a stored one — including one auto-detection wrote,
      // which otherwise could never be undone from here.
      if (platform) {
        await setSessionPlatform(id, platform).catch(console.error)
      } else if (existing) {
        await clearSessionPlatform(id).catch(console.error)
      }
      onCreated(id)
    } catch (e) {
      error = errorText(e)
    } finally {
      saving = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel()
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="New session"
  tabindex="-1"
  {onkeydown}
  transition:fade={{ duration: 140 }}
>
  <form
    class="dialog"
    onsubmit={(e) => {
      e.preventDefault()
      submit()
    }}
    transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}
  >
    <h2>{isEdit ? 'Edit Session' : 'New Session'}</h2>

    <label>
      Protocol
      <select bind:value={protocol} onchange={onProtocolChange} disabled={isEdit}>
        <option value="ssh">SSH</option>
        <option value="telnet">Telnet</option>
        <option value="serial">Serial</option>
        {#if isWindowsHost || protocol === 'wsl'}
          <option value="wsl">WSL</option>
        {/if}
      </select>
    </label>

    <label>
      Name
      <input bind:value={name} placeholder="My router" required />
    </label>

    {#if protocol === 'ssh' || protocol === 'telnet'}
      <div class="row">
        <label class="grow">
          Host
          <input
            bind:value={host}
            placeholder="192.168.1.1"
            required
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
          />
        </label>
        <label class="port">
          Port
          <input type="number" bind:value={port} min={1} max={65535} />
        </label>
      </div>
    {/if}

    {#if protocol === 'ssh'}
      <label>
        Username
        <input
          bind:value={username}
          placeholder="admin"
          required
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
        />
      </label>
      <label>
        Authentication
        <select bind:value={authType}>
          <option value="password">Password</option>
          <option value="key">Private key file</option>
          <option value="keyboard-interactive">Keyboard-interactive (2FA / OTP)</option>
          <option value="agent">SSH agent</option>
        </select>
      </label>
      {#if authType === 'password'}
        {#if vaultEntries.length > 0}
          <label>
            Use a saved credential (vault)
            <select bind:value={vaultCredentialId} onchange={onVaultCredentialChange}>
              <option value={null}>— type a password —</option>
              {#each vaultEntries as v (v.id)}
                <option value={v.id}>{v.name}{v.username ? ` (${v.username})` : ''}</option>
              {/each}
            </select>
          </label>
        {/if}
        {#if vaultCredentialId === null}
          <label>
            Password
            <input
              type="password"
              bind:value={password}
              placeholder={isEdit ? 'leave blank to keep current' : ''}
            />
          </label>
        {/if}
      {:else if authType === 'key'}
        <label>
          Key file
          <span class="row-input">
            <input bind:value={keyPath} placeholder="~/.ssh/id_ed25519" spellcheck="false" />
            <button type="button" class="pick-btn" onclick={pickKeyFile}>Browse…</button>
          </span>
        </label>
        <label>
          Passphrase (if any)
          <input
            type="password"
            bind:value={passphrase}
            placeholder={isEdit ? 'leave blank to keep current' : ''}
          />
        </label>
      {/if}
      {#if isEdit}
        <p class="hint">
          Changing the authentication method or entering a new password / passphrase updates this
          session's credential on save. Leaving the secret blank keeps the current one. Typing a
          password on a session that uses a vault entry gives it its own private secret — the shared
          vault entry is left unchanged.
        </p>
      {/if}

      {#if bastionCandidates.length > 0}
        <label>
          Jump host (ProxyJump)
          <select bind:value={viaSessionId}>
            <option value={null}>— Direct connection —</option>
            {#each bastionCandidates as b (b.id)}
              <option value={b.id}>{b.name}{b.host ? ` (${b.host})` : ''}</option>
            {/each}
          </select>
        </label>
        {#if viaSessionId != null}
          <p class="hint">
            ProxyJump (like <code>ssh -J</code>): ZapTerm connects to the jump host, then tunnels an
            <strong>end-to-end</strong> SSH session to this target through it. The jump host only forwards
            encrypted traffic — it never sees this session's credentials or host key. Chain more hops
            by giving the jump host its own jump host.
          </p>
        {/if}
      {/if}
    {/if}

    {#if protocol === 'serial'}
      <label>
        Device
        {#if availablePorts.length > 0}
          <select bind:value={device}>
            {#each availablePorts as p (p)}
              <option value={p}>{p}</option>
            {/each}
          </select>
        {:else}
          <input bind:value={device} placeholder="/dev/ttyUSB0 or COM3" />
        {/if}
      </label>
      <label>
        Baud Rate
        <select bind:value={baudRate}>
          {#each [9600, 19200, 38400, 57600, 115200] as b (b)}
            <option value={b}>{b}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if protocol === 'wsl'}
      <label>
        Distribution
        {#if wslDistros.length > 0}
          <select bind:value={wslDistro}>
            {#each wslDistros as d (d)}
              <option value={d}>{d}</option>
            {/each}
          </select>
        {:else}
          <input bind:value={wslDistro} placeholder="Ubuntu" autocomplete="off" />
        {/if}
      </label>
      <label>
        User <span class="hint-pill">optional</span>
        <input
          bind:value={wslUser}
          placeholder="the distribution's default user"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>
    {/if}

    {#if folders.length > 0}
      <label>
        Folder
        <select bind:value={folderId}>
          <option value={null}>— None —</option>
          {#each folders as f (f.id)}
            <option value={f.id}>{f.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    <label>
      Tags <span class="hint-pill">comma-separated</span>
      <input bind:value={tagsInput} placeholder="core, prod, datacenter-mad" />
    </label>

    <label>
      Notes
      <textarea
        bind:value={notes}
        rows="2"
        placeholder="Context for this host — console access, owner, caveats…"
      ></textarea>
    </label>

    <label>
      Anti-idle <span class="hint-pill">keepalive</span>
      <div class="anti-idle-row">
        <input
          class="ai-send"
          bind:value={antiIdleSend}
          placeholder={'send (e.g. ! or \\x00)'}
          spellcheck="false"
          autocomplete="off"
        />
        <span class="ai-every">every</span>
        <input class="ai-sec" type="number" min="5" bind:value={antiIdleSec} placeholder="100" />
        <span class="ai-unit">s</span>
      </div>
      <span class="ai-hint"
        >Sends the string on a timer while connected, to beat server idle timeouts. Use <code
          >\x00</code
        > for an invisible keepalive. Leave the interval empty to disable.</span
      >
    </label>

    {#if isNet}
      <label class="checkbox-row">
        <input type="checkbox" bind:checked={autoReconnect} />
        <span>
          Auto-reconnect if the connection drops
          <span class="ai-hint"
            >Manual reconnect always works. Quick-connect and serial sessions never auto-reconnect.</span
          >
        </span>
      </label>
    {/if}

    {#if platforms.length > 0}
      <label>
        Platform <span class="hint-pill">command hints</span>
        <select bind:value={platform}>
          <option value={null}>— Use global default —</option>
          {#each platforms as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if macroSnippets.length > 0}
      <label>
        Run a macro on connect <span class="hint-pill">automation</span>
        <select bind:value={onConnectMacroId}>
          <option value={null}>— None —</option>
          {#each macroSnippets as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if colorSchemes.length > 0}
      <label>
        Color scheme
        <select bind:value={colorScheme}>
          <option value={null}>— Use global default —</option>
          {#each colorSchemes as cs (cs.id)}
            <option value={cs.name}>{cs.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    <details class="login-script" bind:open={showLoginScript}>
      <summary
        >Login automation ({loginSteps.length} step{loginSteps.length === 1 ? '' : 's'})</summary
      >
      <p class="hint">
        Runs on connect, step by step. <strong>expect</strong> waits for a pattern then sends text;
        <strong>send</strong>
        sends immediately; <strong>wait</strong> pauses for N ms.
        <strong>Enter is added automatically</strong>
        — use <code>\r</code> for a bare Enter, <code>\n</code> for extra newlines.
      </p>
      {#each loginSteps as step, idx (idx)}
        <div class="step">
          <span class="step-num">{idx + 1}</span>
          <select class="step-kind" bind:value={step.kind} title="Step type">
            <option value="expect">expect</option>
            <option value="send">send</option>
            <option value="wait">wait</option>
          </select>
          {#if step.kind === 'expect'}
            <input
              class="step-expect"
              placeholder="expect (e.g. Password:)"
              bind:value={step.expect}
              spellcheck="false"
            />
            <input class="step-send" placeholder="send" bind:value={step.send} spellcheck="false" />
            <input
              class="step-timeout"
              type="number"
              min={500}
              step={500}
              bind:value={step.timeout_ms}
              title="timeout (ms)"
            />
            <label class="step-rx" title="Treat 'expect' as a regular expression">
              <input type="checkbox" bind:checked={step.is_regex} />
              .*
            </label>
          {:else if step.kind === 'send'}
            <input
              class="step-expect"
              placeholder={'send now — text + Enter (use \\r for just Enter)'}
              bind:value={step.send}
              spellcheck="false"
            />
          {:else}
            <input
              class="step-timeout step-wait"
              type="number"
              min={100}
              step={100}
              bind:value={step.timeout_ms}
              title="pause (ms)"
            />
            <span class="step-waithint">ms pause</span>
          {/if}
          <button type="button" class="step-rm" onclick={() => removeStep(idx)} title="Remove step"
            ><Icon name="x" size={12} /></button
          >
        </div>
      {/each}
      <button type="button" class="add-step" onclick={addStep}>+ Add step</button>
    </details>

    <details class="login-script" bind:open={showTriggers}>
      <summary>Triggers ({triggers.length})</summary>
      <p class="hint">
        When a <strong>pattern</strong> matches a line of output — or a prompt still waiting for
        input, like <code>(y/n)</code> or <code>--More--</code> — fire an action:
        <strong>notify</strong> (toast), <strong>send</strong> (type the text + Enter back), or
        <strong>bell</strong>. Use <code>\n</code> for a newline in send.
      </p>
      {#each triggers as trig, idx (idx)}
        <div class="step">
          <label class="step-rx" title="Enable this trigger">
            <input type="checkbox" bind:checked={trig.enabled} />
          </label>
          <input
            class="step-expect"
            placeholder="pattern (e.g. %LINK-3)"
            bind:value={trig.pattern}
            spellcheck="false"
          />
          <select class="step-timeout" bind:value={trig.action} title="action">
            <option value="notify">notify</option>
            <option value="send">send</option>
            <option value="bell">bell</option>
          </select>
          <input
            class="step-send"
            placeholder={trig.action === 'send' ? 'text to send' : 'message (optional)'}
            bind:value={trig.text}
            spellcheck="false"
          />
          <label class="step-rx" title="Treat 'pattern' as a regular expression">
            <input type="checkbox" bind:checked={trig.is_regex} />
            .*
          </label>
          <button
            type="button"
            class="step-rm"
            onclick={() => removeTrigger(idx)}
            title="Remove trigger"><Icon name="x" size={12} /></button
          >
        </div>
      {/each}
      <button type="button" class="add-step" onclick={addTrigger}>+ Add trigger</button>
    </details>

    {#if protocol === 'ssh'}
      <details class="login-script" bind:open={showForwards}>
        <summary>Port forwards ({forwards.length})</summary>
        <p class="hint">
          Tunnels opened automatically when this session connects.
          <strong>Local</strong> (-L) binds a local port to a remote target;
          <strong>Dynamic</strong> (-D) is a local SOCKS5 proxy;
          <strong>Remote</strong> (-R) binds a port on the server back to a local target.
        </p>
        {#each forwards as f, idx (idx)}
          <input
            class="fwd-name"
            placeholder="Name (optional) — e.g. FortiGate GUI, RDP to jump host"
            bind:value={f.name}
            maxlength="80"
            aria-label="Forward name"
          />
          <div class="fwd">
            <select class="fwd-kind" bind:value={f.kind} title="Forward type">
              <option value="local">Local -L</option>
              <option value="dynamic">Dynamic -D</option>
              <option value="remote">Remote -R</option>
            </select>
            <input
              class="fwd-bind"
              placeholder="bind (127.0.0.1)"
              bind:value={f.bind_addr}
              spellcheck="false"
            />
            <input
              class="fwd-port"
              type="number"
              min={0}
              max={65535}
              placeholder="port"
              bind:value={f.bind_port}
              title="bind port (0 = auto)"
            />
            {#if f.kind !== 'dynamic'}
              <span class="fwd-arrow" aria-hidden="true">→</span>
              <input
                class="fwd-target"
                placeholder="target host"
                bind:value={f.target_host}
                spellcheck="false"
              />
              <input
                class="fwd-port"
                type="number"
                min={1}
                max={65535}
                placeholder="port"
                bind:value={f.target_port}
                title="target port"
              />
            {:else}
              <span class="fwd-socks">SOCKS5 proxy</span>
            {/if}
            <button
              type="button"
              class="step-rm"
              onclick={() => removeForward(idx)}
              title="Remove forward"><Icon name="x" size={12} /></button
            >
          </div>
        {/each}
        <button type="button" class="add-step" onclick={addForward}>+ Add forward</button>
      </details>
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="actions">
      <button type="button" class="cancel" onclick={onCancel}>Cancel</button>
      <button type="submit" class="save" disabled={saving}>
        {saving ? 'Saving…' : isEdit ? 'Save changes' : 'Save'}
      </button>
    </div>
  </form>
</div>

<style>
  .hint-pill {
    display: inline-block;
    margin-left: 0.4rem;
    padding: 0.05rem 0.4rem;
    background: var(--zx-active-bg);
    color: var(--zx-accent);
    font-size: 0.6rem;
    border-radius: 0.5rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: 600;
    vertical-align: middle;
  }

  .anti-idle-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .anti-idle-row .ai-send {
    flex: 1;
    min-width: 0;
    font-family: var(--zx-font-mono);
  }
  .anti-idle-row .ai-sec {
    width: 4.5rem;
    flex-shrink: 0;
  }
  .ai-every,
  .ai-unit {
    color: var(--zx-text-muted);
    font-size: 0.78rem;
    flex-shrink: 0;
  }
  .ai-hint {
    display: block;
    margin-top: 0.3rem;
    color: var(--zx-text-dim);
    font-size: 0.72rem;
    line-height: 1.45;
  }
  .ai-hint code {
    font-family: var(--zx-font-mono);
    background: var(--zx-active-bg);
    padding: 0 0.25rem;
    border-radius: 0.25rem;
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    /* Keep the dialog inside the window even on short viewports. */
    padding: 2rem 1rem;
    overflow: hidden;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    padding: 1.5rem;
    width: 42rem;
    max-width: 95vw;
    /* Cap to the viewport and scroll the form body when it's taller. */
    max-height: 100%;
    overflow-y: auto;
    /* Two columns so the form is wider and shorter rather than a tall stack.
       Wide items (host/port row, hints, the details panels and the action
       buttons) span both columns. */
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-content: start;
    gap: 0.7rem 1.1rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }

  .dialog > h2,
  .dialog > .row,
  .dialog > .hint,
  .dialog > .error,
  .dialog > details,
  .dialog > .actions {
    grid-column: 1 / -1;
  }

  /* Collapse to a single column when the window is narrow. */
  @media (max-width: 34rem) {
    .dialog {
      grid-template-columns: 1fr;
    }
  }

  .dialog::-webkit-scrollbar {
    width: 8px;
  }
  .dialog::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--zx-text) 22%, transparent);
    border-radius: 4px;
  }

  h2 {
    margin: 0 0 0.25rem;
    font-size: 1rem;
    font-weight: 600;
    color: var(--zx-text);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.8rem;
    color: var(--zx-text-muted);
  }

  .checkbox-row {
    flex-direction: row;
    align-items: flex-start;
    gap: 0.5rem;
    cursor: pointer;
  }
  .checkbox-row input[type='checkbox'] {
    width: auto;
    margin-top: 0.15rem;
    flex-shrink: 0;
    accent-color: var(--zx-accent);
  }
  .checkbox-row .ai-hint {
    margin-top: 0.15rem;
  }

  input,
  select,
  textarea {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-size: 0.85rem;
    padding: 0.35rem 0.5rem;
    outline: none;
    font-family: inherit;
  }

  textarea {
    resize: vertical;
    min-height: 2.2rem;
    line-height: 1.4;
  }

  input:focus-visible,
  select:focus-visible,
  textarea:focus-visible {
    border-color: var(--zx-accent);
    box-shadow: var(--zx-ring);
  }

  .row {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .grow {
    flex: 1;
  }

  .port {
    width: 5rem;
    flex-shrink: 0;
  }

  .error {
    font-size: 0.8rem;
    color: var(--zx-err);
    margin: 0;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
    /* Keep Cancel/Save reachable while the form body scrolls. */
    position: sticky;
    bottom: -1.5rem;
    margin: 0.25rem -1.5rem -1.5rem;
    padding: 0.85rem 1.5rem;
    background: var(--zx-surface);
    border-top: 1px solid var(--zx-border);
  }

  button {
    font-size: 0.82rem;
    font-family: inherit;
    padding: 0.35rem 0.9rem;
    border-radius: 0.25rem;
    border: none;
    cursor: pointer;
  }

  .cancel {
    background: var(--zx-surface-2);
    color: var(--zx-text-muted);
  }

  .cancel:hover {
    background: var(--zx-hover-bg);
  }

  .save {
    background: var(--zx-accent);
    color: var(--zx-on-accent);
  }

  .save:hover:not(:disabled) {
    background: color-mix(in srgb, var(--zx-accent) 85%, black);
  }

  .save:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .row-input {
    display: flex;
    gap: 0.4rem;
  }

  .row-input input {
    flex: 1;
  }

  .pick-btn {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.25rem;
    color: var(--zx-text);
    font-family: inherit;
    font-size: 0.78rem;
    padding: 0.35rem 0.7rem;
    cursor: pointer;
    flex-shrink: 0;
  }

  .pick-btn:hover {
    background: var(--zx-hover-bg);
  }

  /* Login automation editor */
  .login-script {
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.5rem 0.7rem;
  }

  .login-script summary {
    cursor: pointer;
    font-size: 0.82rem;
    color: var(--zx-text-muted);
    user-select: none;
  }

  .hint {
    font-size: 0.74rem;
    color: var(--zx-text-muted);
    margin: 0;
    line-height: 1.4;
    padding: 0.25rem 0.45rem;
    background: color-mix(in srgb, var(--zx-warn) 8%, transparent);
    border-left: 2px solid color-mix(in srgb, var(--zx-warn) 40%, transparent);
    border-radius: 0.2rem;
  }

  .login-script .hint {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    margin: 0.4rem 0 0.55rem;
    line-height: 1.5;
  }

  .login-script .hint code {
    background: color-mix(in srgb, var(--zx-text) 6%, transparent);
    padding: 0 0.2rem;
    border-radius: 0.2rem;
    color: var(--zx-text);
  }

  .step {
    display: flex;
    gap: 0.3rem;
    align-items: center;
    margin-bottom: 0.3rem;
  }

  .step-num {
    width: 1.3rem;
    text-align: center;
    color: var(--zx-text-dim);
    font-size: 0.7rem;
    flex-shrink: 0;
  }

  .step-expect,
  .step-send {
    flex: 1;
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
  }

  .step-timeout {
    width: 4.5rem;
    font-size: 0.78rem;
    flex-shrink: 0;
  }

  .step-kind {
    width: 5rem;
    font-size: 0.72rem;
    flex-shrink: 0;
    font-family: var(--zx-font-mono);
  }

  .step-waithint {
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    flex-shrink: 0;
  }

  .step-rx {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    color: var(--zx-text-muted);
    flex-shrink: 0;
    cursor: pointer;
  }
  .step-rx input {
    accent-color: var(--zx-accent);
  }

  .step-rm {
    background: transparent;
    color: var(--zx-text-dim);
    border: none;
    cursor: pointer;
    font-size: 0.85rem;
    padding: 0 0.35rem;
    border-radius: 0.2rem;
  }

  .step-rm:hover {
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 15%, transparent);
  }

  .add-step {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px dashed var(--zx-border);
    border-radius: 0.25rem;
    padding: 0.3rem 0.7rem;
    font-size: 0.78rem;
    cursor: pointer;
    font-family: inherit;
    margin-top: 0.3rem;
  }

  .add-step:hover {
    color: var(--zx-text);
    border-color: var(--zx-accent);
  }

  .fwd {
    display: flex;
    gap: 0.3rem;
    align-items: center;
    margin-bottom: 0.6rem;
  }
  .fwd-name {
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 0.2rem;
    font-size: 0.78rem;
  }
  .fwd-kind {
    width: 6.5rem;
    font-size: 0.76rem;
    flex-shrink: 0;
  }
  .fwd-bind,
  .fwd-target {
    flex: 1;
    min-width: 0;
    font-family: var(--zx-font-mono);
    font-size: 0.78rem;
  }
  .fwd-port {
    width: 4.8rem;
    font-size: 0.78rem;
    flex-shrink: 0;
    font-family: var(--zx-font-mono);
  }
  .fwd-arrow {
    color: var(--zx-text-dim);
    flex-shrink: 0;
  }
  .fwd-socks {
    flex: 1;
    font-size: 0.74rem;
    color: var(--zx-text-dim);
    font-style: italic;
  }
</style>
