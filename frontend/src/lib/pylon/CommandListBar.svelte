<script lang="ts">
  /// Bottom bar: the detected vendor's command lists, then "Recent". Each chip opens a
  /// panel with the list's commands, grouped and searchable. A command runs
  /// on click (Shift+click types it without Enter); one ending in a
  /// `<placeholder>` is typed up to it and waits; other placeholders and
  /// recipes (multi-line debugs) ask for their values in a small form first.
  import { ask } from '@tauri-apps/plugin-dialog'
  import { sendInputText, writeUserCmdlist } from '$lib/bridge/commands'
  import { errorText } from '$lib/bridge/errors'
  import { barContext, recents, loadRecents } from '$lib/stores/snippets.svelte'
  import Icon from '$lib/icons/Icon.svelte'
  import type { RecentCommand } from '$lib/bridge/types'
  import {
    getFocusedSessionId,
    focusSession,
    broadcast,
    broadcastTargets,
    noteOutgoing,
  } from '$lib/stores/sessionRuntime.svelte'
  import { terminalSettings } from '$lib/stores/settings.svelte'
  import { showToast } from '$lib/ui/toast-store.svelte'
  import {
    listsForPlatform,
    commandMode,
    commandParts,
    typedPrefix,
    placeholderNames,
    fillCommand,
    fillRecipe,
    itemMatches,
    type CommandItem,
    type CommandList,
    type ListItem,
    type ListIcon,
    type RecipeItem,
  } from '$lib/cmdlists'
  import { userLists } from '$lib/cmdlists/userLists.svelte'
  import { parseVendorLists } from '$lib/cmdlists'
  import {
    CUSTOM_ID,
    CUSTOM_PLATFORM,
    customCommand,
    customFileText,
    customList,
    hasCommand,
    withItem,
    withoutItem,
    type ItemAt,
  } from '$lib/cmdlists/custom'
  import type { PylonTheme } from '$lib/themes/index'

  interface Props {
    theme: PylonTheme
  }
  let { theme }: Props = $props()

  const vendor = $derived(listsForPlatform(barContext.platform, userLists.lists))

  // ── Open panel ────────────────────────────────────────────────────────
  let openId = $state<string | null>(null)
  let query = $state('')
  let activeIndex = $state(0)
  let rowEl = $state<HTMLElement | null>(null)
  let panelEl = $state<HTMLElement | null>(null)
  let searchEl = $state<HTMLInputElement | null>(null)
  let panelLeft = $state(0)

  /// The form shown instead of the list: a recipe, or a command whose
  /// placeholders must be filled before it can be sent.
  type Form =
    | { kind: 'recipe'; item: RecipeItem; values: Record<string, string> }
    | { kind: 'command'; item: CommandItem; values: Record<string, string> }
  let form = $state<Form | null>(null)

  /// The user's own commands, the same on every platform.
  const custom = $derived(customList(userLists.lists))
  const customOpen = $derived(openId === CUSTOM_ID)
  const openList = $derived(
    customOpen ? custom : (vendor?.lists.find((l) => l.id === openId) ?? null),
  )

  /// Sections filtered by the search box, and the same items flat (for ↑↓).
  const filtered = $derived.by(() => {
    if (!openList) return []
    return openList.sections
      .map((s) => ({ title: s.title, items: s.items.filter((i) => itemMatches(i, query)) }))
      .filter((s) => s.items.length > 0)
  })
  const flat = $derived(filtered.flatMap((s) => s.items))

  function count(list: CommandList): number {
    return list.sections.reduce((n, s) => n + s.items.length, 0)
  }

  function toggle(list: CommandList, chip: HTMLElement) {
    if (openId === list.id) {
      close()
      return
    }
    openId = list.id
    query = ''
    activeIndex = 0
    form = null
    // Anchor the panel to the chip, kept inside the row.
    const row = rowEl?.getBoundingClientRect()
    const c = chip.getBoundingClientRect()
    if (row) {
      const width = list.debug ? 720 : 600
      panelLeft = Math.max(0, Math.min(c.left - row.left, row.width - width - 8))
    }
    queueMicrotask(() => searchEl?.focus())
  }

  function close() {
    openId = null
    form = null
    edit = null
  }

  // ── Custom list ───────────────────────────────────────────────────────
  /// The add/edit form of the custom panel (`at` null = adding).
  let edit = $state<{ at: ItemAt | null; cmd: string; desc: string } | null>(null)
  let customError = $state('')

  function toggleCustom() {
    if (customOpen) {
      close()
      return
    }
    recentOpen = false
    openId = CUSTOM_ID
    query = ''
    activeIndex = 0
    form = null
    edit = null
    customError = ''
    const row = rowEl?.getBoundingClientRect()
    if (row) panelLeft = Math.max(0, row.width - 600 - 8)
    queueMicrotask(() => searchEl?.focus())
  }

  /// Where an item of the custom list is (the rows show the same objects).
  function customPos(item: ListItem): ItemAt | null {
    for (const [si, s] of custom.sections.entries()) {
      const ii = s.items.indexOf(item)
      if (ii >= 0) return { si, ii }
    }
    return null
  }

  async function saveCustom(next: CommandList): Promise<boolean> {
    // A hand-edited custom.json with a mistake is shown as empty: saving now
    // would overwrite it and lose the user's commands.
    const broken = userLists.errors[CUSTOM_PLATFORM]
    if (broken) {
      customError = `custom.json has an error, fix it and press Reload in Settings → Command lists: ${broken}`
      showToast({ kind: 'error', title: 'custom.json has an error', detail: broken })
      return false
    }
    const text = customFileText(next)
    try {
      await writeUserCmdlist(CUSTOM_PLATFORM, text)
    } catch (e) {
      customError = `Couldn't save: ${errorText(e)}`
      return false
    }
    const r = parseVendorLists(text, CUSTOM_PLATFORM)
    if (r.ok) userLists.lists = { ...userLists.lists, [CUSTOM_PLATFORM]: r.value }
    customError = ''
    return true
  }

  function startEdit(item: ListItem | null) {
    const at = item ? customPos(item) : null
    edit =
      item && item.kind === 'command'
        ? { at, cmd: item.cmd, desc: item.desc }
        : { at: null, cmd: '', desc: '' }
  }

  async function submitEdit() {
    if (!edit) return
    const item = customCommand(edit.cmd, edit.desc)
    if (!item) return
    if (await saveCustom(withItem(custom, edit.at, item))) edit = null
  }

  async function removeCustom(item: ListItem) {
    const at = customPos(item)
    if (!at) return
    const label = item.kind === 'command' ? item.cmd : item.name
    const yes = await ask(`Remove this from Custom?\n\n${label}`, {
      title: 'Remove from Custom',
      kind: 'warning',
    })
    if (yes) await saveCustom(withoutItem(custom, at))
  }

  /// ☆ on a Recent entry: keep it in the custom list.
  async function saveRecent(r: RecentCommand) {
    const item = customCommand(r.text)
    if (!item) return
    if (hasCommand(custom, item.cmd)) {
      showToast({ kind: 'info', title: 'Already in Custom', detail: item.cmd })
      return
    }
    if (await saveCustom(withItem(custom, null, item)))
      showToast({ kind: 'success', title: 'Saved to Custom', detail: item.cmd })
  }

  // Close on a click outside the row/panel.
  $effect(() => {
    if (!openId) return
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node
      if (rowEl?.contains(t) || panelEl?.contains(t)) return
      close()
    }
    window.addEventListener('pointerdown', onDown, true)
    return () => window.removeEventListener('pointerdown', onDown, true)
  })

  // ── Debug state (per session) ─────────────────────────────────────────
  /// Sessions where a debug recipe was started and not stopped from here.
  let debugging = $state<Record<string, boolean>>({})
  const focusedDebugging = $derived.by(() => {
    // Re-read when the focus changes: barContext follows the focused session.
    void barContext.savedSessionId
    const id = getFocusedSessionId()
    return id ? !!debugging[id] : false
  })

  // ── Sending ───────────────────────────────────────────────────────────
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

  /// Send lines (each followed by Enter, unless `noEnter` for the last one)
  /// to the focused session, and to every broadcast target. Lines go one by
  /// one with a short pause: a serial console at 9600 baud can drop input
  /// pasted all at once.
  async function send(lines: string[], noEnter = false): Promise<boolean> {
    const focused = getFocusedSessionId()
    if (!focused) {
      showToast({ kind: 'error', title: 'No session focused', detail: 'Click a terminal first.' })
      return false
    }
    const targets = broadcast.enabled ? [focused, ...broadcastTargets(focused)] : [focused]
    try {
      for (let i = 0; i < lines.length; i++) {
        const last = i === lines.length - 1
        const text = lines[i] + (last && noEnter ? '' : '\n')
        for (const id of targets) {
          noteOutgoing(id, new TextEncoder().encode(text))
          await sendInputText(id, text)
        }
        if (!last) await sleep(150)
      }
      focusSession(focused)
      return true
    } catch (e) {
      showToast({ kind: 'error', title: 'Send failed', detail: errorText(e) })
      return false
    }
  }

  /// Send raw keys (e.g. Ctrl+C) as they are, no Enter.
  async function sendKeys(keys: string): Promise<boolean> {
    const focused = getFocusedSessionId()
    if (!focused) {
      showToast({ kind: 'error', title: 'No session focused', detail: 'Click a terminal first.' })
      return false
    }
    const targets = broadcast.enabled ? [focused, ...broadcastTargets(focused)] : [focused]
    try {
      for (const id of targets) {
        noteOutgoing(id, new TextEncoder().encode(keys))
        await sendInputText(id, keys)
      }
      focusSession(focused)
      return true
    } catch (e) {
      showToast({ kind: 'error', title: 'Send failed', detail: errorText(e) })
      return false
    }
  }

  async function confirmAction(text: string): Promise<boolean> {
    return ask(`This changes state on the device:\n\n${text}\n\nSend it?`, {
      title: 'Confirm command',
      kind: 'warning',
    })
  }

  // ── Shift tracking ────────────────────────────────────────────────────
  /// Shift+click types without Enter. `click.shiftKey` alone isn't relied
  /// on (it came through unset on Windows/WebView2): Shift is also tracked
  /// from the keyboard and read again when the button is pressed.
  let shiftHeld = false
  let shiftAtPress = false
  function trackShift(e: KeyboardEvent) {
    if (e.key === 'Shift') shiftHeld = e.type === 'keydown'
  }
  function clearShift() {
    shiftHeld = false
  }
  function notePress(e: PointerEvent) {
    shiftAtPress = e.shiftKey || shiftHeld
  }
  /// Whether this click is a Shift+click; resets the press state.
  function shiftClick(e: MouseEvent): boolean {
    const shift = e.shiftKey || shiftHeld || shiftAtPress
    shiftAtPress = false
    return shift
  }

  /// Plain commands get a visible "Type" button (types without Enter). The
  /// others already stop before Enter or open a form with its own button.
  function canTypeOnly(item: ListItem): boolean {
    return item.kind === 'command' && !item.keys && commandMode(item) === 'run'
  }

  async function pick(item: ListItem, typeOnly = false) {
    if (item.kind === 'recipe') {
      const values: Record<string, string> = {}
      for (const f of item.fields) values[f.key] = f.default ?? ''
      form = { kind: 'recipe', item, values }
      return
    }
    if (item.keys) {
      close()
      await sendKeys(item.keys)
      return
    }
    const mode = commandMode(item)
    if (mode === 'ask') {
      const values: Record<string, string> = {}
      for (const n of placeholderNames(item.cmd)) values[n] = ''
      form = { kind: 'command', item, values }
      return
    }
    if (mode === 'type') {
      // Typed up to the placeholder; the user finishes it in the terminal.
      close()
      await send([typedPrefix(item)], true)
      return
    }
    if (item.action && !typeOnly && !(await confirmAction(item.cmd))) return
    close()
    await send([item.cmd], typeOnly)
  }

  const formLines = $derived.by(() => {
    if (!form) return []
    return form.kind === 'recipe'
      ? fillRecipe(form.item, form.values)
      : [fillCommand(form.item.cmd, form.values)]
  })
  const formReady = $derived(!!form && Object.values(form.values).every((v) => v.trim() !== ''))

  async function submitForm(typeOnly = false) {
    if (!form || !formReady) return
    const lines = formLines
    const isAction = form.kind === 'command' && form.item.action
    const isDebugRecipe = form.kind === 'recipe' && !!openList?.debug
    if (isAction && !typeOnly && !(await confirmAction(lines.join('\n')))) return
    close()
    const ok = await send(lines, typeOnly)
    if (ok && isDebugRecipe && !typeOnly) {
      const id = getFocusedSessionId()
      if (id) debugging = { ...debugging, [id]: true }
    }
  }

  async function stopDebug() {
    if (!vendor) return
    // FortiOS: Ctrl+C first — a sniffer or flow trace still printing holds
    // the prompt, and the disable/reset lines would be lost in its output.
    if (vendor.stopKeys) {
      if (!(await sendKeys(vendor.stopKeys))) return
      await sleep(150)
    }
    if (await send(vendor.stopDebug)) {
      const id = getFocusedSessionId()
      if (id) {
        const next = { ...debugging }
        delete next[id]
        debugging = next
      }
    }
  }

  // ── Recent commands ───────────────────────────────────────────────────
  /// Reloaded on open: the list otherwise only refreshes when the focused
  /// session changes, so it would miss what was just run.
  let recentOpen = $state(false)
  let recentWrap = $state<HTMLElement | null>(null)

  function toggleRecent() {
    recentOpen = !recentOpen
    if (recentOpen) {
      close()
      void loadRecents()
    }
  }

  /// Click runs it again; Shift+click types it without Enter (to edit it
  /// first), like the list items.
  async function pickRecent(r: RecentCommand, typeOnly = false) {
    recentOpen = false
    await send([r.text.replace(/\n+$/, '')], typeOnly)
  }

  $effect(() => {
    if (!recentOpen) return
    const onDown = (e: PointerEvent) => {
      if (recentWrap && !recentWrap.contains(e.target as Node)) recentOpen = false
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') recentOpen = false
    }
    window.addEventListener('pointerdown', onDown, true)
    window.addEventListener('keydown', onKey, true)
    return () => {
      window.removeEventListener('pointerdown', onDown, true)
      window.removeEventListener('keydown', onKey, true)
    }
  })

  // ── Keyboard (search box and form) ────────────────────────────────────
  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      close()
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      activeIndex = Math.min(activeIndex + 1, Math.max(flat.length - 1, 0))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      activeIndex = Math.max(activeIndex - 1, 0)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const item = flat[activeIndex]
      if (item) void pick(item, e.shiftKey)
    }
  }

  function onFormKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      form = null
      queueMicrotask(() => searchEl?.focus())
    } else if (e.key === 'Enter') {
      e.preventDefault()
      void submitForm(e.shiftKey)
    }
  }

  // Keep the highlighted item in range as the filter changes.
  $effect(() => {
    if (activeIndex >= flat.length) activeIndex = Math.max(flat.length - 1, 0)
  })
</script>

<svelte:window onkeydown={trackShift} onkeyup={trackShift} onblur={clearShift} />

{#snippet icon(name: ListIcon, size: number)}
  <svg
    width={size}
    height={size}
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {#if name === 'system'}
      <circle cx="8" cy="8" r="2.2" />
      <path
        d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"
      />
    {:else if name === 'interfaces'}
      <rect x="2" y="5" width="12" height="7" rx="1.5" />
      <path d="M5 8h1M8 8h1M11 8h1" />
    {:else if name === 'routing'}
      <circle cx="3.5" cy="12.5" r="1.5" />
      <circle cx="12.5" cy="3.5" r="1.5" />
      <path d="M5 12.5h4a2.5 2.5 0 0 0 0-5H7a2.5 2.5 0 0 1 0-5h4" />
    {:else if name === 'bgp'}
      <circle cx="3.5" cy="8" r="1.8" />
      <circle cx="12.5" cy="3.5" r="1.8" />
      <circle cx="12.5" cy="12.5" r="1.8" />
      <path d="M5.2 7.2l5.6-2.8M5.2 8.8l5.6 2.8" />
    {:else if name === 'ospf'}
      <path d="M8 1.8l5.4 3.1v6.2L8 14.2l-5.4-3.1V4.9z" />
    {:else if name === 'vpn'}
      <rect x="3" y="7" width="10" height="7" rx="1.5" />
      <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
    {:else if name === 'sdwan'}
      <path d="M2 8h3l2-4h2l2 4h3M2 8h3l2 4h2l2-4" />
    {:else if name === 'ha'}
      <rect x="2" y="3" width="5" height="10" rx="1.2" />
      <rect x="9" y="3" width="5" height="10" rx="1.2" />
      <path d="M7 6.5h2M7 9.5h2" />
    {:else if name === 'security'}
      <path d="M8 1.8l5 2v4c0 3.2-2.1 5.4-5 6.4-2.9-1-5-3.2-5-6.4v-4z" />
      <path d="M5.8 8l1.6 1.6L10.4 6.6" />
    {:else if name === 'users'}
      <circle cx="6" cy="5.5" r="2.2" />
      <path d="M2 13c.4-2.3 2-3.6 4-3.6s3.6 1.3 4 3.6" />
      <circle cx="11.5" cy="6" r="1.7" />
      <path d="M11 9.5c1.6 0 2.7 1 3 2.8" />
    {:else if name === 'logs'}
      <rect x="2.5" y="2" width="11" height="12" rx="1.5" />
      <path d="M5 5.5h6M5 8h6M5 10.5h3.5" />
    {:else if name === 'fortilink'}
      <rect x="2" y="9" width="12" height="4" rx="1" />
      <path
        d="M5 11h.01M8 11h.01M11 11h.01M8 9V6M5.5 4.5a3.5 3.5 0 0 1 5 0M4 3a5.6 5.6 0 0 1 8 0"
      />
    {:else}
      <ellipse cx="8" cy="9.5" rx="3.2" ry="4" />
      <path
        d="M8 5.5V3.5M5.8 4.2L4.4 2.8M10.2 4.2l1.4-1.4M4.8 9H2.5M11.2 9h2.3M5.2 12.4l-1.6 1.4M10.8 12.4l1.6 1.4"
      />
    {/if}
  </svg>
{/snippet}

{#snippet cmdText(cmd: string)}
  {#each commandParts(cmd) as part, i (i)}
    {#if part.placeholder}<span class="hole">{part.text}</span>{:else}{part.text}{/if}
  {/each}
{/snippet}

<div
  class="lists-row"
  bind:this={rowEl}
  style:background={theme.tabBarBg}
  style:border-color={theme.border}
  style:--hover={theme.itemHoverBg}
  style:--text={theme.textPrimary}
  style:--dim={theme.textDim}
  style:--border={theme.border}
  style:--mono={terminalSettings.fontFamily}
>
  {#if vendor}
    <span class="row-label">Lists</span>
    <div class="chips">
      {#each vendor.lists as list (list.id)}
        {@const open = openId === list.id}
        <button
          type="button"
          class="chip"
          class:open
          style:--c={list.color}
          aria-expanded={open}
          aria-haspopup="dialog"
          title={list.summary}
          onclick={(e) => toggle(list, e.currentTarget as HTMLElement)}
        >
          <span class="chip-icon">{@render icon(list.icon, 14)}</span>
          {list.name}
          {#if list.debug && focusedDebugging}
            <span class="live-dot" title="A debug is running in this session"></span>
          {:else}
            <span class="chip-count">{count(list)}</span>
          {/if}
        </button>
      {/each}
    </div>
  {:else}
    <span class="spacer"></span>
  {/if}

  <!-- Custom and Recent stacked, so the lists get the whole width. -->
  <div class="side">
    <button
      type="button"
      class="recent-btn custom-btn"
      class:open={customOpen}
      onclick={toggleCustom}
      title={customOpen ? undefined : 'Your own commands, on every platform'}
      aria-haspopup="dialog"
      aria-expanded={customOpen}
    >
      <span aria-hidden="true">★</span>
      <span>Custom</span>
      {#if count(custom)}<span class="chip-count">{count(custom)}</span>{/if}
    </button>

    <div class="recent-wrap" bind:this={recentWrap}>
      <button
        type="button"
        class="recent-btn"
        class:open={recentOpen}
        onclick={toggleRecent}
        title={recentOpen ? undefined : 'Last commands run in this session'}
        aria-haspopup="menu"
        aria-expanded={recentOpen}
      >
        <Icon name="clock" size={12} />
        <span>Recent</span>
      </button>
      {#if recentOpen}
        <div class="recent-menu" role="menu" style:background={theme.bodyBg}>
          {#if recents.length === 0}
            <div class="recent-empty">No commands yet</div>
          {:else}
            {#each recents as r (r.text)}
              <div class="recent-row">
                <button
                  type="button"
                  class="recent-item"
                  role="menuitem"
                  onpointerdown={notePress}
                  onclick={(e) => pickRecent(r, shiftClick(e))}
                  title="{r.text} — Shift+click types it without Enter">{r.text}</button
                >
                <button
                  type="button"
                  class="type-btn"
                  title="Type without Enter — edit it before running"
                  aria-label="Type {r.text} without Enter"
                  onclick={() => pickRecent(r, true)}>Type</button
                >
                <button
                  type="button"
                  class="type-btn star-btn"
                  title="Save to Custom"
                  aria-label="Save {r.text} to Custom"
                  onclick={() => saveRecent(r)}>☆</button
                >
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if openList}
    <div
      class="panel"
      class:wide={openList.debug}
      bind:this={panelEl}
      role="dialog"
      aria-label={openList.name}
      style:left="{panelLeft}px"
      style:background={theme.bodyBg}
      style:--c={openList.color}
      style:--accent={theme.accent}
      style:--on-accent={theme.onAccent}
    >
      <div class="panel-head">
        <span class="head-icon">{@render icon(openList.icon, 16)}</span>
        <div class="head-text">
          <span class="head-title">{openList.name}</span>
          <span class="head-sub">{openList.summary} · {count(openList)}</span>
        </div>
        <span class="vendor">{customOpen ? 'All platforms' : vendor?.label}</span>
        {#if openList.debug}
          <button
            type="button"
            class="stop-btn"
            onclick={stopDebug}
            title={[...(vendor?.stopKeys ? ['Ctrl+C'] : []), ...(vendor?.stopDebug ?? [])].join(
              ' + ',
            )}
          >
            <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true"
              ><rect x="3" y="3" width="10" height="10" rx="2" fill="currentColor" /></svg
            >
            Stop debug
          </button>
        {/if}
      </div>

      {#if form}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="form" onkeydown={onFormKey}>
          <button type="button" class="back" onclick={() => (form = null)}>‹ Back</button>
          <div class="form-title">
            {form.kind === 'recipe' ? form.item.name : 'Fill in the command'}
          </div>
          {#if form.kind === 'recipe'}
            <div class="form-desc">{form.item.desc}</div>
          {/if}
          {#if Object.keys(form.values).length > 0}
            <div class="fields">
              {#if form.kind === 'recipe'}
                {#each form.item.fields as f, i (f.key)}
                  <label class="field">
                    <span>{f.label}</span>
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="text"
                      bind:value={form.values[f.key]}
                      placeholder={f.placeholder ?? ''}
                      autofocus={i === 0}
                      spellcheck="false"
                    />
                  </label>
                {/each}
              {:else}
                {#each Object.keys(form.values) as key, i (key)}
                  <label class="field">
                    <span>{key}</span>
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="text"
                      bind:value={form.values[key]}
                      autofocus={i === 0}
                      spellcheck="false"
                    />
                  </label>
                {/each}
              {/if}
            </div>
          {/if}
          <div class="preview-label">Will send</div>
          <pre class="preview">{formLines.join('\n')}</pre>
          <div class="form-actions">
            <button
              type="button"
              class="ghost-btn"
              disabled={!formReady}
              onclick={() => submitForm(true)}>Type without Enter</button
            >
            <button
              type="button"
              class="primary-btn"
              disabled={!formReady}
              onclick={() => submitForm()}>{form.kind === 'recipe' ? 'Run recipe' : 'Run'}</button
            >
          </div>
        </div>
      {:else if edit}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="form"
          onkeydown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation()
              edit = null
            }
          }}
        >
          <button type="button" class="back" onclick={() => (edit = null)}>‹ Back</button>
          <div class="form-title">{edit.at ? 'Edit command' : 'Add a command'}</div>
          <div class="fields">
            <label class="field">
              <span>Command</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="mono-input"
                bind:value={edit.cmd}
                placeholder="show interfaces <name>"
                spellcheck="false"
                autofocus
                onkeydown={(e) => e.key === 'Enter' && submitEdit()}
              />
            </label>
            <label class="field">
              <span>Description (optional)</span>
              <input
                bind:value={edit.desc}
                placeholder="What it shows or does"
                onkeydown={(e) => e.key === 'Enter' && submitEdit()}
              />
            </label>
          </div>
          <div class="form-desc">
            <span class="hole">name</span> in angle brackets, like &lt;name&gt;, is asked for when you
            run it.
          </div>
          {#if customError}<div class="form-error">{customError}</div>{/if}
          <div class="form-actions">
            <button
              type="button"
              class="primary-btn"
              disabled={!edit.cmd.trim()}
              onclick={submitEdit}>Save</button
            >
          </div>
        </div>
      {:else}
        <label class="search">
          <svg
            width="14"
            height="14"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"><circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5L14 14" /></svg
          >
          <input
            type="text"
            bind:this={searchEl}
            bind:value={query}
            onkeydown={onSearchKey}
            placeholder="Filter this list…"
            aria-label="Filter commands"
            spellcheck="false"
          />
          <kbd>Esc</kbd>
        </label>

        <div class="items">
          {#if flat.length === 0}
            {#if customOpen && userLists.errors[CUSTOM_PLATFORM]}
              <div class="empty form-error">
                custom.json has an error, fix it and press Reload in Settings → Command lists:
                {userLists.errors[CUSTOM_PLATFORM]}
              </div>
            {:else if customOpen && !query}
              <div class="empty">
                No commands yet. Add one below, or save one from Recent with ☆.
              </div>
            {:else}
              <div class="empty">No command matches “{query}”.</div>
            {/if}
          {/if}
          {#each filtered as section, si (si)}
            <div class="section-title">{section.title}</div>
            {#each section.items as item, ii (ii)}
              {@const idx = flat.indexOf(item)}
              <div
                class="item-row"
                class:active={idx === activeIndex}
                role="presentation"
                onmouseenter={() => (activeIndex = idx)}
              >
                <button
                  type="button"
                  class="item"
                  class:with-type={canTypeOnly(item)}
                  class:with-tools={customOpen}
                  onpointerdown={notePress}
                  onclick={(e) => pick(item, shiftClick(e))}
                >
                  <span class="item-line">
                    {#if item.kind === 'command'}
                      <span class="item-cmd">{@render cmdText(item.cmd)}</span>
                    {:else}
                      <span class="item-name">{item.name}</span>
                    {/if}
                    <span class="spacer"></span>
                    {#if item.kind === 'recipe'}
                      <span class="tag tag-recipe"
                        >{item.lines.length} {item.lines.length === 1 ? 'line' : 'lines'}</span
                      >
                    {:else if item.action}
                      <span class="tag tag-action">changes state</span>
                    {:else if commandMode(item) === 'type'}
                      <span class="tag tag-type">you finish it</span>
                    {:else if commandMode(item) === 'ask'}
                      <span class="tag tag-type">asks values</span>
                    {/if}
                  </span>
                  <span class="item-desc">{item.desc}</span>
                </button>
                {#if canTypeOnly(item) && !customOpen}
                  <button
                    type="button"
                    class="type-btn"
                    title="Type without Enter — edit it before running"
                    aria-label="Type {item.kind === 'command' ? item.cmd : ''} without Enter"
                    onclick={() => pick(item, true)}>Type</button
                  >
                {/if}
                {#if customOpen}
                  <div class="row-tools">
                    {#if canTypeOnly(item)}
                      <button
                        type="button"
                        class="type-btn"
                        title="Type without Enter — edit it before running"
                        aria-label="Type {item.kind === 'command' ? item.cmd : ''} without Enter"
                        onclick={() => pick(item, true)}>Type</button
                      >
                    {/if}
                    <button
                      type="button"
                      class="type-btn icon-btn"
                      title="Edit"
                      aria-label="Edit"
                      onclick={() => startEdit(item)}>✎</button
                    >
                    <button
                      type="button"
                      class="type-btn icon-btn danger"
                      title="Remove from Custom"
                      aria-label="Remove from Custom"
                      onclick={() => removeCustom(item)}>✕</button
                    >
                  </div>
                {/if}
              </div>
            {/each}
          {/each}
        </div>

        <div class="panel-foot">
          {#if customOpen}
            <button type="button" class="add-btn" onclick={() => startEdit(null)}
              >+ Add command</button
            >
          {/if}
          <span><b>Click</b> runs</span>
          <span><b>Type</b> or <b>Shift+click</b> types without Enter</span>
          <span><span class="hole">value</span> you fill in</span>
          <span class="spacer"></span>
          <span>↑↓ ↵</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .lists-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-top: 1px solid;
    flex-shrink: 0;
    user-select: none;
    min-height: 32px;
  }

  .row-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--dim);
    padding-right: 4px;
    flex-shrink: 0;
  }

  /* Chips stay on one line when they fit and wrap onto a second line
     rather than hiding behind a scroll. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    flex: 1;
    min-width: 0;
  }

  .chip {
    font-family: var(--mono) !important;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 9px 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    font-weight: 500;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    transition:
      background 0.1s,
      border-color 0.1s;
  }
  .chip:hover {
    background: var(--hover);
    border-color: var(--c);
  }
  .chip-icon {
    display: inline-flex;
    color: var(--c);
  }
  .chip.open {
    background: var(--c);
    border-color: var(--c);
    color: #fff;
  }
  .chip.open .chip-icon {
    color: #fff;
  }
  .chip-count {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--dim) 18%, transparent);
    color: var(--dim);
  }
  .chip.open .chip-count {
    background: rgba(255, 255, 255, 0.22);
    color: #fff;
  }
  .live-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 30%, transparent);
  }
  .chip.open .live-dot {
    background: #fff;
  }

  /* Custom over Recent, at the right edge. */
  .side {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex-shrink: 0;
    margin-left: 4px;
  }
  .side > .recent-btn,
  .side .recent-wrap .recent-btn {
    width: 100%;
    height: 21px;
    font-size: 11px;
    padding: 0 8px;
  }
  .side .chip-count {
    margin-left: auto;
    padding: 0 5px;
  }
  .recent-wrap {
    position: relative;
    flex-shrink: 0;
  }
  .recent-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 9px;
    border-radius: 7px;
    border: 1px dashed var(--border);
    background: transparent;
    color: var(--dim);
    font-size: 12px;
    font-family: var(--mono);
    cursor: pointer;
    white-space: nowrap;
  }
  .recent-btn:hover,
  .recent-btn.open {
    border-style: solid;
    background: var(--hover);
    color: var(--text);
  }
  .recent-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 30;
    display: flex;
    flex-direction: column;
    min-width: 240px;
    max-width: min(70ch, 80vw);
    max-height: 50vh;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .recent-item {
    text-align: left;
    font-size: 12px;
    font-family: var(--mono);
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .recent-item:hover {
    background: var(--hover);
  }
  .recent-empty {
    font-size: 12px;
    padding: 6px 8px;
    color: var(--dim);
  }

  .vendor {
    font-size: 11px;
    color: var(--dim);
    padding: 2px 8px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--dim) 12%, transparent);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* Panel: opens upwards (the bar sits at the bottom of the window). */
  .panel {
    position: absolute;
    bottom: calc(100% + 6px);
    width: 600px;
    max-width: calc(100% - 16px);
    max-height: min(70vh, 640px);
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.28);
    z-index: 30;
    overflow: hidden;
    color: var(--text);
    user-select: text;
  }
  .panel.wide {
    width: 720px;
  }

  .panel-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 10px;
  }
  .head-icon {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
    flex-shrink: 0;
  }
  .head-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .head-title {
    font-size: 14px;
    font-weight: 600;
  }
  .head-sub {
    font-size: 12px;
    color: var(--dim);
  }
  .stop-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid var(--c);
    background: transparent;
    color: var(--c);
    font-weight: 600;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    flex-shrink: 0;
  }
  .stop-btn:hover {
    background: color-mix(in srgb, var(--c) 12%, transparent);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    margin: 0 14px 8px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--dim);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: none;
    outline: none;
    box-shadow: none;
    background: transparent;
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
  }
  .search input:focus,
  .search input:focus-visible {
    outline: none;
  }
  kbd {
    font-family: var(--mono);
    font-size: 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 5px;
  }

  .items {
    overflow-y: auto;
    padding: 0 8px 8px;
    display: flex;
    flex-direction: column;
  }
  .section-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--dim);
    padding: 8px 8px 3px;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
    border: none;
    background: transparent;
    border-radius: 7px;
    padding: 6px 8px;
    cursor: pointer;
    color: var(--text);
    font-family: inherit;
  }
  .item-row {
    position: relative;
    display: flex;
    border-radius: 7px;
  }
  .item-row.active {
    background: var(--hover);
  }
  .item-row .item {
    flex: 1;
    min-width: 0;
  }
  .item.with-type {
    padding-right: 58px;
  }
  .type-btn {
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg, transparent);
    color: var(--dim);
    cursor: pointer;
    flex-shrink: 0;
  }
  .type-btn:hover {
    color: var(--text);
    border-color: var(--dim);
  }
  .item-row .type-btn {
    position: absolute;
    right: 8px;
    top: 7px;
    opacity: 0;
  }
  .item-row:hover .type-btn,
  .item-row.active .type-btn,
  .type-btn:focus-visible {
    opacity: 1;
  }
  .recent-row {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: 4px;
  }
  .recent-row:hover {
    background: var(--hover);
  }
  .recent-row .recent-item {
    flex: 1;
    min-width: 0;
  }
  .recent-row .type-btn {
    opacity: 0;
    margin-right: 4px;
  }
  .recent-row:hover .type-btn,
  .recent-row .type-btn:focus-visible {
    opacity: 1;
  }
  .item-line {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .item-cmd {
    font-family: var(--mono);
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .item-name {
    font-size: 13px;
    font-weight: 600;
  }
  .item-desc {
    font-size: 12px;
    color: var(--dim);
  }
  .spacer {
    flex: 1;
  }
  .hole {
    font-family: var(--mono);
    font-size: 11.5px;
    color: #9a5a1a;
    background: color-mix(in srgb, #e6a35a 22%, transparent);
    border: 1px dashed #d9a465;
    border-radius: 4px;
    padding: 0 5px;
    margin: 0 1px;
  }
  .tag {
    font-size: 10.5px;
    padding: 1px 6px;
    border-radius: 5px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .tag-type {
    color: #9a5a1a;
    background: color-mix(in srgb, #e6a35a 18%, transparent);
  }
  .tag-action {
    color: #b23b3b;
    background: color-mix(in srgb, #b23b3b 14%, transparent);
  }
  .tag-recipe {
    color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .empty {
    font-size: 12px;
    color: var(--dim);
    padding: 12px 8px;
  }

  .panel-foot {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--dim);
  }
  .panel-foot b {
    font-weight: 600;
    color: var(--text);
  }

  /* Recipe / placeholder form */
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 14px 14px;
    overflow-y: auto;
  }
  .back {
    align-self: flex-start;
    border: none;
    background: transparent;
    color: var(--dim);
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    padding: 0;
  }
  .form-title {
    font-size: 14px;
    font-weight: 600;
  }
  .form-desc {
    font-size: 12px;
    color: var(--dim);
    margin-top: -6px;
  }
  .fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--dim);
  }
  .field input {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0 9px;
    background: transparent;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12.5px;
    outline: none;
  }
  .field input:focus {
    border-color: var(--accent);
  }
  .preview-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--dim);
  }
  .preview {
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: #20212b;
    color: #d6d8e0;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .ghost-btn,
  .primary-btn {
    height: 32px;
    padding: 0 14px;
    border-radius: 8px;
    font-weight: 500;
    font-size: 12.5px;
    font-family: inherit;
    cursor: pointer;
  }
  .ghost-btn {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
  }
  .primary-btn {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }
  .ghost-btn:disabled,
  .primary-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* ── Custom list ── */
  .item.with-tools {
    padding-right: 120px;
  }
  .item-row .row-tools {
    position: absolute;
    right: 8px;
    top: 7px;
    display: flex;
    gap: 4px;
    opacity: 0;
  }
  .item-row:hover .row-tools,
  .item-row.active .row-tools,
  .row-tools:focus-within {
    opacity: 1;
  }
  .item-row .row-tools .type-btn {
    position: static;
    opacity: 1;
  }
  .icon-btn {
    min-width: 24px;
  }
  .icon-btn.danger:hover {
    color: #e5484d;
    border-color: #e5484d;
  }
  .star-btn {
    font-size: 12px;
  }
  .mono-input {
    font-family: var(--mono) !important;
  }
  .form-error {
    color: #e5484d;
    font-size: 12px;
  }
  .add-btn {
    flex-shrink: 0;
    white-space: nowrap;
    font-family: inherit;
    font-size: 11.5px;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 6px;
    border: 1px solid var(--c, var(--border));
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .add-btn:hover {
    background: var(--hover);
  }
</style>
