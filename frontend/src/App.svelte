<script lang="ts">
  import { errorText } from '$lib/bridge/errors'
  import { onMount } from 'svelte'
  import { getTheme, applyTheme } from '$lib/themes/store.svelte'
  import { themes, themeLabels, colorSchemeFor } from '$lib/themes/index'
  import MarkTile from '$lib/icons/MarkTile.svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import TitleBar from '$lib/pylon/TitleBar.svelte'
  import Sidebar from '$lib/pylon/Sidebar.svelte'
  import TabBar from '$lib/pylon/TabBar.svelte'
  import type { TabEntry } from '$lib/pylon/TabBar.svelte'
  import type { PaneData } from '$lib/pylon/Pane.svelte'
  import SplitTree from '$lib/pylon/SplitTree.svelte'
  import GridView from '$lib/pylon/GridView.svelte'
  import MasterInputBar from '$lib/pylon/MasterInputBar.svelte'
  import ComparisonPanel from '$lib/orchestrator/ComparisonPanel.svelte'
  import { CommandRunner, type RunnerHost } from '$lib/orchestrator/commandRunner.svelte'
  import CommandListDialog from '$lib/orchestrator/CommandListDialog.svelte'
  import TunnelsManagerDialog from '$lib/terminal/TunnelsManagerDialog.svelte'
  import {
    type PaneTreeNode,
    type SplitDirection,
    eachLeaf,
    firstLeafId,
    leaf as leafNode,
    leafCount,
    removeLeaf,
    setRatio,
    splitLeaf,
  } from '$lib/pylon/paneTree'
  import { SvelteMap } from 'svelte/reactivity'
  import StatusBar from '$lib/pylon/StatusBar.svelte'
  import NewSessionDialog from '$lib/sessions/NewSessionDialog.svelte'
  import QuickConnectDialog from '$lib/sessions/QuickConnectDialog.svelte'
  import type { ConnectParams } from '$lib/sessions/QuickConnectDialog.svelte'
  import ResizeHandles from '$lib/pylon/ResizeHandles.svelte'
  import SettingsModal from '$lib/settings/SettingsModal.svelte'
  import KeyboardInteractiveDialog from '$lib/terminal/KeyboardInteractiveDialog.svelte'
  import SnippetsDialog from '$lib/sessions/SnippetsDialog.svelte'
  import ScriptPanel from '$lib/scripting/ScriptPanel.svelte'
  import ScriptStartDialog from '$lib/scripting/ScriptStartDialog.svelte'
  import {
    hostBusy,
    initScriptHost,
    scriptRun,
    startNotebookFromSettings,
    startScript,
  } from '$lib/scripting/scriptHost.svelte'
  import CommandPalette from '$lib/palette/CommandPalette.svelte'
  import type { PaletteItem } from '$lib/palette/CommandPalette.svelte'
  import Toasts from '$lib/ui/Toasts.svelte'
  import VariablesDialog from '$lib/snippets/VariablesDialog.svelte'
  import { resolveSnippet } from '$lib/snippets/variables.svelte'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import PromptDialog from '$lib/ui/PromptDialog.svelte'
  import {
    ask,
    message,
    open as openFileDialog,
    save as saveFileDialog,
  } from '@tauri-apps/plugin-dialog'
  import {
    exportSessions,
    importSessions,
    importSshConfig,
    importPutty,
    importMobaXterm,
    importSecureCrt,
  } from '$lib/bridge/commands'
  import type { ImportSummary } from '$lib/bridge/commands'
  import { loadHintsSettings } from '$lib/hints/store.svelte'
  import { loadUserLists } from '$lib/cmdlists/userLists.svelte'
  import { broadcast, sessionRuntime, paneToSession } from '$lib/stores/sessionRuntime.svelte'
  import { localShells, platformForShell } from '$lib/stores/localShell.svelte'
  import { loadBindings, matchAction, type ShortcutAction } from '$lib/stores/keybindings.svelte'
  import {
    listSessions,
    listFolders,
    moveSavedSession,
    reorderSavedSession,
    deleteSavedSession,
    cloneSavedSession,
    createFolder,
    renameFolder,
    deleteFolder,
    sendInputText,
    openLogsDir,
  } from '$lib/bridge/commands'
  import {
    getFocusedSessionId,
    broadcastTargets,
    focusSession,
    claimBroadcastScope,
    releaseBroadcastScope,
  } from '$lib/stores/sessionRuntime.svelte'
  import {
    visibleSnippets,
    snippets,
    loadSnippets,
    loadVisibleSnippets,
    setBarContext,
    matchSnippetShortcut,
  } from '$lib/stores/snippets.svelte'
  import { getSessionPlatform, getSessionTcpMss, openExternal } from '$lib/bridge/commands'
  import {
    createSnippet,
    updateSnippet,
    deleteSnippet,
    setSnippetSteps,
    setSnippetFolder,
    setSnippetShortcut,
    scriptExamplesDir,
    setSnippetPositions,
    exportMacros,
    importMacros,
  } from '$lib/bridge/commands'
  import CommandListBar from '$lib/pylon/CommandListBar.svelte'
  import MacroDialog from '$lib/pylon/MacroDialog.svelte'
  import MacroImportDialog from '$lib/pylon/MacroImportDialog.svelte'
  import VaultDialog from '$lib/sessions/VaultDialog.svelte'
  import { runMacroOnFocused } from '$lib/orchestrator/macroRunner'
  import { getVersion } from '@tauri-apps/api/app'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { SYNC_EVENT, type SyncStatus } from '$lib/bridge/commands'
  import { handleSyncStatus } from '$lib/backup/sync'
  import {
    loadSettings,
    uiSettings,
    clampSidebarWidth,
    applySidebarWidth,
    DEFAULT_SIDEBAR_WIDTH,
  } from '$lib/stores/settings.svelte'
  import type { SavedSession, Folder, BroadcastGroup, Snippet, LoginStep } from '$lib/bridge/types'
  import { groups, loadGroups } from '$lib/stores/groups.svelte'
  import GroupsDialog from '$lib/sessions/GroupsDialog.svelte'

  // ── types ───────────────────────────────────────────────────────────────────

  type PaneStatus = 'connecting' | 'connected' | 'error' | 'lost' | 'closed'

  /**
   * A tab is either a split-pane tree (the default, recursive binary splits)
   * or a flat grid of panes (multi-host orchestration). The grid carries its
   * own ordered pane list + column count and an optional source group id; it
   * is rendered by GridView and is NOT splittable.
   */
  type TabLayout =
    | { kind: 'split'; root: PaneTreeNode }
    | { kind: 'grid'; panes: PaneData[]; cols: number; groupId: number | null }

  interface AppTab {
    id: number
    layout: TabLayout
    /** Pane status keyed by paneId. Reactive map for fine-grained updates. */
    statuses: SvelteMap<number, PaneStatus>
    /** Stable label/colour for the tab (mirrors the first leaf at creation). */
    label: string
    color: string
  }

  /** All panes in a tab, regardless of layout. */
  function tabPanes(tab: AppTab): PaneData[] {
    if (tab.layout.kind === 'grid') return tab.layout.panes
    const out: PaneData[] = []
    eachLeaf(tab.layout.root, (p) => out.push(p))
    return out
  }

  /** First pane id in a tab (for focus + tab-strip summary). */
  function firstPaneId(tab: AppTab): number {
    if (tab.layout.kind === 'grid') return tab.layout.panes[0]?.id ?? -1
    return firstLeafId(tab.layout.root)
  }

  // ── theme ────────────────────────────────────────────────────────────────────

  const theme = $derived(getTheme())

  /** Reveal the session-log folder in the file explorer (sidebar + palette). */
  async function openLogsFolder() {
    try {
      const path = await openLogsDir()
      showToast({ kind: 'info', title: 'Logs', detail: path })
    } catch (e) {
      showToast({ kind: 'error', title: 'Logs', detail: errorText(e) })
    }
  }

  /// Cycle through the theme registry in declaration order.
  function toggleTheme() {
    const keys = Object.keys(themes)
    const idx = keys.indexOf(theme.name)
    applyTheme(keys[(idx + 1) % keys.length] ?? 'parchment')
  }

  // Publish the active theme as global `--zx-*` CSS custom properties on the
  // document root. This lets fixed-position overlays (dialogs, toasts, the
  // command palette) — which render outside the prop-threaded layout tree —
  // pick up theme colors via plain `var(--zx-…)` in their own styles, with
  // no prop plumbing. Re-runs whenever `theme` changes because it reads the
  // derived value. Fallback values live in app.css so first paint never flashes.
  $effect(() => {
    const root = document.documentElement.style
    const set = (k: string, v: string) => root.setProperty(k, v)
    set('--zx-app-bg', theme.appBg)
    set('--zx-body-bg', theme.bodyBg)
    set('--zx-surface', theme.sidebarBg)
    set('--zx-surface-2', theme.titlebarBg)
    set('--zx-text', theme.textPrimary)
    set('--zx-text-muted', theme.textMuted)
    set('--zx-text-dim', theme.textDim)
    set('--zx-accent', theme.accent)
    set('--zx-accent-2', theme.accent2)
    set('--zx-on-accent', theme.onAccent)
    set('--zx-border', theme.border)
    set('--zx-radius', theme.radius)
    set('--zx-hover-bg', theme.itemHoverBg)
    set('--zx-active-bg', theme.itemActiveBg)
    set('--zx-active-border', theme.itemActiveBorder)
    set('--zx-ok', theme.ok)
    set('--zx-warn', theme.warn)
    set('--zx-err', theme.err)
    set('--zx-font-ui', theme.fontUi)
    set('--zx-font-mono', theme.fontMono)
    set('--zx-shadow', theme.windowShadow)
    set('--zx-term-bg', theme.terminal.bg)
    set('--zx-term-fg', theme.terminal.fg)
    set('--zx-term-dim', theme.terminal.dim)
    set('--zx-term-ok', theme.terminal.ok)
    set('--zx-term-warn', theme.terminal.warn)
    set('--zx-term-err', theme.terminal.err)
    root.colorScheme = colorSchemeFor(theme.appBg)
  })

  // ── state ────────────────────────────────────────────────────────────────────

  let nextId = 1

  /// Tab / pane colours: vivid hues that read on light and dark themes.
  const TAB_PALETTE = [
    '#22d3ee', // cyan
    '#f472b6', // pink
    '#a78bfa', // violet
    '#f59e0b', // amber
    '#ef4444', // red
    '#10b981', // emerald
    '#f97316', // orange
    '#84cc16', // lime
    '#3b82f6', // blue
    '#d946ef', // fuchsia
    '#14b8a6', // teal
    '#eab308', // yellow
  ]

  /// A random colour for a new tab or pane, preferring one no open tab
  /// already shows so neighbouring tabs stay easy to tell apart. (The
  /// right-click menu on a tab still sets a colour by hand.)
  function pickColor(alsoAvoid: string[] = []): string {
    const used = new Set([...tabs.map((t) => t.color), ...alsoAvoid])
    const free = TAB_PALETTE.filter((c) => !used.has(c))
    const pool = free.length > 0 ? free : TAB_PALETTE
    return pool[Math.floor(Math.random() * pool.length)] as string
  }

  function mkPane(
    label: string,
    opts?: Partial<Omit<PaneData, 'id' | 'label' | 'color'>>,
  ): PaneData {
    const id = nextId++
    return { id, label, color: pickColor(), ...opts }
  }

  function mkTab(pane: PaneData): AppTab {
    return {
      id: nextId++,
      layout: { kind: 'split', root: leafNode(pane) },
      statuses: new SvelteMap<number, PaneStatus>([[pane.id, 'connecting']]),
      label: pane.label,
      color: pane.color,
    }
  }

  function mkGridTab(panes: PaneData[], label: string, groupId: number | null): AppTab {
    const cols = Math.max(1, Math.ceil(Math.sqrt(panes.length)))
    const statuses = new SvelteMap<number, PaneStatus>()
    for (const p of panes) statuses.set(p.id, 'connecting')
    return {
      id: nextId++,
      layout: { kind: 'grid', panes, cols, groupId },
      statuses,
      label,
      color: panes[0]?.color ?? pickColor(),
    }
  }

  // Declared first: pickColor() looks at the open tabs.
  let tabs = $state<AppTab[]>([])
  const firstPane = mkPane('shell')
  const firstTab = mkTab(firstPane)
  tabs = [firstTab]
  let activeTabId = $state(firstTab.id)
  let focusedPaneId = $state(firstPane.id)
  let sessions = $state<SavedSession[]>([])
  let folders = $state<Folder[]>([])
  let showNewSession = $state(false)
  let showQuickConnect = $state(false)
  let showSettings = $state(false)
  let showSnippets = $state(false)
  let showGroups = $state(false)
  let showCommandList = $state(false)
  let showTunnelsManager = $state(false)

  /// Live session UUID → human label (pane name), for the tunnels manager.
  const sessionLabels = $derived<Record<string, string>>(
    Object.fromEntries(
      tabs.flatMap((t) =>
        tabPanes(t)
          .map((p) => [paneToSession.get(p.id), p.label] as const)
          .filter((e): e is [string, string] => typeof e[0] === 'string'),
      ),
    ),
  )
  let showAbout = $state(false)
  // Bundle version from tauri.conf.json — single source of truth.
  let appVersion = $state('')
  getVersion()
    .then((v) => (appVersion = v))
    .catch(() => {})
  let showPalette = $state(false)
  // Lightweight text-prompt dialog (replaces window.prompt which Tauri blocks).
  let prompt = $state<{
    title: string
    initial?: string
    placeholder?: string
    submitLabel?: string
    onSubmit: (v: string) => void
  } | null>(null)
  function openPrompt(p: NonNullable<typeof prompt>) {
    prompt = p
  }

  // Live throughput stats keyed by runtime session UUID, refreshed every
  // second from the backend's session-stats event.
  let bytesPerSecBySession = $state<Record<string, number>>({})
  /// When set, opens NewSessionDialog in edit mode with the given session pre-filled.
  let editingSession = $state<SavedSession | null>(null)
  let multiOn = $state(false)

  // Macro library (sidebar Macros section): macros are snippets carrying steps.
  let editingMacro = $state<Snippet | 'new' | null>(null)
  let showMacroImport = $state(false)
  let showVault = $state(false)
  const macros = $derived(snippets.filter((s) => s.steps_json))
  // Distinct, sorted folder names across macros — autocomplete in the dialog.
  const macroFolders = $derived(
    [...new Set(macros.map((m) => m.folder).filter((f): f is string => !!f))].sort(),
  )

  async function refreshSnippets() {
    await Promise.all([loadSnippets(), loadVisibleSnippets()])
  }

  async function saveMacro(v: {
    name: string
    color: string | null
    folder: string | null
    steps: LoginStep[]
    shortcut: string | null
  }) {
    try {
      const stepsJson = JSON.stringify(v.steps)
      let id: number
      if (editingMacro && editingMacro !== 'new') {
        // Preserve the macro's existing content/platform; only name/color change here.
        await updateSnippet(
          editingMacro.id,
          v.name,
          editingMacro.content,
          editingMacro.platform,
          v.color,
        )
        id = editingMacro.id
      } else {
        id = await createSnippet(v.name, '', null, v.color)
      }
      await setSnippetSteps(id, stepsJson)
      await setSnippetFolder(id, v.folder)
      await setSnippetShortcut(id, v.shortcut)
      editingMacro = null
      await refreshSnippets()
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Save macro failed',
        detail: errorText(e),
      })
    }
  }

  async function importMacro(macros: { name: string; steps: LoginStep[] }[]) {
    try {
      let totalSteps = 0
      for (const m of macros) {
        const id = await createSnippet(m.name, '', null, null)
        await setSnippetSteps(id, JSON.stringify(m.steps))
        totalSteps += m.steps.length
      }
      showMacroImport = false
      await refreshSnippets()
      const label =
        macros.length === 1
          ? `${macros[0]!.name} (${totalSteps} steps)`
          : `${macros.length} macros (${totalSteps} steps)`
      showToast({ kind: 'success', title: 'Macro imported', detail: label })
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Import failed',
        detail: errorText(e),
      })
    }
  }

  async function deleteMacro() {
    if (!editingMacro || editingMacro === 'new') return
    const name = editingMacro.name
    try {
      await deleteSnippet(editingMacro.id)
      editingMacro = null
      await refreshSnippets()
      showToast({ kind: 'info', title: 'Macro deleted', detail: name })
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Delete macro failed',
        detail: errorText(e),
      })
    }
  }

  /// Duplicate a macro: a fresh snippet named "<name> (copy)" with the same
  /// content/platform/color, then copy its steps + folder. Mirrors the
  /// session-clone toast pattern.
  async function cloneMacro(src: Snippet) {
    try {
      const id = await createSnippet(`${src.name} (copy)`, src.content, src.platform, src.color)
      if (src.steps_json) await setSnippetSteps(id, src.steps_json)
      if (src.folder) await setSnippetFolder(id, src.folder)
      await refreshSnippets()
      showToast({ kind: 'success', title: 'Macro duplicated', detail: `${src.name} (copy)` })
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Duplicate failed',
        detail: errorText(e),
      })
    }
  }

  /// Move a macro to a folder (`null` = ungrouped) from the sidebar Macros zone.
  async function setMacroFolder(m: Snippet, folder: string | null) {
    try {
      await setSnippetFolder(m.id, folder)
      await refreshSnippets()
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Move macro failed',
        detail: errorText(e),
      })
    }
  }

  /// Persist a new order for a macro folder group (ids in their new order).
  async function reorderMacros(orderedIds: number[]) {
    try {
      await setSnippetPositions(orderedIds)
      await refreshSnippets()
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Reorder macros failed',
        detail: errorText(e),
      })
    }
  }

  /// Export macros to a JSON file. `ids = null` exports all macros.
  async function exportMacrosToFile(ids: number[] | null, label: string) {
    try {
      const path = await saveFileDialog({
        defaultPath: 'zapterm-macros.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      })
      if (!path) return
      await exportMacros(path, ids)
      showToast({ kind: 'success', title: 'Macros exported', detail: label })
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Export failed',
        detail: errorText(e),
      })
    }
  }

  /// Import macros from a JSON file (skip-by-name idempotent).
  async function importMacrosFromFile() {
    try {
      const path = await openFileDialog({
        multiple: false,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      })
      if (!path || typeof path !== 'string') return
      const summary = await importMacros(path)
      await refreshSnippets()
      showToast({
        kind: 'success',
        title: 'Macros imported',
        detail: `${summary.added} added, ${summary.updated} updated`,
      })
    } catch (e) {
      showToast({
        kind: 'error',
        title: 'Import failed',
        detail: errorText(e),
      })
    }
  }

  // Mirror focusedPaneId + multi-exec toggle into the shared runtime store so
  // snippets / broadcast can act on the currently focused session.
  $effect(() => {
    sessionRuntime.focusedPaneId = focusedPaneId
  })
  $effect(() => {
    broadcast.enabled = multiOn
  })

  // Tell the bottom bar which session is focused, so it shows that vendor's
  // command lists and that session's recent commands. Effects
  // re-run when `focusedPaneId` flips; we resolve the saved-session id
  // inside so a folder/local tab cleanly resets to "no platform".
  $effect(() => {
    void focusedPaneId // explicit dep
    const pane = focusedPaneData
    const sid = pane?.savedSession?.id ?? null
    if (sid != null) {
      getSessionPlatform(sid)
        .then((p) => setBarContext(p, sid))
        .catch(() => setBarContext(null, sid))
    } else if (pane && !pane.ssh && !pane.telnet) {
      // Local shell tab (no saved row, not SSH/Telnet): give the bar the
      // lists for the shell that actually started — PowerShell/cmd →
      // Windows; bash/zsh/fish → macOS on a Mac, Linux elsewhere. Before the
      // shell is known, the host OS decides. This is also what makes a local
      // session count as "recognised" for hints.
      const runtimeId = paneToSession.get(pane.id)
      const localPlatform = platformForShell(runtimeId ? localShells[runtimeId] : null)
      setBarContext(localPlatform, null).catch(() => {})
    } else {
      setBarContext(null, null).catch(() => {})
    }
  })

  // ── derived ──────────────────────────────────────────────────────────────────

  const activeTab = $derived(tabs.find((t) => t.id === activeTabId))

  /// Returns the first leaf's status as a coarse summary for the tab strip.
  function summaryStatus(tab: AppTab): PaneStatus {
    const id = firstPaneId(tab)
    return tab.statuses.get(id) ?? 'connecting'
  }

  /// Tab-strip label. A single-pane tab keeps its creation name; once split
  /// into multiple sessions it reflects them ("A · B", or "A +2" for more), so
  /// the strip stops showing only the first session's name.
  function tabDisplayLabel(t: AppTab): string {
    const names = tabPanes(t)
      .filter((p) => !p.empty)
      .map((p) => p.label)
    if (names.length <= 1) return names[0] ?? t.label
    if (names.length === 2) return `${names[0]} · ${names[1]}`
    return `${names[0]} +${names.length - 1}`
  }

  const tabEntries = $derived<TabEntry[]>(
    tabs.map((t) => ({
      id: t.id,
      label: tabDisplayLabel(t),
      color: t.color,
      status: summaryStatus(t),
    })),
  )

  /// Find a pane by id in the active tab (split tree or grid).
  function findActiveLeaf(paneId: number): PaneData | null {
    if (!activeTab) return null
    return tabPanes(activeTab).find((p) => p.id === paneId) ?? null
  }

  const focusedPaneData = $derived<PaneData | null>(findActiveLeaf(focusedPaneId))

  const focusedStatus = $derived<PaneStatus>(activeTab?.statuses.get(focusedPaneId) ?? 'connecting')

  /** Aggregated live status per saved-session id, across every open pane in
   *  every tab — drives the status dot on the sidebar avatars. A session with
   *  no open pane (or only `closed` ones) is absent from the map and shows no
   *  dot (idle / not opened). When a session has several panes open, the most
   *  reassuring status wins: connected > connecting > error. */
  const sessionStatuses = $derived.by<Map<number, 'connecting' | 'connected' | 'error'>>(() => {
    const m = new Map<number, 'connecting' | 'connected' | 'error'>()
    const rank = { error: 1, connecting: 2, connected: 3 } as const
    for (const t of tabs) {
      for (const p of tabPanes(t)) {
        const sid = p.savedSession?.id
        if (sid == null) continue
        const raw = t.statuses.get(p.id)
        // A lost link shows as an error dot, like a failed connect.
        const st = raw === 'lost' ? 'error' : raw
        if (st !== 'connecting' && st !== 'connected' && st !== 'error') continue
        const cur = m.get(sid)
        if (!cur || rank[st] > rank[cur]) m.set(sid, st)
      }
    }
    return m
  })

  const splitOn = $derived(
    !!activeTab && activeTab.layout.kind === 'split' && leafCount(activeTab.layout.root) > 1,
  )

  /// Live session UUIDs of the active tab's panes — feeds the master input
  /// bar on regular (split) tabs when multi-exec is on. Grid tabs render
  /// their own bar inside GridView.
  const activeTabSessionIds = $derived(
    activeTab
      ? tabPanes(activeTab)
          .map((p) => paneToSession.get(p.id))
          .filter((id): id is string => typeof id === 'string')
      : [],
  )

  // MultiExec on a regular (single/split) tab reaches only that tab's
  // sessions, like its master bar — never sessions in hidden tabs. A grid tab
  // claims its own scope (GridView); the owner token keeps the two from
  // clearing each other whatever order their effects run in.
  const tabScopeOwner = {}
  $effect(() => {
    if (!activeTab || activeTab.layout.kind === 'grid') return
    claimBroadcastScope(tabScopeOwner, new Set(activeTabSessionIds))
    return () => releaseBroadcastScope(tabScopeOwner)
  })

  // ── Run & compare on a regular (split) tab ──────────────────────────────────
  // Mirrors GridView's runner so "Compare" works outside broadcast-group grids:
  // send one command to every live pane of the active tab and diff the outputs.
  const compareRunner = new CommandRunner()
  let showComparePanel = $state(false)

  async function runCompareActiveTab(command: string) {
    if (!activeTab) return
    const hosts: RunnerHost[] = []
    for (const pane of tabPanes(activeTab)) {
      const sid = paneToSession.get(pane.id)
      if (!sid) continue
      const savedId = pane.savedSession?.id
      const platform = savedId != null ? await getSessionPlatform(savedId).catch(() => null) : null
      hosts.push({ sessionId: sid, label: pane.label, platform })
    }
    showComparePanel = true
    await compareRunner.run(hosts, command)
  }

  /// Every live session across all tabs, labelled — target list for the
  /// command-list dialog.
  const liveTargets = $derived(
    tabs.flatMap((t) =>
      tabPanes(t)
        .map((p) => ({
          sessionId: paneToSession.get(p.id),
          label: p.label,
          tabLabel: t.label,
          active: t.id === activeTabId,
        }))
        .filter(
          (x): x is { sessionId: string; label: string; tabLabel: string; active: boolean } =>
            typeof x.sessionId === 'string',
        ),
    ),
  )
  const canClosePanes = $derived(splitOn)

  const statusHost = $derived(
    focusedPaneData?.ssh?.host ?? focusedPaneData?.savedSession?.host ?? '',
  )

  const statusPort = $derived(
    focusedPaneData?.ssh?.port ?? focusedPaneData?.savedSession?.port ?? undefined,
  )

  const statusProtocol = $derived(
    focusedPaneData?.savedSession?.protocol?.toUpperCase() ??
      (focusedPaneData?.ssh ? 'SSH' : focusedPaneData?.telnet ? 'TELNET' : 'LOCAL'),
  )

  const activeSessionName = $derived(activeTab?.label ?? '')

  // ── data loading ─────────────────────────────────────────────────────────────

  async function load() {
    // Each call independent so a single failure doesn't blank the whole tree.
    listSessions()
      .then((v) => (sessions = v))
      .catch((e) => console.error('listSessions', e))
    listFolders()
      .then((v) => (folders = v))
      .catch((e) => console.error('listFolders', e))
    loadSnippets().catch((e) => console.error('loadSnippets', e))
    loadGroups().catch((e) => console.error('loadGroups', e))
  }

  $effect(() => {
    load()
    loadSettings()
    loadBindings()
    loadHintsSettings()
    void loadUserLists()
  })

  // ── tab management ────────────────────────────────────────────────────────────

  function openSavedSessionTab(s: SavedSession, skipOnConnectMacro = false): number {
    // If the focused pane is an empty split placeholder, open the session INTO
    // it (filling the pane the user just split) instead of a new tab.
    const activeT = tabs.find((t) => t.id === activeTabId)
    if (activeT && activeT.layout.kind === 'split') {
      const target = tabPanes(activeT).find((p) => p.id === focusedPaneId)
      if (target?.empty) {
        target.savedSession = s
        target.label = s.name
        target.color = pickColor()
        target.ssh = undefined
        target.telnet = undefined
        target.empty = false
        target.skipOnConnectMacro = skipOnConnectMacro
        activeT.statuses.set(target.id, 'connecting')
        return target.id
      }
    }
    const pane = mkPane(s.name, { savedSession: s, skipOnConnectMacro } as Partial<PaneData>)
    const tab = mkTab(pane)
    tabs = [...tabs, tab]
    activeTabId = tab.id
    focusedPaneId = pane.id
    return pane.id
  }

  // ── Python scripts ──────────────────────────────────────────────────────────
  /// Script chosen in the file dialog, awaiting the Run / Dry run confirmation.
  let scriptPending = $state<string | null>(null)
  /// Saved-session id → stored platform, filled lazily for the script host
  /// (which needs a synchronous answer; live detection also feeds it).
  const savedPlatformCache = new Map<number, string | null>()

  async function chooseScript() {
    if (hostBusy()) {
      scriptRun.visible = true
      await message(
        scriptRun.mode === 'notebook'
          ? 'Turn off the notebook connection first (Turn off, in the panel).'
          : 'A script is already running — wait for it or stop it.',
        { title: 'Run Python script', kind: 'info' },
      )
      return
    }
    // Start in the examples folder (created on first use) so there is always
    // something ready to dry-run and edit.
    const examples = await scriptExamplesDir(false).catch(() => undefined)
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      defaultPath: examples,
      filters: [{ name: 'Python script', extensions: ['py'] }],
    })
    if (typeof picked === 'string') scriptPending = picked
  }

  /// Tools → Allow notebook connection: turn it on, or show the panel if it
  /// already is.
  async function openNotebookConnection() {
    if (scriptRun.mode === 'notebook' && hostBusy()) {
      scriptRun.visible = true
      return
    }
    if (hostBusy()) {
      scriptRun.visible = true
      await message('A script is running — wait for it or stop it first.', {
        title: 'Notebook connection',
        kind: 'info',
      })
      return
    }
    try {
      await startNotebookFromSettings()
    } catch (e) {
      // e.g. a script was started while the settings were being read.
      scriptRun.visible = true
      await message(String(e instanceof Error ? e.message : e), {
        title: 'Notebook connection',
        kind: 'info',
      })
    }
  }

  /// Pane id + tab of the pane whose live session is `sessionId`.
  function paneOfSession(sessionId: string): { paneId: number; tab: AppTab } | null {
    for (const [paneId, sid] of paneToSession) {
      if (sid !== sessionId) continue
      const tab = tabs.find((t) => tabPanes(t).some((p) => p.id === paneId))
      if (tab) return { paneId, tab }
    }
    return null
  }

  function describeSession(sessionId: string) {
    const found = paneOfSession(sessionId)
    if (!found) return null
    const pane = tabPanes(found.tab).find((p) => p.id === found.paneId)
    if (!pane) return null
    let platform: string | null = null
    const saved = pane.savedSession
    if (saved) {
      if (savedPlatformCache.has(saved.id)) {
        platform = savedPlatformCache.get(saved.id) ?? null
      } else {
        savedPlatformCache.set(saved.id, null)
        getSessionPlatform(saved.id)
          .then((p) => savedPlatformCache.set(saved.id, p))
          .catch(() => {})
      }
    } else if (!pane.ssh && !pane.telnet) {
      platform = 'linux' // local shell tab
    }
    return { id: sessionId, name: pane.label, platform }
  }

  /// Re-read the stored platform of the saved session behind `sessionId`, so a
  /// script sees the one set in the session dialog (or detected) right now,
  /// not a stale or not-yet-loaded cache entry.
  async function refreshSessionPlatform(sessionId: string) {
    const found = paneOfSession(sessionId)
    const saved = found
      ? tabPanes(found.tab).find((p) => p.id === found.paneId)?.savedSession
      : undefined
    if (!saved) return
    const p = await getSessionPlatform(saved.id).catch(() => null)
    savedPlatformCache.set(saved.id, p)
  }

  /// Saved sessions under `folder` (by name, including sub-folders), or all.
  function savedSessionNames(folder: string | null): string[] {
    if (!folder) return sessions.map((s) => s.name)
    const roots = folders.filter((f) => f.name === folder).map((f) => f.id)
    const inTree = new Set<number>(roots)
    let grew = true
    while (grew) {
      grew = false
      for (const f of folders) {
        if (f.parent_id != null && inTree.has(f.parent_id) && !inTree.has(f.id)) {
          inTree.add(f.id)
          grew = true
        }
      }
    }
    return sessions.filter((s) => s.folder_id != null && inTree.has(s.folder_id)).map((s) => s.name)
  }

  function addLocalTab() {
    const pane = mkPane('shell')
    const tab = mkTab(pane)
    tabs = [...tabs, tab]
    activeTabId = tab.id
    focusedPaneId = pane.id
  }

  /// Open every saved session in `group` as a tiled grid in a single new tab,
  /// driven by the master input bar. Missing members (deleted sessions) are
  /// skipped. Each pane connects independently via the existing path.
  function openGroupAsGrid(group: BroadcastGroup) {
    const members = group.session_ids
      .map((sid) => sessions.find((s) => s.id === sid))
      .filter((s): s is SavedSession => !!s)
    if (members.length === 0) {
      showToast({ kind: 'warning', title: group.name, detail: 'This group has no sessions.' })
      return
    }
    // Distinct colours inside the grid too.
    const taken: string[] = []
    const panes = members.map((s) => {
      const color = pickColor(taken)
      taken.push(color)
      return mkPane(s.name, { savedSession: s, color } as Partial<PaneData>)
    })
    const tab = mkGridTab(panes, group.name, group.id)
    tabs = [...tabs, tab]
    activeTabId = tab.id
    focusedPaneId = panes[0]!.id
  }

  function closeTab(id: number) {
    if (tabs.length <= 1) return
    const idx = tabs.findIndex((t) => t.id === id)
    tabs = tabs.filter((t) => t.id !== id)
    if (activeTabId === id) {
      activeTabId = (tabs[idx] ?? tabs[idx - 1])?.id ?? tabs[0]!.id
    }
    // The focused pane may have gone with the tab; without this the status bar
    // and "focused session" lookups kept pointing at the closed pane.
    if (!tabs.some((t) => tabPanes(t).some((p) => p.id === focusedPaneId))) {
      const active = tabs.find((t) => t.id === activeTabId)
      if (active) focusedPaneId = firstPaneId(active)
    }
  }

  function renameTab(id: number, label: string) {
    const t = tabs.find((x) => x.id === id)
    if (t) t.label = label.trim() || t.label
  }

  function setTabColor(id: number, color: string) {
    const t = tabs.find((x) => x.id === id)
    if (t) t.color = color
  }

  // Open a new tab with the same connection parameters as `id`'s first pane.
  function duplicateTab(id: number) {
    const tab = tabs.find((t) => t.id === id)
    if (!tab) return
    const first = tabPanes(tab)[0]
    if (!first) return
    if (first.savedSession) {
      openSavedSessionTab(first.savedSession)
      return
    }
    const pane = mkPane(first.label, {
      ssh: first.ssh,
      telnet: first.telnet,
    } as Partial<PaneData>)
    const newTab = mkTab(pane)
    newTab.color = tab.color
    tabs = [...tabs, newTab]
    activeTabId = newTab.id
    focusedPaneId = pane.id
  }

  function activateTab(id: number) {
    activeTabId = id
    const tab = tabs.find((t) => t.id === id)
    if (tab) focusedPaneId = firstPaneId(tab)
  }

  function handleStatusChange(tabId: number, paneId: number, status: PaneStatus) {
    const tab = tabs.find((t) => t.id === tabId)
    if (!tab) return
    tab.statuses.set(paneId, status)
  }

  /// Split the focused leaf in the active tab in `direction`. No-op on grids.
  function handleSplit(paneId: number, direction: SplitDirection) {
    const tab = tabs.find((t) => t.id === activeTabId)
    if (!tab || tab.layout.kind !== 'split') return
    // Split into an EMPTY placeholder pane (no auto local-shell). The user
    // fills it by clicking a saved session in the sidebar, which routes into
    // the focused empty pane. No status until it's filled.
    const newPane = mkPane('New pane', { empty: true } as Partial<PaneData>)
    tab.layout.root = splitLeaf(tab.layout.root, paneId, direction, newPane)
    focusedPaneId = newPane.id
  }

  /// Close a single pane. On a split tab, collapses the tree (closing the tab
  /// if it was the last leaf). On a grid tab, drops the pane from the grid
  /// (closing the tab when the last pane goes).
  function handleClosePane(paneId: number) {
    const tab = tabs.find((t) => t.id === activeTabId)
    if (!tab) return
    if (tab.layout.kind === 'grid') {
      const remaining = tab.layout.panes.filter((p) => p.id !== paneId)
      tab.statuses.delete(paneId)
      if (remaining.length === 0) {
        closeTab(tab.id)
        return
      }
      tab.layout.panes = remaining
      tab.layout.cols = Math.max(1, Math.ceil(Math.sqrt(remaining.length)))
      if (focusedPaneId === paneId) focusedPaneId = firstPaneId(tab)
      return
    }
    const next = removeLeaf(tab.layout.root, paneId)
    tab.statuses.delete(paneId)
    if (next === null) {
      closeTab(tab.id)
      return
    }
    tab.layout.root = next
    if (focusedPaneId === paneId) {
      focusedPaneId = firstPaneId(tab)
    }
  }

  /// Drag-resize a divider — `path` walks the tree from the root.
  function handleResize(path: number[], ratio: number) {
    const tab = tabs.find((t) => t.id === activeTabId)
    if (!tab || tab.layout.kind !== 'split') return
    tab.layout.root = setRatio(tab.layout.root, path, ratio)
  }

  // ── Resizable sidebar divider ──────────────────────────────────────────────
  // The sidebar width is driven by the reactive `uiSettings.sidebarWidth`
  // (loaded on startup, defaults to DEFAULT_SIDEBAR_WIDTH). Dragging the handle
  // updates it live, clamped to [MIN, 50% of the window]; the chosen width is
  // persisted on drag-end via the generic settings bridge. The terminal pane's
  // own ResizeObserver/FitAddon reflows automatically once the flex row resizes.
  let draggingSidebar = $state(false)

  function onSidebarHandleDown(e: PointerEvent) {
    e.preventDefault()
    draggingSidebar = true
    const startX = e.clientX
    const startWidth = uiSettings.sidebarWidth
    const handle = e.currentTarget as HTMLElement
    // Capture the pointer so releasing OUTSIDE the OS window still fires
    // pointerup on this element — otherwise the drag could "stick".
    handle.setPointerCapture(e.pointerId)
    const move = (ev: PointerEvent) => {
      uiSettings.sidebarWidth = clampSidebarWidth(startWidth + (ev.clientX - startX))
    }
    const end = () => {
      draggingSidebar = false
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', end)
      handle.removeEventListener('pointercancel', end)
      try {
        handle.releasePointerCapture(e.pointerId)
      } catch {
        // capture may already be gone (e.g. on pointercancel) — ignore.
      }
      // Persist the final width (debounced to drag-end).
      applySidebarWidth(uiSettings.sidebarWidth).catch(console.error)
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', end)
    handle.addEventListener('pointercancel', end)
  }

  /// Double-click the handle → reset to the default width (and persist it).
  function resetSidebarWidth() {
    applySidebarWidth(DEFAULT_SIDEBAR_WIDTH).catch(console.error)
  }

  // Re-clamp on window resize so a previously-saved wide sidebar can't exceed
  // 50% of the window after it shrinks.
  $effect(() => {
    function onResize() {
      uiSettings.sidebarWidth = clampSidebarWidth(uiSettings.sidebarWidth)
    }
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  })

  /// TabBar's "Split" button: split the currently focused pane horizontally.
  function handleSplitFocused() {
    handleSplit(focusedPaneId, 'h')
  }

  function handleSessionAdded() {
    showNewSession = false
    load()
  }

  // ── keyboard shortcuts ────────────────────────────────────────────────────────

  /// Execute the handler tied to a [`ShortcutAction`]. Called both from the
  /// global document listener and from xterm's custom key handler (via
  /// `onGlobalShortcut`), so xterm-focused panes route shortcuts identically.
  function dispatchAction(action: ShortcutAction) {
    switch (action) {
      case 'new-session':
        showNewSession = true
        break
      case 'new-tab':
        addLocalTab()
        break
      case 'close-tab':
        closeTab(activeTabId)
        break
      case 'next-tab':
      case 'prev-tab': {
        const dir = action === 'prev-tab' ? -1 : 1
        const idx = tabs.findIndex((t) => t.id === activeTabId)
        const next = tabs[(idx + dir + tabs.length) % tabs.length]
        if (next) activateTab(next.id)
        break
      }
      case 'settings':
        showSettings = true
        break
      case 'snippets':
        showSnippets = true
        break
      case 'split-h':
        handleSplit(focusedPaneId, 'h')
        break
      case 'split-v':
        handleSplit(focusedPaneId, 'v')
        break
      case 'multi-exec':
        multiOn = !multiOn
        break
      case 'quick-connect':
        showQuickConnect = true
        break
    }
  }

  /// Check the configured update endpoint, ask the user, and install on confirm.
  /// Requires the updater plugin to be `active` in tauri.conf.json with a valid
  /// signing pubkey + endpoint — until then this surfaces a clear error.
  /// In-app auto-update isn't wired (the repo/releases are private, so there's
  /// no public manifest a client could read). Open the releases page instead,
  /// where the latest build can be downloaded.
  async function handleCheckUpdates() {
    const releasesUrl = 'https://github.com/CarlosTejeiro/zapterm/releases'
    try {
      await openExternal(releasesUrl)
      showToast({
        kind: 'info',
        title: 'Releases',
        detail: `Current version ${appVersion}. Opening the releases page…`,
      })
    } catch (e) {
      showToast({ kind: 'error', title: 'Releases', detail: errorText(e) })
    }
  }

  // ── export / import ──────────────────────────────────────────────────────

  /// Export the whole environment to a user-chosen JSON file. Warns that
  /// login scripts travel verbatim (their `send` steps may contain secrets);
  /// passwords themselves never leave the keyring.
  async function handleExport() {
    const proceed = await ask(
      'This exports sessions, folders, groups, snippets and highlight rules.\n\n' +
        'Passwords are NOT exported. Note: login scripts ARE exported verbatim ' +
        '(their "send" steps may contain secrets you typed).',
      { title: 'Export sessions', kind: 'info' },
    )
    if (!proceed) return
    const path = await saveFileDialog({
      title: 'Export ZapTerm sessions',
      defaultPath: 'zapterm-sessions.json',
      filters: [{ name: 'ZapTerm export', extensions: ['json'] }],
    })
    if (!path) return
    try {
      const s = await exportSessions(path)
      showToast({
        kind: 'success',
        title: 'Export complete',
        detail: `${s.sessions} sessions, ${s.folders} folders, ${s.groups} groups, ${s.snippets} snippets, ${s.rules} rules → ${s.path}`,
      })
    } catch (e) {
      showToast({ kind: 'error', title: 'Export failed', detail: errorText(e) })
    }
  }

  /// Shared toast reporting for every import source.
  async function reportImport(s: ImportSummary) {
    showToast({
      kind: 'success',
      title: 'Import complete',
      detail:
        `${s.sessions_added} new sessions (${s.sessions_skipped} already existed), ` +
        `${s.folders_added} folders, ${s.groups_added} groups, ` +
        `${s.snippets_added} snippets, ${s.rules_added} rules`,
    })
    if (s.warnings.length > 0) {
      showToast({
        kind: 'warning',
        title: `Import: ${s.warnings.length} warning(s)`,
        detail: s.warnings.slice(0, 3).join(' · ') + (s.warnings.length > 3 ? ' …' : ''),
      })
    }
    await load()
  }

  /// Merge a ZapTerm export file into the current environment (idempotent —
  /// existing items are skipped) and refresh every affected store.
  async function handleImport() {
    const path = await openFileDialog({
      title: 'Import ZapTerm sessions',
      multiple: false,
      directory: false,
      filters: [{ name: 'ZapTerm export', extensions: ['json'] }],
    })
    if (typeof path !== 'string' || !path) return
    try {
      await reportImport(await importSessions(path))
    } catch (e) {
      showToast({ kind: 'error', title: 'Import failed', detail: errorText(e) })
    }
  }

  /// Import hosts from the OpenSSH client config. Defaults to ~/.ssh/config;
  /// declining the prompt opens a file picker for a custom location.
  async function handleImportSshConfig() {
    const useDefault = await ask(
      'Import hosts from ~/.ssh/config?\n\nChoose "No" to pick a different SSH config file.',
      { title: 'Import from SSH config', kind: 'info' },
    )
    let path: string | undefined
    if (!useDefault) {
      const picked = await openFileDialog({
        title: 'Choose ssh_config file',
        multiple: false,
        directory: false,
      })
      if (typeof picked !== 'string' || !picked) return
      path = picked
    }
    try {
      await reportImport(await importSshConfig(path))
    } catch (e) {
      showToast({ kind: 'error', title: 'SSH config import failed', detail: errorText(e) })
    }
  }

  /// PuTTY: on Windows offer the registry directly; otherwise (or on "No")
  /// pick a `.reg` export. The backend errors clearly if the registry is
  /// asked for off-Windows.
  async function handleImportPutty() {
    const fromRegistry = await ask(
      'Import from the Windows registry?\n\nChoose "No" to pick an exported .reg file (required on macOS/Linux).',
      { title: 'Import from PuTTY', kind: 'info' },
    )
    let path: string | undefined
    if (!fromRegistry) {
      const picked = await openFileDialog({
        title: 'Choose PuTTY .reg export',
        multiple: false,
        directory: false,
        filters: [{ name: 'Registry export', extensions: ['reg'] }],
      })
      if (typeof picked !== 'string' || !picked) return
      path = picked
    }
    try {
      await reportImport(await importPutty(path))
    } catch (e) {
      showToast({ kind: 'error', title: 'PuTTY import failed', detail: errorText(e) })
    }
  }

  /// MobaXterm: pick a MobaXterm.ini or .mxtsessions file.
  async function handleImportMobaXterm() {
    const picked = await openFileDialog({
      title: 'Choose MobaXterm.ini',
      multiple: false,
      directory: false,
      filters: [{ name: 'MobaXterm', extensions: ['ini', 'mxtsessions'] }],
    })
    if (typeof picked !== 'string' || !picked) return
    try {
      await reportImport(await importMobaXterm(picked))
    } catch (e) {
      showToast({ kind: 'error', title: 'MobaXterm import failed', detail: errorText(e) })
    }
  }

  /// SecureCRT: pick the "Sessions" directory of its Config folder.
  async function handleImportSecureCrt() {
    const picked = await openFileDialog({
      title: 'Choose SecureCRT Sessions folder',
      directory: true,
      multiple: false,
    })
    if (typeof picked !== 'string' || !picked) return
    try {
      await reportImport(await importSecureCrt(picked))
    } catch (e) {
      showToast({ kind: 'error', title: 'SecureCRT import failed', detail: errorText(e) })
    }
  }

  /// Open a new tab from a [`QuickConnectDialog`] submission. Connections are
  /// in-memory only (no SQLite row), so closing the tab cleans up everything.
  function handleQuickConnect(params: ConnectParams) {
    showQuickConnect = false
    if (params.type === 'local') {
      addLocalTab()
      return
    }
    if (params.type === 'ssh') {
      const pane = mkPane(`${params.user}@${params.host}`, {
        ssh: {
          host: params.host,
          port: params.port,
          user: params.user,
          auth: { type: 'password', password: params.password },
        },
      } as Partial<PaneData>)
      const tab = mkTab(pane)
      tabs = [...tabs, tab]
      activeTabId = tab.id
      focusedPaneId = pane.id
      return
    }
    // telnet
    const pane = mkPane(`telnet://${params.host}`, {
      telnet: { host: params.host, port: params.port },
    } as Partial<PaneData>)
    const tab = mkTab(pane)
    tabs = [...tabs, tab]
    activeTabId = tab.id
    focusedPaneId = pane.id
  }

  /// Fire the i-th snippet (0-indexed) at the focused session. Used by the
  /// Ctrl+Shift+1..9 quick-launch shortcut wired in TerminalTab. We address
  /// `visibleSnippets` (what the bar currently shows for this session)
  /// rather than the global list — so the [N] pill on screen matches the
  /// key the user pressed.
  async function fireSnippetByIndex(idx: number) {
    const s = visibleSnippets[idx]
    if (s) await fireSnippet(s)
  }

  /// Fire a snippet or macro by id — the target of its own keyboard shortcut.
  /// Looks in the FULL list: macros live in the sidebar, not the 9-slot bar.
  async function fireSnippetById(id: number) {
    const s = snippets.find((x) => x.id === id)
    if (s) await fireSnippet(s)
  }

  /// Send one snippet to the focused session (and the broadcast group). A
  /// macro runs its expect/send/wait steps — vault references are written by
  /// the backend, so a password never passes through the UI.
  async function fireSnippet(s: Snippet) {
    if (s.steps_json) {
      await runMacroOnFocused(s)
      return
    }
    const focused = getFocusedSessionId()
    if (!focused) return
    // Resolve any {{variables}} (prompts the user) before sending.
    const text = await resolveSnippet(s.content)
    if (text === null) return
    await sendInputText(focused, text).catch(console.error)
    // Variable prompts steal focus; restore it so the user can keep typing
    // after a snippet that carries no trailing newline.
    focusSession(focused)
    if (broadcast.enabled) {
      for (const id of broadcastTargets(focused)) {
        await sendInputText(id, text).catch(() => {})
      }
    }
  }

  /// xterm forwards `(action, e)` here when its custom key handler matches a
  /// binding. `action` is the [`ShortcutAction`] id (typed as string for the
  /// existing callback signature). The `snippet:N` namespace is handled here
  /// to avoid widening the typed action union.
  function handleGlobalShortcut(action: string, _e: KeyboardEvent) {
    if (action.startsWith('snippet-id:')) {
      const id = parseInt(action.slice('snippet-id:'.length), 10)
      if (!Number.isNaN(id)) fireSnippetById(id)
      return
    }
    if (action.startsWith('snippet:')) {
      const idx = parseInt(action.slice('snippet:'.length), 10)
      if (!Number.isNaN(idx)) fireSnippetByIndex(idx)
      return
    }
    dispatchAction(action as ShortcutAction)
  }

  // Build the command-palette item list. Re-derives whenever sessions or
  // theme change. Mirrors the existing dispatchAction surface plus theme
  // switching + the "open saved session" shortcut.
  const paletteItems = $derived<PaletteItem[]>([
    ...sessions.map((s) => ({
      id: `session-${s.id}`,
      label: s.name,
      subtitle: `${s.protocol.toUpperCase()} · ${s.host ?? ''}${s.port ? ':' + s.port : ''}${s.username ? ' · ' + s.username : ''}`,
      icon: 'term' as const,
      section: 'Sessions' as const,
      run: () => openSavedSessionTab(s),
    })),
    {
      id: 'act-new-tab',
      label: 'New local tab',
      icon: 'plus' as const,
      section: 'Actions' as const,
      run: addLocalTab,
    },
    {
      id: 'act-new-session',
      label: 'New saved session',
      icon: 'file' as const,
      section: 'Actions' as const,
      run: () => (showNewSession = true),
    },
    {
      id: 'act-quick',
      label: 'Quick Connect',
      icon: 'bolt' as const,
      section: 'Actions' as const,
      run: () => (showQuickConnect = true),
    },
    {
      id: 'act-run-script',
      label: 'Run Python script…',
      icon: 'term' as const,
      section: 'Actions' as const,
      run: chooseScript,
    },
    {
      id: 'act-notebook',
      label: 'Allow notebook connection',
      icon: 'term' as const,
      section: 'Actions' as const,
      run: () => void openNotebookConnection(),
    },
    {
      id: 'act-script-examples',
      label: 'Open script examples folder',
      icon: 'folder' as const,
      section: 'Actions' as const,
      run: () => {
        scriptExamplesDir(true).catch(() => {})
      },
    },
    {
      id: 'act-snippets',
      label: 'Open Snippets',
      icon: 'book' as const,
      section: 'Actions' as const,
      run: () => (showSnippets = true),
    },
    {
      id: 'act-groups',
      label: 'Broadcast groups…',
      icon: 'cast' as const,
      section: 'Actions' as const,
      run: () => (showGroups = true),
    },
    {
      id: 'act-cmdlist',
      label: 'Send command list…',
      icon: 'file' as const,
      section: 'Actions' as const,
      run: () => (showCommandList = true),
    },
    {
      id: 'act-tunnels',
      label: 'Active tunnels…',
      icon: 'tunnel' as const,
      section: 'Actions' as const,
      run: () => (showTunnelsManager = true),
    },
    {
      id: 'act-settings',
      label: 'Open Settings',
      icon: 'gear' as const,
      section: 'Actions' as const,
      run: () => (showSettings = true),
    },
    ...groups.map((g) => ({
      id: `group-${g.id}`,
      label: `Open group: ${g.name}`,
      subtitle: `${g.session_ids.length} session${g.session_ids.length === 1 ? '' : 's'} · grid`,
      icon: 'cast' as const,
      section: 'Groups' as const,
      run: () => openGroupAsGrid(g),
    })),
    {
      id: 'act-split-h',
      label: 'Split horizontally',
      icon: 'split' as const,
      section: 'Actions' as const,
      run: () => handleSplit(focusedPaneId, 'h'),
    },
    {
      id: 'act-split-v',
      label: 'Split vertically',
      icon: 'splitV' as const,
      section: 'Actions' as const,
      run: () => handleSplit(focusedPaneId, 'v'),
    },
    {
      id: 'act-multi',
      label: 'Toggle multi-exec',
      icon: 'cast' as const,
      section: 'Actions' as const,
      run: () => (multiOn = !multiOn),
    },
    {
      id: 'act-close-tab',
      label: 'Close current tab',
      icon: 'x' as const,
      section: 'Actions' as const,
      run: () => closeTab(activeTabId),
    },
    {
      id: 'act-open-logs',
      label: 'Open logs folder',
      icon: 'folder' as const,
      section: 'Actions' as const,
      run: openLogsFolder,
    },
    {
      id: 'act-export',
      label: 'Export sessions…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleExport,
    },
    {
      id: 'act-import',
      label: 'Import sessions…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleImport,
    },
    {
      id: 'act-import-ssh',
      label: 'Import from SSH config…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleImportSshConfig,
    },
    {
      id: 'act-import-putty',
      label: 'Import from PuTTY…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleImportPutty,
    },
    {
      id: 'act-import-moba',
      label: 'Import from MobaXterm…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleImportMobaXterm,
    },
    {
      id: 'act-import-scrt',
      label: 'Import from SecureCRT…',
      icon: 'transfer' as const,
      section: 'Actions' as const,
      run: handleImportSecureCrt,
    },
    {
      id: 'act-about',
      label: 'About ZapTerm',
      icon: 'book' as const,
      section: 'Actions' as const,
      run: () => (showAbout = true),
    },
    ...Object.entries(themeLabels).map(([key, label]) => ({
      id: `theme-${key}`,
      label: `Tema · ${label}`,
      icon: 'contrast' as const,
      section: 'Themes' as const,
      run: () => applyTheme(key),
    })),
  ])

  // Throughput in the StatusBar reflects the runtime session of the focused
  // pane. paneToSession is the runtime-store map updated by TerminalTab.
  const focusedRuntimeSessionId = $derived(paneToSession.get(focusedPaneId) ?? null)

  // TCP MSS for the focused SSH session. Initial value comes from the
  // snapshot the backend captured at connect; live updates arrive via the
  // `session-mss-updated` Tauri event the backend polling task emits
  // (every ~5s, only when the value actually changes).
  let focusedMss = $state<{ send: number | null; recv: number | null } | null>(null)
  // Keyed cache so the listener doesn't blow away the snapshot for a
  // session that isn't currently focused.
  const mssBySession = new Map<string, { send: number | null; recv: number | null }>()

  $effect(() => {
    const sid = focusedRuntimeSessionId
    if (!sid) {
      focusedMss = null
      return
    }
    const cached = mssBySession.get(sid)
    if (cached) {
      focusedMss = cached
    } else {
      getSessionTcpMss(sid)
        .then((m) => {
          mssBySession.set(sid, m)
          if (focusedRuntimeSessionId === sid) focusedMss = m
        })
        .catch(() => {
          focusedMss = null
        })
    }
  })

  onMount(() => {
    function onKeydown(e: KeyboardEvent) {
      // Command palette: Ctrl+K or Cmd+K — handled BEFORE the keybindings
      // matcher so user overrides don't conflict.
      if ((e.ctrlKey || e.metaKey) && e.key === 'k' && !e.altKey && !e.shiftKey) {
        e.preventDefault()
        showPalette = true
        return
      }
      const action = matchAction(e)
      if (action) {
        e.preventDefault()
        dispatchAction(action)
        return
      }
      // A snippet/macro shortcut pressed outside the terminal (sidebar, button
      // bar…). Not while typing in a field — that keystroke is the user's text,
      // and the shortcut editor captures combos itself. Inside the terminal,
      // TerminalTab handles it and stops propagation, so it can't fire twice.
      const t = e.target as HTMLElement | null
      const typing =
        !!t && (t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')
      if (typing) return
      const bound = matchSnippetShortcut(e)
      if (!bound) return
      e.preventDefault()
      fireSnippet(bound)
    }
    document.addEventListener('keydown', onKeydown)

    let unlistenStats: UnlistenFn | null = null
    listen<{ session_id: string; bytes_per_sec: number }>('session-stats', (e) => {
      bytesPerSecBySession = {
        ...bytesPerSecBySession,
        [e.payload.session_id]: e.payload.bytes_per_sec,
      }
    }).then((fn) => {
      unlistenStats = fn
    })

    // Live MSS updates — backend emits when the value changes (typically
    // because Path-MTU Discovery revised the negotiated size). We cache by
    // session so navigating away and back retains the latest known value.
    let unlistenMss: UnlistenFn | null = null
    listen<{ session_id: string; mss: { send: number | null; recv: number | null } }>(
      'session-mss-updated',
      (e) => {
        mssBySession.set(e.payload.session_id, e.payload.mss)
        if (focusedRuntimeSessionId === e.payload.session_id) {
          focusedMss = e.payload.mss
        }
      },
    ).then((fn) => {
      unlistenMss = fn
    })

    // Platform auto-detect — backend emits once per session the first time
    // it latches onto a recognisable prompt. Surface a toast and refresh
    // the saved-session list so the new platform shows up in EditSession
    // without the user having to reload the panel.
    let unlistenPlatform: UnlistenFn | null = null
    listen<{ session_id: string; platform: string; display_name: string }>(
      'session-platform-detected',
      async (e) => {
        showToast({
          kind: 'info',
          title: 'Platform detected',
          detail: e.payload.display_name,
        })
        // Refresh sessions so the updated platform is visible immediately.
        listSessions()
          .then((v) => (sessions = v))
          .catch(() => {})
        // If the detection landed on the focused session, refresh the bar with
        // the new platform — that's what surfaces the seeded defaults. Match by
        // RUNTIME session id so this also works for unsaved/quick-connect
        // sessions (which have no saved-session row).
        const focusedRuntimeSid = paneToSession.get(focusedPaneId) ?? null
        if (focusedRuntimeSid === e.payload.session_id) {
          const savedSid = focusedPaneData?.savedSession?.id ?? null
          await setBarContext(e.payload.platform, savedSid)
        }
      },
    ).then((fn) => {
      unlistenPlatform = fn
    })

    // SFTP "edit remote file": the backend re-uploads on each save and emits
    // these so the user gets feedback that their edit landed.
    let unlistenEditSaved: UnlistenFn | null = null
    listen<string>('sftp-edit-saved', (e) => {
      showToast({ kind: 'success', title: 'Uploaded', detail: e.payload })
    }).then((fn) => {
      unlistenEditSaved = fn
    })
    let unlistenEditError: UnlistenFn | null = null
    listen<string>('sftp-edit-error', (e) => {
      showToast({ kind: 'error', title: 'Upload failed', detail: e.payload })
    }).then((fn) => {
      unlistenEditError = fn
    })

    // Sync folder: the background checker emits when another device
    // published changes, on a conflict, or on an error. Each distinct remote
    // bundle is offered once — declining it doesn't nag every few minutes.
    let unlistenSync: UnlistenFn | null = null
    let lastSyncOffer = ''
    listen<SyncStatus>(SYNC_EVENT, (e) => {
      const st = e.payload
      const key = `${st.state}|${st.remote?.written_at ?? ''}|${st.detail ?? ''}`
      if (key === lastSyncOffer) return
      lastSyncOffer = key
      handleSyncStatus(st).catch((err) => console.error('sync', err))
    }).then((fn) => {
      unlistenSync = fn
    })

    // Python scripts: the host carries out each `zapterm` call on our tabs.
    let teardownScripts: (() => void) | null = null
    initScriptHost({
      focusedSession: () => {
        const sid = paneToSession.get(focusedPaneId)
        return sid ? describeSession(sid) : null
      },
      openTabs: () => {
        const out = []
        for (const t of tabs) {
          for (const p of tabPanes(t)) {
            const sid = paneToSession.get(p.id)
            const d = sid ? describeSession(sid) : null
            if (d) out.push(d)
          }
        }
        return out
      },
      describe: describeSession,
      refreshPlatform: refreshSessionPlatform,
      savedNames: savedSessionNames,
      openSaved: (name) => {
        const s = sessions.find((x) => x.name === name)
        if (!s) throw new Error(`no saved session called "${name}"`)
        return openSavedSessionTab(s, scriptRun.readOnly)
      },
      paneStatus: (paneId) => {
        const tab = tabs.find((t) => tabPanes(t).some((p) => p.id === paneId))
        if (!tab) return 'closed'
        return tab.statuses.get(paneId) ?? 'connecting'
      },
      closeSession: (sessionId) => {
        const found = paneOfSession(sessionId)
        if (!found) return
        activeTabId = found.tab.id
        handleClosePane(found.paneId)
      },
      confirm: (message) => ask(message, { title: 'Script', kind: 'warning' }),
    }).then((fn) => {
      teardownScripts = fn
    })

    return () => {
      if (teardownScripts) teardownScripts()
      document.removeEventListener('keydown', onKeydown)
      if (unlistenStats) unlistenStats()
      if (unlistenPlatform) unlistenPlatform()
      if (unlistenMss) unlistenMss()
      if (unlistenEditSaved) unlistenEditSaved()
      if (unlistenEditError) unlistenEditError()
      if (unlistenSync) unlistenSync()
    }
  })
</script>

<!-- ── Resize handles (frameless window) ───────────────────────────────────── -->
<ResizeHandles />

<!-- ── PYLON shell ──────────────────────────────────────────────────────────── -->
<div
  class="pylon-shell"
  style:background={theme.appBg}
  style:--cursor-glow={theme.terminal.cursor}
  data-glow={theme.glows ? 'true' : 'false'}
  data-theme={theme.name}
>
  <TitleBar
    {theme}
    sessionName={activeSessionName}
    themeName={theme.name}
    onNewSession={() => (showNewSession = true)}
    onQuickConnect={() => (showQuickConnect = true)}
    onSettings={() => (showSettings = true)}
    onSnippets={() => (showSnippets = true)}
    onRunScript={chooseScript}
    onScriptExamples={() => scriptExamplesDir(true).catch(() => {})}
    onNotebook={() => void openNotebookConnection()}
    onCheckUpdates={handleCheckUpdates}
    onSetTheme={(k) => applyTheme(k)}
    onAbout={() => (showAbout = true)}
    onExport={handleExport}
    onImport={handleImport}
    onImportSshConfig={handleImportSshConfig}
    onImportPutty={handleImportPutty}
    onImportMobaXterm={handleImportMobaXterm}
    onImportSecureCrt={handleImportSecureCrt}
    onCommandList={() => (showCommandList = true)}
    onTunnelsManager={() => (showTunnelsManager = true)}
  />

  <div class="pylon-body" class:resizing-sidebar={draggingSidebar}>
    <Sidebar
      {theme}
      width={uiSettings.sidebarWidth}
      {sessions}
      {folders}
      activeSessionId={focusedPaneData?.savedSession?.id}
      {sessionStatuses}
      onSelect={(s) => openSavedSessionTab(s)}
      onEdit={(s) => (editingSession = s)}
      onDuplicate={async (s) => {
        try {
          await cloneSavedSession(s.id)
          await load()
          showToast({ kind: 'success', title: 'Session duplicated', detail: `${s.name} (copy)` })
        } catch (e) {
          showToast({
            kind: 'error',
            title: 'Duplicate failed',
            detail: errorText(e),
          })
        }
      }}
      onDelete={async (s) => {
        // Sessions that jump through this one would silently start
        // connecting directly (the link is cleared): say so first.
        const via = sessions.filter((x) => x.via_session_id === s.id).map((x) => x.name)
        const viaNote = via.length
          ? `\n\n${via.length === 1 ? '1 session uses' : `${via.length} sessions use`} it as jump host (${via.join(', ')}) and will connect directly.`
          : ''
        const ok = await ask(`Delete "${s.name}"? This can't be undone.${viaNote}`, {
          title: 'Delete session',
          kind: 'warning',
        })
        if (!ok) return
        try {
          await deleteSavedSession(s.id)
          await load()
        } catch (e) {
          showToast({ kind: 'error', title: 'Delete failed', detail: errorText(e) })
        }
      }}
      onCreateFolder={() => {
        openPrompt({
          title: 'New folder',
          placeholder: 'My folder',
          submitLabel: 'Create',
          onSubmit: async (name) => {
            prompt = null
            try {
              await createFolder(name, null)
              await load()
            } catch (e) {
              showToast({ kind: 'error', title: 'Create folder failed', detail: errorText(e) })
            }
          },
        })
      }}
      onRenameFolder={(folder) => {
        openPrompt({
          title: `Rename "${folder.name}"`,
          initial: folder.name,
          submitLabel: 'Rename',
          onSubmit: async (name) => {
            prompt = null
            if (name === folder.name) return
            try {
              await renameFolder(folder.id, name)
              await load()
            } catch (e) {
              showToast({ kind: 'error', title: 'Rename failed', detail: errorText(e) })
            }
          },
        })
      }}
      onCreateSubfolder={(parent) => {
        openPrompt({
          title: `New folder in "${parent.name}"`,
          placeholder: 'My folder',
          submitLabel: 'Create',
          onSubmit: async (name) => {
            prompt = null
            try {
              await createFolder(name, parent.id)
              await load()
            } catch (e) {
              showToast({ kind: 'error', title: 'Create folder failed', detail: errorText(e) })
            }
          },
        })
      }}
      onDeleteFolder={async (folder) => {
        // Deleting a folder also deletes its subfolders (ON DELETE CASCADE);
        // the sessions of all of them move to the root.
        const sub = new Set<number>()
        const walk = (id: number) => {
          for (const f of folders)
            if (f.parent_id === id && !sub.has(f.id)) {
              sub.add(f.id)
              walk(f.id)
            }
        }
        walk(folder.id)
        sub.delete(folder.id)
        const subNote = sub.size
          ? ` Its ${sub.size === 1 ? 'subfolder is' : `${sub.size} subfolders are`} deleted too; all their sessions move to the root.`
          : ' Its sessions move to the root.'
        const ok = await ask(`Delete folder "${folder.name}"?${subNote}`, {
          title: 'Delete folder',
          kind: 'warning',
        })
        if (!ok) return
        try {
          await deleteFolder(folder.id)
          await load()
        } catch (e) {
          showToast({ kind: 'error', title: 'Delete folder failed', detail: errorText(e) })
        }
      }}
      onMove={async (sessionId, folderId) => {
        try {
          await moveSavedSession(sessionId, folderId)
          await load()
        } catch (e) {
          showToast({ kind: 'error', title: 'Move failed', detail: errorText(e) })
        }
      }}
      onReorder={async (sessionId, folderId, targetIndex) => {
        try {
          await reorderSavedSession(sessionId, folderId, targetIndex)
          await load()
        } catch (e) {
          showToast({ kind: 'error', title: 'Reorder failed', detail: errorText(e) })
        }
      }}
      onAddSession={() => (showNewSession = true)}
      onSettings={() => (showSettings = true)}
      onToggleTheme={toggleTheme}
      onOpenLogs={openLogsFolder}
      {macros}
      onRunMacro={(m) => runMacroOnFocused(m)}
      onEditMacro={(m) => (editingMacro = m)}
      onCloneMacro={cloneMacro}
      onSetMacroFolder={setMacroFolder}
      onReorderMacros={reorderMacros}
      onNewMacro={() => (editingMacro = 'new')}
      onImportMacro={() => (showMacroImport = true)}
      onImportMacrosJson={importMacrosFromFile}
      onExportMacros={() => exportMacrosToFile(null, `${macros.length} macros`)}
      onExportMacro={(m) => exportMacrosToFile([m.id], m.name)}
      onOpenVault={() => (showVault = true)}
    />

    <!-- Draggable divider between the sidebar and the terminal area. Widen the
         sidebar to read long session names; double-click resets to default. -->
    <div
      class="sidebar-resizer"
      class:dragging={draggingSidebar}
      style:--accent={theme.accent}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
      onpointerdown={onSidebarHandleDown}
      ondblclick={resetSidebarWidth}
    ></div>

    <div class="pylon-workspace" style:background={theme.bodyBg}>
      <TabBar
        {theme}
        tabs={tabEntries}
        {activeTabId}
        {splitOn}
        {multiOn}
        onActivate={activateTab}
        onAdd={addLocalTab}
        onClose={closeTab}
        onRename={renameTab}
        onDuplicate={duplicateTab}
        onSetColor={setTabColor}
        onToggleSplit={handleSplitFocused}
        onToggleMulti={() => (multiOn = !multiOn)}
      />

      <div class="pylon-panes">
        {#each tabs as tab (tab.id)}
          <!-- All tabs stay mounted; inactive ones are hidden with CSS
               (display:none). This keeps each tab's xterm instance — and its
               backend session — alive across tab switches, so scrollback and
               the running shell survive instead of being torn down and
               re-opened (which looked like a `clear`). -->
          <div class="pane-slot" class:hidden={tab.id !== activeTabId}>
            {#if tab.layout.kind === 'grid'}
              <GridView
                panes={tab.layout.panes}
                cols={tab.layout.cols}
                {theme}
                {focusedPaneId}
                active={tab.id === activeTabId}
                onFocus={(id) => (focusedPaneId = id)}
                onClosePane={(id) => handleClosePane(id)}
                onStatusChange={(id, s) => handleStatusChange(tab.id, id, s)}
                onGlobalShortcut={handleGlobalShortcut}
              />
            {:else}
              <SplitTree
                node={tab.layout.root}
                {theme}
                {focusedPaneId}
                {canClosePanes}
                onFocus={(id) => (focusedPaneId = id)}
                onSplit={(id, dir) => handleSplit(id, dir)}
                onClosePane={(id) => handleClosePane(id)}
                onStatusChange={(id, s) => handleStatusChange(tab.id, id, s)}
                onGlobalShortcut={handleGlobalShortcut}
                onResize={(path, ratio) => handleResize(path, ratio)}
              />
            {/if}
          </div>
        {/each}
      </div>

      {#if multiOn && activeTab?.layout.kind === 'split'}
        <!-- Multi-exec on a regular tab: same master bar as grid mode,
             broadcasting to every live pane of the active tab, with the same
             run-and-compare audit. -->
        <MasterInputBar
          {theme}
          sessionIds={activeTabSessionIds}
          onRunCompare={runCompareActiveTab}
        />
      {/if}

      <CommandListBar {theme} />

      <StatusBar
        {theme}
        status={focusedStatus}
        host={statusHost || undefined}
        port={statusPort ?? undefined}
        protocol={statusProtocol}
        bytesPerSec={focusedRuntimeSessionId
          ? bytesPerSecBySession[focusedRuntimeSessionId]
          : undefined}
        mss={focusedMss}
      />
    </div>
  </div>
</div>

<!-- ── Dialogs ──────────────────────────────────────────────────────────────── -->
{#if showNewSession}
  <NewSessionDialog
    {folders}
    onCancel={() => (showNewSession = false)}
    onCreated={(_id) => handleSessionAdded()}
  />
{/if}

{#if editingSession}
  <NewSessionDialog
    {folders}
    existing={editingSession}
    onCancel={() => (editingSession = null)}
    onCreated={() => {
      editingSession = null
      load()
    }}
  />
{/if}

{#if editingMacro}
  {#key editingMacro}
    <MacroDialog
      {theme}
      macro={editingMacro === 'new' ? null : editingMacro}
      folders={macroFolders}
      onSave={saveMacro}
      onDelete={editingMacro === 'new' ? undefined : deleteMacro}
      onClose={() => (editingMacro = null)}
    />
  {/key}
{/if}

{#if showMacroImport}
  <MacroImportDialog {theme} onImport={importMacro} onClose={() => (showMacroImport = false)} />
{/if}

{#if showVault}
  <VaultDialog {theme} onClose={() => (showVault = false)} />
{/if}

{#if showQuickConnect}
  <QuickConnectDialog onCancel={() => (showQuickConnect = false)} onConnect={handleQuickConnect} />
{/if}

<KeyboardInteractiveDialog />

{#if showSnippets}
  <SnippetsDialog onClose={() => (showSnippets = false)} />
{/if}

{#if scriptPending}
  {@const path = scriptPending}
  <ScriptStartDialog
    {path}
    target={(() => {
      const sid = paneToSession.get(focusedPaneId)
      return sid ? (describeSession(sid)?.name ?? null) : null
    })()}
    onRun={(mode, allow) => {
      // Read the path BEFORE clearing scriptPending: `path` is derived from it.
      const chosen = path
      scriptPending = null
      void startScript(chosen, mode === 'dry', mode === 'read-only', allow)
    }}
    onCancel={() => (scriptPending = null)}
  />
{/if}

{#if scriptRun.visible}
  <ScriptPanel />
{/if}

{#if showGroups}
  <GroupsDialog
    {sessions}
    onClose={() => (showGroups = false)}
    onOpenGrid={(g) => {
      showGroups = false
      openGroupAsGrid(g)
    }}
  />
{/if}

{#if showCommandList}
  <CommandListDialog targets={liveTargets} onClose={() => (showCommandList = false)} />
{/if}

{#if showComparePanel}
  <ComparisonPanel
    {theme}
    command={compareRunner.command}
    hosts={compareRunner.hosts}
    running={compareRunner.running}
    onClose={() => {
      showComparePanel = false
      compareRunner.reset()
    }}
  />
{/if}

{#if showTunnelsManager}
  <TunnelsManagerDialog labels={sessionLabels} onClose={() => (showTunnelsManager = false)} />
{/if}

{#if showSettings}
  <SettingsModal onClose={() => (showSettings = false)} />
{/if}

{#if showPalette}
  <CommandPalette items={paletteItems} onClose={() => (showPalette = false)} />
{/if}

{#if prompt}
  <PromptDialog
    title={prompt.title}
    placeholder={prompt.placeholder}
    initial={prompt.initial}
    submitLabel={prompt.submitLabel}
    onSubmit={prompt.onSubmit}
    onCancel={() => (prompt = null)}
  />
{/if}

<Toasts />
<VariablesDialog />

{#if showAbout}
  <div
    class="about-backdrop"
    onclick={() => (showAbout = false)}
    onkeydown={(e) => {
      if (e.key === 'Escape') showAbout = false
    }}
    role="presentation"
    tabindex="-1"
  >
    <div
      class="about-modal"
      style:background={theme.sidebarBg}
      style:border="1px solid {theme.border}"
      style:font-family={theme.fontUi}
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <div class="about-header" style:border-bottom="1px solid {theme.border}">
        <MarkTile size={20} accent={theme.accent} paper={theme.appBg} />
        <span style:color={theme.textPrimary} style:font-size="15px" style:font-weight="600"
          >ZapTerm</span
        >
        <button class="about-close" style:color={theme.textDim} onclick={() => (showAbout = false)}
          ><Icon name="x" size={13} /></button
        >
      </div>
      <div class="about-body">
        <p style:color={theme.textMuted} style:font-size="12.5px" style:line-height="1.7">
          A modern multiprotocol terminal for network engineers.<br />
          Built with Tauri · Svelte 5 · xterm.js · Rust.
        </p>
        <table class="about-table" style:color={theme.textMuted}>
          <tbody>
            <tr
              ><td style:color={theme.textDim}>Version</td><td style:color={theme.textPrimary}
                >{appVersion || '—'}</td
              ></tr
            >
            <tr><td style:color={theme.textDim}>Runtime</td><td>Tauri 2 · WebView</td></tr>
            <tr
              ><td style:color={theme.textDim}>Theme</td><td style:color={theme.accent}
                >{themeLabels[theme.name] ?? theme.name}</td
              ></tr
            >
          </tbody>
        </table>
        <p class="about-hint" style:color={theme.textDim} style:font-family={theme.fontMono}>
          Credentials stored in the OS keyring; portable mode encrypts them with a local key file
        </p>
      </div>
    </div>
  </div>
{/if}

<style>
  .pylon-shell {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .pylon-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* While dragging the divider, suppress text selection across the whole body
     so a fast drag can't start selecting sidebar/terminal text. Cleared the
     moment `draggingSidebar` flips back to false. */
  .pylon-body.resizing-sidebar {
    user-select: none;
  }

  /* Drag handle between the sidebar and the terminal area. A thin (~5px) hit
     strip with a subtle centred rule that brightens on hover/active. */
  .sidebar-resizer {
    flex: 0 0 5px;
    width: 5px;
    cursor: col-resize;
    position: relative;
    z-index: 5;
    align-self: stretch;
    background: transparent;
    transition: background 0.1s;
  }

  .sidebar-resizer::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 2px;
    width: 1px;
    background: transparent;
    transition: background 0.1s;
  }

  .sidebar-resizer:hover::before,
  .sidebar-resizer.dragging::before {
    background: color-mix(in srgb, var(--accent) 70%, transparent);
  }

  .sidebar-resizer.dragging {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .pylon-workspace {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  /* Terminal zone — the cards float on the body background with a small
     frame around them (handoff: «terminal como tarjeta flotante»), kept tight
     so the terminal fills more of the window while staying a rounded card. */
  .pylon-panes {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
    position: relative;
    padding: 6px;
  }

  .pane-slot {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .pane-slot.hidden {
    display: none;
  }

  .about-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
    z-index: 300;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .about-modal {
    width: 360px;
    border-radius: 8px;
    box-shadow: var(--zx-shadow);
    overflow: hidden;
  }

  .about-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
  }

  .about-close {
    margin-left: auto;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 13px;
    padding: 2px 6px;
    border-radius: 4px;
    color: inherit;
    transition: background 0.1s;
  }

  .about-close:hover {
    background: var(--zx-hover-bg);
  }

  .about-body {
    padding: 0 18px 18px;
  }

  .about-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    margin: 10px 0;
  }

  .about-table td {
    padding: 4px 0;
  }

  .about-table td:first-child {
    width: 80px;
    font-size: 11px;
  }

  .about-hint {
    font-size: 10.5px;
    margin: 10px 0 0;
    opacity: 0.6;
  }
</style>
