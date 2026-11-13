<script lang="ts">
  import type { PylonTheme } from '$lib/themes/index'
  import type { SavedSession, Folder, Snippet } from '$lib/bridge/types'
  import Icon from '$lib/icons/Icon.svelte'
  import SessionAvatar from '$lib/pylon/SessionAvatar.svelte'
  import { rootFolders } from './folderTree'

  interface Props {
    theme: PylonTheme
    /** Sidebar width in px (driven by the resizable divider). */
    width?: number
    sessions: SavedSession[]
    folders: Folder[]
    activeSessionId?: number
    /** Live status per saved-session id → status dot on the avatar
     *  (connecting = amber, connected = green, error = red; absent = no dot).
     *  The focused session additionally gets the accent ring. */
    sessionStatuses?: Map<number, 'connecting' | 'connected' | 'error'>
    onSelect: (session: SavedSession) => void
    onEdit?: (session: SavedSession) => void
    onDuplicate?: (session: SavedSession) => void
    onDelete?: (session: SavedSession) => void
    onCreateFolder?: () => void
    onRenameFolder?: (folder: Folder) => void
    onDeleteFolder?: (folder: Folder) => void
    /** Create a folder inside `parent`. */
    onCreateSubfolder?: (parent: Folder) => void
    /** Reparent a session via drag-and-drop. `folderId = null` drops at root. */
    onMove?: (sessionId: number, folderId: number | null) => void
    /** Reorder a session inside its parent. `targetIndex` is the 0-based slot
     *  among the destination's children AFTER the move. Also used for cross-
     *  folder drops onto a specific row. */
    onReorder?: (sessionId: number, folderId: number | null, targetIndex: number) => void
    onAddSession?: () => void
    onSettings?: () => void
    onToggleTheme?: () => void
    /** Reveal the session-log folder in the file explorer. */
    onOpenLogs?: () => void
    /** Macro library shown in a section below the sessions. */
    macros?: Snippet[]
    onRunMacro?: (macro: Snippet) => void
    onEditMacro?: (macro: Snippet) => void
    /** Duplicate a macro (creates "<name> (copy)"). */
    onCloneMacro?: (macro: Snippet) => void
    /** Move a macro to a folder (`null` = ungrouped). */
    onSetMacroFolder?: (macro: Snippet, folder: string | null) => void
    /** Persist a new order for a folder group (ids in their new order). */
    onReorderMacros?: (orderedIds: number[]) => void
    onNewMacro?: () => void
    onImportMacro?: () => void
    /** Import macros from a ZapTerm JSON file. */
    onImportMacrosJson?: () => void
    /** Export all macros to a ZapTerm JSON file. */
    onExportMacros?: () => void
    /** Export a single macro to a ZapTerm JSON file. */
    onExportMacro?: (macro: Snippet) => void
    /** Open the credential vault manager. */
    onOpenVault?: () => void
  }

  const {
    theme,
    width = 248,
    sessions,
    folders,
    activeSessionId,
    sessionStatuses,
    onSelect,
    onEdit,
    onDuplicate,
    onDelete,
    onCreateFolder,
    onRenameFolder,
    onDeleteFolder,
    onCreateSubfolder,
    onMove,
    onReorder,
    onAddSession,
    onSettings,
    onToggleTheme,
    onOpenLogs,
    macros,
    onRunMacro,
    onEditMacro,
    onCloneMacro,
    onSetMacroFolder,
    onReorderMacros,
    onNewMacro,
    onImportMacro,
    onImportMacrosJson,
    onExportMacros,
    onExportMacro,
    onOpenVault,
  }: Props = $props()

  const macroList = $derived(macros ?? [])

  /// Order macros within a folder group by `position` (nulls last), then by
  /// sort_order, then name — matching the backend's stable fallback.
  function orderGroup(items: Snippet[]): Snippet[] {
    return [...items].sort((a, b) => {
      const pa = a.position ?? Number.MAX_SAFE_INTEGER
      const pb = b.position ?? Number.MAX_SAFE_INTEGER
      if (pa !== pb) return pa - pb
      if (a.sort_order !== b.sort_order) return a.sort_order - b.sort_order
      return a.name.localeCompare(b.name)
    })
  }

  // Group macros for the sidebar: ungrouped first, then named folders (sorted).
  const macroGroups = $derived.by(() => {
    const ungrouped: Snippet[] = []
    const byFolder = new Map<string, Snippet[]>()
    for (const m of macroList) {
      if (m.folder) {
        const arr = byFolder.get(m.folder) ?? []
        arr.push(m)
        byFolder.set(m.folder, arr)
      } else {
        ungrouped.push(m)
      }
    }
    const folders = [...byFolder.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([name, items]) => [name, orderGroup(items)] as [string, Snippet[]])
    return { ungrouped: orderGroup(ungrouped), folders }
  })

  // Membership = collapsed (folders are expanded by default).
  let collapsedFolders = $state<Set<string>>(new Set())
  function toggleMacroFolder(name: string) {
    const next = new Set(collapsedFolders)
    if (next.has(name)) next.delete(name)
    else next.add(name)
    collapsedFolders = next
  }

  // ── Drag-and-drop (native HTML5) ────────────────────────────────────────
  /// Session id currently being dragged. `dragOver` is the drop target's id
  /// (`null` = root). Both reset on dragend.
  let draggingSessionId = $state<number | null>(null)
  let dragOver = $state<number | null | 'none'>('none')
  /// When hovering a session row, this records whether the indicator should
  /// be drawn above (before) or below (after) it. Cleared on dragleave/drop.
  let rowDropTarget = $state<{ sessionId: number; position: 'before' | 'after' } | null>(null)

  function onDragStart(e: DragEvent, sessionId: number) {
    draggingSessionId = sessionId
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move'
      // Pass via DataTransfer too so we can survive cross-frame edge cases.
      e.dataTransfer.setData('application/x-zapterm-session', String(sessionId))
    }
  }

  function onDragOver(e: DragEvent, targetFolderId: number | null) {
    if (draggingSessionId == null) return
    e.preventDefault()
    // Folders nest: the innermost one under the cursor is the target.
    e.stopPropagation()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    dragOver = targetFolderId
  }

  /// Per-row dragover handler: pick "insert before" vs "insert after" based
  /// on whether the cursor is in the upper or lower half of the row, and
  /// render an indicator line. Stops propagation so the parent section's
  /// handler doesn't overwrite the visual feedback.
  function onRowDragOver(e: DragEvent, sessionId: number) {
    if (draggingSessionId == null || draggingSessionId === sessionId) return
    e.preventDefault()
    e.stopPropagation()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const position: 'before' | 'after' = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
    if (rowDropTarget?.sessionId !== sessionId || rowDropTarget?.position !== position) {
      rowDropTarget = { sessionId, position }
    }
  }

  function onRowDragLeave(sessionId: number) {
    if (rowDropTarget?.sessionId === sessionId) rowDropTarget = null
  }

  /// Per-row drop: dispatch a reorder request with the explicit target index.
  /// Falls back to `onMove` (legacy) when no reorder consumer is attached.
  function onRowDrop(e: DragEvent, targetFolderId: number | null, targetSessionId: number) {
    if (draggingSessionId == null) return
    e.preventDefault()
    e.stopPropagation()
    const id = draggingSessionId
    const position = rowDropTarget?.position ?? 'after'

    // Compute peers in the destination AFTER removing the dragged row.
    const peers = sessions
      .filter((s) => (s.folder_id ?? null) === targetFolderId && s.id !== id)
      .map((s) => s.id)
    let idx = peers.indexOf(targetSessionId)
    if (idx < 0) idx = peers.length
    const targetIndex = position === 'before' ? idx : idx + 1

    if (onReorder) {
      onReorder(id, targetFolderId, targetIndex)
    } else {
      // Backwards-compat fallback: reparent only.
      const current = sessions.find((s) => s.id === id)?.folder_id ?? null
      if (current !== targetFolderId) onMove?.(id, targetFolderId)
    }
    resetDrag()
  }

  function onDrop(e: DragEvent, targetFolderId: number | null) {
    if (draggingSessionId == null) return
    e.preventDefault()
    e.stopPropagation()
    const id = draggingSessionId
    const current = sessions.find((s) => s.id === id)?.folder_id ?? null
    // Dropped on the section background (not on a specific row) → append.
    if (onReorder) {
      const peers = sessions.filter((s) => (s.folder_id ?? null) === targetFolderId && s.id !== id)
      onReorder(id, targetFolderId, peers.length)
    } else if (current !== targetFolderId) {
      onMove?.(id, targetFolderId)
    }
    resetDrag()
  }

  function onDragEnd() {
    resetDrag()
  }

  function resetDrag() {
    draggingSessionId = null
    dragOver = 'none'
    rowDropTarget = null
  }

  // ── Macro drag-and-drop ─────────────────────────────────────────────────
  // Mirrors the session DnD above but keyed on macro ids and string folder
  // labels (the `Snippet.folder` field), with a distinct DataTransfer type so
  // a macro drag can never be confused with a session drag. `macroDragOver`
  // holds the hovered folder target (`null` = ungrouped, `'none'` = nothing).
  let draggingMacroId = $state<number | null>(null)
  let macroDragOver = $state<string | null | 'none'>('none')
  let macroRowDropTarget = $state<{ macroId: number; position: 'before' | 'after' } | null>(null)

  function findMacro(id: number): Snippet | undefined {
    return macroList.find((m) => m.id === id)
  }

  function onMacroDragStart(e: DragEvent, macroId: number) {
    draggingMacroId = macroId
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move'
      e.dataTransfer.setData('application/x-zapterm-macro', String(macroId))
    }
  }

  function onMacroDragOver(e: DragEvent, targetFolder: string | null) {
    if (draggingMacroId == null) return
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    macroDragOver = targetFolder
  }

  /// Per-row dragover: pick "insert before" vs "insert after" from the cursor
  /// half and render the accent indicator. Stops propagation so the section's
  /// handler doesn't overwrite the feedback.
  function onMacroRowDragOver(e: DragEvent, macroId: number) {
    if (draggingMacroId == null || draggingMacroId === macroId) return
    e.preventDefault()
    e.stopPropagation()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const position: 'before' | 'after' = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
    if (macroRowDropTarget?.macroId !== macroId || macroRowDropTarget?.position !== position) {
      macroRowDropTarget = { macroId, position }
    }
  }

  function onMacroRowDragLeave(macroId: number) {
    if (macroRowDropTarget?.macroId === macroId) macroRowDropTarget = null
  }

  /// Per-row drop. Same-group → reorder within that group via
  /// `onReorderMacros`. Cross-folder → move into the target row's folder via
  /// `onSetMacroFolder` (the backend resets position on folder change).
  function onMacroRowDrop(e: DragEvent, targetFolder: string | null, targetMacroId: number) {
    if (draggingMacroId == null) return
    e.preventDefault()
    e.stopPropagation()
    // Dropping a macro exactly on itself is a no-op.
    if (targetMacroId === draggingMacroId) {
      resetMacroDrag()
      return
    }
    const id = draggingMacroId
    const dragged = findMacro(id)
    const position = macroRowDropTarget?.position ?? 'after'
    const currentFolder = dragged?.folder ?? null

    if (currentFolder === targetFolder) {
      // Reorder within the same group.
      const group = macroList
        .filter((m) => (m.folder ?? null) === targetFolder)
        .sort((a, b) => {
          const pa = a.position ?? Number.MAX_SAFE_INTEGER
          const pb = b.position ?? Number.MAX_SAFE_INTEGER
          if (pa !== pb) return pa - pb
          if (a.sort_order !== b.sort_order) return a.sort_order - b.sort_order
          return a.name.localeCompare(b.name)
        })
      const ids = group.map((m) => m.id).filter((mid) => mid !== id)
      let idx = ids.indexOf(targetMacroId)
      if (idx < 0) idx = ids.length
      const insertAt = position === 'before' ? idx : idx + 1
      ids.splice(insertAt, 0, id)
      onReorderMacros?.(ids)
    } else if (dragged) {
      // Cross-folder move; backend resets position so it lands at the bottom.
      onSetMacroFolder?.(dragged, targetFolder)
    }
    resetMacroDrag()
  }

  /// Drop on the section/folder background → set folder (move) without an
  /// explicit position. `null` = ungrouped.
  function onMacroDrop(e: DragEvent, targetFolder: string | null) {
    if (draggingMacroId == null) return
    e.preventDefault()
    // Folder groups are nested INSIDE the root droparea, so without this a
    // drop on a group header would bubble and re-fire `onMacroDrop(null)`.
    e.stopPropagation()
    const id = draggingMacroId
    const dragged = findMacro(id)
    const currentFolder = dragged?.folder ?? null
    if (dragged && currentFolder !== targetFolder) {
      onSetMacroFolder?.(dragged, targetFolder)
    }
    resetMacroDrag()
  }

  function onMacroDragEnd() {
    resetMacroDrag()
  }

  function resetMacroDrag() {
    draggingMacroId = null
    macroDragOver = 'none'
    macroRowDropTarget = null
  }

  let search = $state('')
  let expandedSections = $state<Set<string>>(new Set(['pinned', 'sessions', 'macros']))
  let expandedFolders = $state<Set<number>>(new Set())

  const SESSION_COLORS = [
    '#22d3ee',
    '#f472b6',
    '#a78bfa',
    '#f59e0b',
    '#ef4444',
    '#10b981',
    '#f97316',
    '#84cc16',
    '#c89b6b',
    '#5eb3b2',
  ]

  function sessionColor(s: SavedSession): string {
    return SESSION_COLORS[s.id % SESSION_COLORS.length] as string
  }

  /// Tags + notes live inside the per-session `options_json` blob (no schema
  /// change). Parse defensively — a malformed blob just yields no metadata.
  function sessionMeta(s: SavedSession): { tags: string[]; notes: string } {
    try {
      const o = JSON.parse(s.options_json || '{}')
      const tags = Array.isArray(o.tags)
        ? o.tags.filter((t: unknown): t is string => typeof t === 'string')
        : []
      return { tags, notes: typeof o.notes === 'string' ? o.notes : '' }
    } catch {
      return { tags: [], notes: '' }
    }
  }

  const query = $derived(search.toLowerCase())

  /// Match a session by name, host, user, tags or notes, so a search like
  /// "prod" surfaces every tagged host and "10.0.0." finds hosts by address.
  function matchesQuery(s: SavedSession): boolean {
    if (!query) return true
    if (s.name.toLowerCase().includes(query)) return true
    if (s.host?.toLowerCase().includes(query)) return true
    if (s.username?.toLowerCase().includes(query)) return true
    const { tags, notes } = sessionMeta(s)
    return tags.some((t) => t.toLowerCase().includes(query)) || notes.toLowerCase().includes(query)
  }

  const rootSessions = $derived(sessions.filter((s) => s.folder_id === null && matchesQuery(s)))

  function sessionsInFolder(id: number): SavedSession[] {
    return sessions.filter((s) => s.folder_id === id && matchesQuery(s))
  }

  // ── Folder tree ──────────────────────────────────────────────────────────
  // Folders nest through `parent_id` (importers create e.g. DC › Spines).
  // Roots are folders without a parent; a folder whose parent is missing, or
  // that sits in a parent loop, is shown at the root rather than lost.
  const roots = $derived(rootFolders(folders))

  function childFolders(id: number): Folder[] {
    return folders.filter((f) => f.parent_id === id && f.id !== id)
  }

  /** Matching sessions in a folder and all its subfolders. */
  function subtreeCount(id: number, seen: Set<number> = new Set()): number {
    if (seen.has(id)) return 0
    seen.add(id)
    let n = sessionsInFolder(id).length
    for (const c of childFolders(id)) n += subtreeCount(c.id, seen)
    return n
  }

  function toggleSection(key: string) {
    const next = new Set(expandedSections)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    expandedSections = next
  }

  function toggleFolder(id: number) {
    const next = new Set(expandedFolders)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    expandedFolders = next
  }
</script>

<aside
  class="sidebar"
  style:width="{width}px"
  style:background={theme.sidebarBg}
  style:border-right="1px solid {theme.border}"
  style:font-family={theme.fontUi}
  style:--item-hover-bg={theme.itemHoverBg}
  style:--text-primary={theme.textPrimary}
  style:--accent={theme.accent}
  style:--err={theme.err}
  style:--radius={theme.radius}
>
  <!-- Search -->
  <div class="sb-search-wrap">
    <div
      class="sb-search-inner"
      style:background={theme.appBg}
      style:border="1px solid {theme.border}"
      style:border-radius={theme.radius}
      style:color={theme.textDim}
    >
      <Icon name="search" size={13} />
      <input
        class="sb-search"
        type="text"
        placeholder="Search sessions…"
        bind:value={search}
        style:color={theme.textPrimary}
        style:font-family={theme.fontUi}
      />
      <kbd class="sb-kbd" style:border="1px solid {theme.border}" style:font-family={theme.fontMono}
        >⌘K</kbd
      >
    </div>
  </div>

  <!-- Tree -->
  <div class="sb-tree">
    <!-- Root sessions. Drop handlers live on the outer .sb-section so
         dropping at root works even when the section is collapsed. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="sb-section"
      class:dragover={dragOver === null && draggingSessionId != null}
      ondragover={(e) => onDragOver(e, null)}
      ondrop={(e) => onDrop(e, null)}
      ondragenter={() => {
        // Auto-expand on hover so the user sees their drop registered.
        if (draggingSessionId != null && !expandedSections.has('sessions')) {
          const next = new Set(expandedSections)
          next.add('sessions')
          expandedSections = next
        }
      }}
    >
      <div class="sb-section-row" style:color={theme.textDim}>
        <button
          class="sb-section-header"
          style:color={theme.textDim}
          onclick={() => toggleSection('sessions')}
        >
          <Icon name="chevron" size={11} open={expandedSections.has('sessions')} />
          SESSIONS
          <span class="sb-count" style:color={theme.textDim}>{rootSessions.length}</span>
        </button>
        {#if onCreateFolder}
          <button
            class="sb-add-btn"
            title="New folder"
            style:color={theme.textDim}
            onclick={onCreateFolder}><Icon name="folder" size={13} /></button
          >
        {/if}
        <button
          class="sb-add-btn"
          title="New session"
          style:color={theme.textDim}
          onclick={onAddSession}><Icon name="plus" size={13} /></button
        >
      </div>

      {#if expandedSections.has('sessions')}
        <div class="sb-droparea">
          {#each rootSessions as s (s.id)}
            {@const color = sessionColor(s)}
            {@const isActive = s.id === activeSessionId}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
            <div
              class="sb-row"
              class:active={isActive}
              class:dragging={draggingSessionId === s.id}
              class:drop-before={rowDropTarget?.sessionId === s.id &&
                rowDropTarget?.position === 'before'}
              class:drop-after={rowDropTarget?.sessionId === s.id &&
                rowDropTarget?.position === 'after'}
              draggable="true"
              role="button"
              tabindex="0"
              ondragstart={(e) => onDragStart(e, s.id)}
              ondragend={onDragEnd}
              ondragover={(e) => onRowDragOver(e, s.id)}
              ondragleave={() => onRowDragLeave(s.id)}
              ondrop={(e) => onRowDrop(e, null, s.id)}
              style:background={isActive ? theme.itemActiveBg : ''}
              onclick={() => onSelect(s)}
            >
              <SessionAvatar
                name={s.name}
                {color}
                paper={theme.sidebarBg}
                ink={theme.textPrimary}
                accent={theme.accent}
                active={isActive}
                status={sessionStatuses?.get(s.id)}
                ok={theme.ok}
                warn={theme.warn}
                err={theme.err}
                protocol={s.protocol}
                size={22}
              />
              <span
                class="sb-name"
                class:active={isActive}
                style:color={isActive ? theme.textPrimary : theme.textMuted}
                title={sessionMeta(s).notes || undefined}>{s.name}</span
              >
              {#each sessionMeta(s).tags.slice(0, 3) as tag (tag)}
                <span
                  class="sb-tag sb-tagchip"
                  style:color={theme.accent}
                  style:border-color="color-mix(in srgb, {theme.accent} 40%, transparent)"
                  style:font-family={theme.fontUi}>{tag}</span
                >
              {/each}
              {#if onEdit}
                <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                <span
                  class="sb-edit"
                  role="button"
                  title="Edit session"
                  style:color={theme.textDim}
                  onclick={(e) => {
                    e.stopPropagation()
                    onEdit?.(s)
                  }}><Icon name="pencil" size={12} /></span
                >
              {/if}
              {#if onDuplicate}
                <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                <span
                  class="sb-edit"
                  role="button"
                  title="Duplicate session"
                  style:color={theme.textDim}
                  onclick={(e) => {
                    e.stopPropagation()
                    onDuplicate?.(s)
                  }}><Icon name="copy" size={12} /></span
                >
              {/if}
              {#if onDelete}
                <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                <span
                  class="sb-edit sb-del"
                  role="button"
                  title="Delete session"
                  style:color={theme.textDim}
                  onclick={(e) => {
                    e.stopPropagation()
                    onDelete?.(s)
                  }}><Icon name="x" size={12} /></span
                >
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Folders: a tree through parent_id, drawn by the recursive snippet. -->
    {#snippet folderNode(folder: Folder, depth: number)}
      {@const folderSessions = sessionsInFolder(folder.id)}
      {@const subfolders = childFolders(folder.id)}
      {@const total = subtreeCount(folder.id)}
      {#if !query || total > 0}
        <div
          class="sb-section"
          class:dragover={dragOver === folder.id && draggingSessionId != null}
          ondragover={(e) => onDragOver(e, folder.id)}
          ondrop={(e) => onDrop(e, folder.id)}
          ondragenter={() => {
            // Auto-expand on hover so the user can see the drop is registering.
            if (draggingSessionId != null && !expandedFolders.has(folder.id)) {
              const next = new Set(expandedFolders)
              next.add(folder.id)
              expandedFolders = next
            }
          }}
          role="group"
        >
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
          <div
            class="sb-folder-header"
            role="button"
            tabindex="0"
            style:color={theme.textDim}
            style:padding-left="{14 + depth * 14}px"
            onclick={() => toggleFolder(folder.id)}
          >
            <Icon name="chevron" size={11} open={!!query || expandedFolders.has(folder.id)} />
            <Icon name="folder" size={13} />
            <span class="sb-folder-name">{folder.name.toUpperCase()}</span>
            <span class="sb-count" style:color={theme.textDim}>{total}</span>
            {#if onCreateSubfolder}
              <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
              <span
                class="sb-edit"
                role="button"
                title="New subfolder"
                style:color={theme.textDim}
                onclick={(e) => {
                  e.stopPropagation()
                  onCreateSubfolder?.(folder)
                }}><Icon name="plus" size={12} /></span
              >
            {/if}
            {#if onRenameFolder}
              <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
              <span
                class="sb-edit"
                role="button"
                title="Rename folder"
                style:color={theme.textDim}
                onclick={(e) => {
                  e.stopPropagation()
                  onRenameFolder?.(folder)
                }}><Icon name="pencil" size={12} /></span
              >
            {/if}
            {#if onDeleteFolder}
              <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
              <span
                class="sb-edit sb-del"
                role="button"
                title="Delete folder"
                style:color={theme.textDim}
                onclick={(e) => {
                  e.stopPropagation()
                  onDeleteFolder?.(folder)
                }}><Icon name="x" size={12} /></span
              >
            {/if}
          </div>

          <!-- While searching, a folder with matches shows them: collapsed, it
               used to show only a count and hide the sessions found. -->
          {#if query || expandedFolders.has(folder.id)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="sb-droparea"
              class:dragover={dragOver === folder.id && draggingSessionId != null}
              ondragover={(e) => onDragOver(e, folder.id)}
              ondrop={(e) => onDrop(e, folder.id)}
            >
              {#each folderSessions as s (s.id)}
                {@const color = sessionColor(s)}
                {@const isActive = s.id === activeSessionId}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
                <div
                  class="sb-row sb-row-indented"
                  class:active={isActive}
                  class:dragging={draggingSessionId === s.id}
                  class:drop-before={rowDropTarget?.sessionId === s.id &&
                    rowDropTarget?.position === 'before'}
                  class:drop-after={rowDropTarget?.sessionId === s.id &&
                    rowDropTarget?.position === 'after'}
                  draggable="true"
                  role="button"
                  tabindex="0"
                  ondragstart={(e) => onDragStart(e, s.id)}
                  ondragend={onDragEnd}
                  ondragover={(e) => onRowDragOver(e, s.id)}
                  ondragleave={() => onRowDragLeave(s.id)}
                  ondrop={(e) => onRowDrop(e, folder.id, s.id)}
                  style:background={isActive ? theme.itemActiveBg : ''}
                  onclick={() => onSelect(s)}
                >
                  <SessionAvatar
                    name={s.name}
                    {color}
                    paper={theme.sidebarBg}
                    ink={theme.textPrimary}
                    accent={theme.accent}
                    active={isActive}
                    status={sessionStatuses?.get(s.id)}
                    ok={theme.ok}
                    warn={theme.warn}
                    err={theme.err}
                    protocol={s.protocol}
                    size={19}
                  />
                  <span
                    class="sb-name sb-name-sm"
                    class:active={isActive}
                    style:color={isActive ? theme.textPrimary : theme.textMuted}
                    title={sessionMeta(s).notes || undefined}>{s.name}</span
                  >
                  {#each sessionMeta(s).tags.slice(0, 3) as tag (tag)}
                    <span
                      class="sb-tag sb-tagchip"
                      style:color={theme.accent}
                      style:border-color="color-mix(in srgb, {theme.accent} 40%, transparent)"
                      style:font-family={theme.fontUi}>{tag}</span
                    >
                  {/each}
                  {#if onEdit}
                    <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                    <span
                      class="sb-edit"
                      role="button"
                      title="Edit session"
                      style:color={theme.textDim}
                      onclick={(e) => {
                        e.stopPropagation()
                        onEdit?.(s)
                      }}><Icon name="pencil" size={12} /></span
                    >
                  {/if}
                  {#if onDuplicate}
                    <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                    <span
                      class="sb-edit"
                      role="button"
                      title="Duplicate session"
                      style:color={theme.textDim}
                      onclick={(e) => {
                        e.stopPropagation()
                        onDuplicate?.(s)
                      }}><Icon name="copy" size={12} /></span
                    >
                  {/if}
                  {#if onDelete}
                    <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
                    <span
                      class="sb-edit sb-del"
                      role="button"
                      title="Delete session"
                      style:color={theme.textDim}
                      onclick={(e) => {
                        e.stopPropagation()
                        onDelete?.(s)
                      }}><Icon name="x" size={12} /></span
                    >
                  {/if}
                </div>
              {/each}
              {#each subfolders as child (child.id)}
                {#if depth < 16}
                  {@render folderNode(child, depth + 1)}
                {/if}
              {/each}
              {#if folderSessions.length === 0 && subfolders.length === 0}
                <span class="sb-empty" style:color={theme.textDim}>empty — drop a session here</span
                >
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    {/snippet}
    {#each roots as folder (folder.id)}
      {@render folderNode(folder, 0)}
    {/each}

    {#if sessions.length === 0 && !search}
      <p class="sb-hint" style:color={theme.textDim}>No sessions yet.</p>
    {/if}

    {#snippet macroRow(m: Snippet, folder: string | null)}
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
      <div
        class="sb-row"
        class:dragging={draggingMacroId === m.id}
        class:drop-before={macroRowDropTarget?.macroId === m.id &&
          macroRowDropTarget?.position === 'before'}
        class:drop-after={macroRowDropTarget?.macroId === m.id &&
          macroRowDropTarget?.position === 'after'}
        role="button"
        tabindex="0"
        draggable="true"
        title="Run on the focused session"
        ondragstart={(e) => onMacroDragStart(e, m.id)}
        ondragend={onMacroDragEnd}
        ondragover={(e) => onMacroRowDragOver(e, m.id)}
        ondragleave={() => onMacroRowDragLeave(m.id)}
        ondrop={(e) => onMacroRowDrop(e, folder, m.id)}
        onclick={() => onRunMacro?.(m)}
      >
        <span class="sb-macro-bolt" style:color={m.color ?? theme.accent}>
          <Icon name="bolt" size={13} />
        </span>
        <span class="sb-name" style:color={theme.textMuted}>{m.name}</span>
        {#if onCloneMacro}
          <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
          <span
            class="sb-edit"
            role="button"
            title="Clone macro"
            style:color={theme.textDim}
            onclick={(e) => {
              e.stopPropagation()
              onCloneMacro?.(m)
            }}><Icon name="copy" size={12} /></span
          >
        {/if}
        {#if onExportMacro}
          <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
          <span
            class="sb-edit"
            role="button"
            title="Export macro (JSON)"
            style:color={theme.textDim}
            onclick={(e) => {
              e.stopPropagation()
              onExportMacro?.(m)
            }}><Icon name="upload" size={12} /></span
          >
        {/if}
        {#if onEditMacro}
          <!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
          <span
            class="sb-edit"
            role="button"
            title="Edit macro"
            style:color={theme.textDim}
            onclick={(e) => {
              e.stopPropagation()
              onEditMacro?.(m)
            }}><Icon name="pencil" size={12} /></span
          >
        {/if}
      </div>
    {/snippet}

    <!-- Macros: a library of saved macros (snippets with steps), run on the
         focused session with one click. Lives below the sessions. -->
    {#if !search && (macroList.length > 0 || onNewMacro)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="sb-section"
        class:dragover={macroDragOver === null && draggingMacroId != null}
        ondragover={(e) => onMacroDragOver(e, null)}
        ondrop={(e) => onMacroDrop(e, null)}
      >
        <div class="sb-section-row" style:color={theme.textDim}>
          <button
            class="sb-section-header"
            style:color={theme.textDim}
            onclick={() => toggleSection('macros')}
          >
            <Icon name="chevron" size={11} open={expandedSections.has('macros')} />
            MACROS
            <span class="sb-count" style:color={theme.textDim}>{macroList.length}</span>
          </button>
          {#if onExportMacros && macroList.length > 0}
            <button
              class="sb-add-btn sb-secondary"
              title="Export all macros to a JSON file"
              style:color={theme.textDim}
              onclick={onExportMacros}><Icon name="upload" size={13} /></button
            >
          {/if}
          {#if onImportMacrosJson}
            <button
              class="sb-add-btn sb-secondary"
              title="Import macros from a JSON file"
              style:color={theme.textDim}
              onclick={onImportMacrosJson}><Icon name="download" size={13} /></button
            >
          {/if}
          {#if onImportMacro}
            <button
              class="sb-add-btn sb-secondary"
              title="Import from a MobaXterm macro"
              style:color={theme.textDim}
              onclick={onImportMacro}><Icon name="transfer" size={13} /></button
            >
          {/if}
          {#if onNewMacro}
            <button
              class="sb-add-btn"
              title="New macro"
              style:color={theme.textDim}
              onclick={onNewMacro}><Icon name="plus" size={13} /></button
            >
          {/if}
        </div>
        {#if expandedSections.has('macros')}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="sb-droparea"
            class:dragover={macroDragOver === null && draggingMacroId != null}
            ondragover={(e) => onMacroDragOver(e, null)}
            ondrop={(e) => onMacroDrop(e, null)}
          >
            {#each macroGroups.ungrouped as m (m.id)}
              {@render macroRow(m, null)}
            {/each}
            {#each macroGroups.folders as [folderName, items] (folderName)}
              <div
                class="sb-macro-folder-group"
                class:dragover={macroDragOver === folderName && draggingMacroId != null}
                ondragover={(e) => onMacroDragOver(e, folderName)}
                ondrop={(e) => onMacroDrop(e, folderName)}
                role="group"
              >
                <button
                  class="sb-folder-row"
                  style:color={theme.textDim}
                  onclick={() => toggleMacroFolder(folderName)}
                  ondragenter={() => {
                    // Auto-expand on hover so the drop is visibly registered.
                    if (draggingMacroId != null && collapsedFolders.has(folderName)) {
                      const next = new Set(collapsedFolders)
                      next.delete(folderName)
                      collapsedFolders = next
                    }
                  }}
                >
                  <Icon name="chevron" size={10} open={!collapsedFolders.has(folderName)} />
                  <Icon name="folder" size={12} />
                  <span class="sb-folder-name">{folderName}</span>
                  <span class="sb-count" style:color={theme.textDim}>{items.length}</span>
                </button>
                {#if !collapsedFolders.has(folderName)}
                  <div class="sb-folder-items">
                    {#each items as m (m.id)}
                      {@render macroRow(m, folderName)}
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
            {#if macroList.length === 0}
              <span class="sb-empty" style:color={theme.textDim}
                >No macros yet — click + to add</span
              >
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Footer: user chip + theme + settings -->
  <div class="sb-footer" style:border-top="1px solid {theme.border}">
    <span class="sb-footer-spacer"></span>
    {#if onOpenLogs}
      <button
        class="sb-settings-btn"
        title="Open logs folder"
        aria-label="Open logs folder"
        onclick={onOpenLogs}
        style:color={theme.textDim}
      >
        <Icon name="folder" size={14} />
      </button>
    {/if}
    {#if onOpenVault}
      <button
        class="sb-settings-btn"
        title="Credential vault"
        onclick={onOpenVault}
        style:color={theme.textDim}
      >
        <Icon name="key" size={14} />
      </button>
    {/if}
    <button
      class="sb-settings-btn"
      title="Cycle theme"
      onclick={onToggleTheme}
      style:color={theme.textDim}
    >
      <Icon name="contrast" size={14} />
    </button>
    <button
      class="sb-settings-btn"
      title="Settings"
      onclick={onSettings}
      style:color={theme.textDim}
    >
      <Icon name="gear" size={14} />
    </button>
  </div>
</aside>

<style>
  .sidebar {
    /* Width is driven by an inline style from the resizable divider; 248px
       is the default when no prop is supplied. */
    width: 248px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sb-search-wrap {
    padding: 12px 12px 8px;
    flex-shrink: 0;
  }

  .sb-search-inner {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 10px;
  }

  .sb-search {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 12.5px;
    min-width: 0;
  }

  .sb-search::placeholder {
    color: inherit;
    opacity: 0.85;
  }

  .sb-kbd {
    font-size: 10.5px;
    border-radius: 4px;
    padding: 1px 5px;
    line-height: 1.4;
    flex-shrink: 0;
  }

  .sb-tree {
    flex: 1;
    overflow-y: auto;
    padding: 2px 0 4px;
  }

  .sb-tree::-webkit-scrollbar {
    width: 4px;
  }
  .sb-tree::-webkit-scrollbar-track {
    background: transparent;
  }
  .sb-tree::-webkit-scrollbar-thumb {
    background: rgba(127, 127, 127, 0.25);
    border-radius: 2px;
  }

  .sb-section {
    margin-bottom: 2px;
  }

  .sb-section-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px 4px;
  }

  .sb-add-btn {
    background: none;
    border: none;
    cursor: pointer;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    padding: 0;
    flex-shrink: 0;
    transition:
      background 0.1s,
      color 0.1s;
    color: inherit;
  }

  .sb-add-btn:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.06));
    color: var(--text-primary, #2c2924);
  }

  .sb-section-header {
    flex: 1;
    width: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 1px;
    text-transform: uppercase;
    font-family: inherit;
    user-select: none;
    color: inherit;
  }

  .sb-folder-header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 14px;
    cursor: pointer;
    user-select: none;
  }

  .sb-folder-name {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.8px;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Reveal rename/delete icons on hover over the folder row. */
  .sb-folder-header:hover .sb-edit {
    opacity: 0.6;
  }

  .sb-count {
    margin-left: auto;
    font-size: 10.5px;
    font-weight: 400;
    letter-spacing: 0;
  }

  .sb-section-header .sb-count {
    margin-left: 2px;
  }

  /* A narrow sidebar ellipses the title rather than pushing the buttons out. */
  .sb-section-header {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }

  /* Import / export are occasional: shown on hover so the header stays clean;
     "+" is always visible. */
  .sb-section-row .sb-secondary {
    display: none;
  }
  .sb-section-row:hover .sb-secondary,
  .sb-section-row:focus-within .sb-secondary {
    display: flex;
  }

  .sb-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    height: 34px;
    background: none;
    border: none;
    border-radius: var(--radius, 7px);
    cursor: pointer;
    font-family: inherit;
    text-align: left;
    transition: background 0.1s;
    position: relative;
  }

  .sb-row:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.04)) !important;
  }

  .sb-row-indented {
    padding-left: 22px;
    height: 32px;
  }

  /* Drop indicators — a thin accent rule above (before) or below (after)
     the hovered row. Pure CSS, no extra DOM. */
  .sb-row.drop-before::before,
  .sb-row.drop-after::after {
    content: '';
    position: absolute;
    left: 8px;
    right: 8px;
    height: 2px;
    background: var(--accent);
    border-radius: 2px;
    pointer-events: none;
  }
  .sb-row.drop-before::before {
    top: -1px;
  }
  .sb-row.drop-after::after {
    bottom: -1px;
  }

  .sb-name {
    flex: 1;
    font-size: 12px;
    font-weight: 400;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color 0.1s;
  }

  .sb-name.active {
    font-weight: 500;
  }

  .sb-name-sm {
    font-size: 11.5px;
  }

  .sb-droparea {
    display: flex;
    flex-direction: column;
    border: 1px dashed transparent;
    border-radius: var(--radius, 7px);
    transition:
      border-color 0.1s,
      background 0.1s;
    padding: 1px;
    margin: 0 8px;
  }

  .sb-droparea.dragover {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  /* Highlight the whole folder section when dragging over a collapsed
     folder so it's obvious you can drop on the header too. */
  .sb-section.dragover {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    border-radius: var(--radius, 7px);
  }

  .sb-row.dragging {
    opacity: 0.5;
  }

  .sb-edit {
    margin-left: 0;
    padding: 2px 3px;
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
    display: inline-flex;
    align-items: center;
    transition:
      opacity 0.1s,
      background 0.1s;
  }
  /* Row action icons take no room until the row is hovered / active, so
     names use the full width instead of ellipsing next to invisible icons. */
  .sb-row .sb-edit {
    display: none;
  }
  .sb-row:hover .sb-edit,
  .sb-row.active .sb-edit,
  .sb-row:focus-within .sb-edit {
    display: inline-flex;
    opacity: 0.6;
  }
  .sb-edit:hover {
    opacity: 1 !important;
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.08));
  }
  .sb-del:hover {
    color: var(--err) !important;
    background: color-mix(in srgb, var(--err) 12%, transparent) !important;
  }

  /* Tags yield to the action icons on hover/active. (Protocol now lives as a
     glyph on the avatar, so only user tag chips remain here.) */
  .sb-row:hover .sb-tag,
  .sb-row.active .sb-tag {
    display: none;
  }

  .sb-tag {
    font-size: 9.5px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 5px;
    border: 1px solid;
    border-radius: 4px;
    flex-shrink: 0;
    line-height: 1.4;
  }

  /* User tags: lower-case words, accent-tinted, capped width. Inherit the
     hide-on-hover from `.sb-tag` so the action icons get room. */
  .sb-tagchip {
    text-transform: none;
    letter-spacing: 0;
    font-size: 10px;
    border-radius: 5px;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sb-empty {
    display: block;
    padding: 4px 22px;
    font-size: 11px;
    font-style: italic;
  }

  .sb-macro-bolt {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
  }

  /* Highlight a macro folder group when dragging a macro over its header so
     it's clear the drop will reparent into that folder. Mirrors the session
     `.sb-section.dragover` treatment. */
  .sb-macro-folder-group.dragover {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    border-radius: var(--radius, 7px);
  }

  .sb-folder-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 4px 10px 4px 16px;
    font-family: inherit;
    font-size: 11.5px;
    text-align: left;
  }
  .sb-folder-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sb-folder-items {
    padding-left: 10px;
  }

  .sb-hint {
    font-size: 11px;
    padding: 12px 14px;
    margin: 0;
    line-height: 1.6;
  }

  .sb-footer {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 14px;
    flex-shrink: 0;
  }

  .sb-footer-spacer {
    flex: 1;
  }

  .sb-settings-btn {
    background: none;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border-radius: 4px;
    transition:
      background 0.1s,
      color 0.1s;
    color: inherit;
  }

  .sb-settings-btn:hover {
    background: var(--item-hover-bg, rgba(255, 255, 255, 0.06));
    color: var(--text-primary, #2c2924);
  }
</style>
