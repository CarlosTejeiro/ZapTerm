<script lang="ts">
  import Pane from './Pane.svelte'
  import type { PaneData } from './Pane.svelte'
  import MasterInputBar from './MasterInputBar.svelte'
  import ComparisonPanel from '$lib/orchestrator/ComparisonPanel.svelte'
  import { CommandRunner, type RunnerHost } from '$lib/orchestrator/commandRunner.svelte'
  import { getSessionPlatform } from '$lib/bridge/commands'
  import type { PylonTheme } from '$lib/themes/index'
  import {
    paneToSession,
    claimBroadcastScope,
    releaseBroadcastScope,
  } from '$lib/stores/sessionRuntime.svelte'

  type PaneStatus = 'connecting' | 'connected' | 'error' | 'lost' | 'closed'

  interface Props {
    panes: PaneData[]
    cols: number
    theme: PylonTheme
    focusedPaneId: number
    /** This grid's tab is the visible one. Every tab stays mounted, so only
     *  the active grid may scope the broadcast. */
    active: boolean
    onFocus: (paneId: number) => void
    onClosePane: (paneId: number) => void
    onStatusChange: (paneId: number, status: PaneStatus) => void
    onGlobalShortcut?: (key: string, e: KeyboardEvent) => void
  }

  const {
    panes,
    cols,
    theme,
    focusedPaneId,
    active,
    onFocus,
    onClosePane,
    onStatusChange,
    onGlobalShortcut,
  }: Props = $props()

  // Live session UUIDs of the panes currently in this grid. Recomputed as
  // panes connect/disconnect (paneToSession is reactive).
  const liveSessionIds = $derived(
    panes.map((p) => paneToSession.get(p.id)).filter((id): id is string => typeof id === 'string'),
  )

  // While this grid's tab is the active one, scope all broadcast fan-out
  // (per-pane typing, snippets) to THIS group's live sessions instead of every
  // open pane. Hidden tabs stay mounted, so a background grid must not hold
  // the scope: typing in one grid used to reach the devices of another.
  const scopeOwner = {}
  $effect(() => {
    if (!active) return
    claimBroadcastScope(scopeOwner, new Set(liveSessionIds))
    return () => releaseBroadcastScope(scopeOwner)
  })

  // ── Run & compare (command runner) ──────────────────────────────────────
  const runner = new CommandRunner()
  let showPanel = $state(false)

  async function runCompare(command: string) {
    // Resolve each live pane's platform (for prompt-return detection). Panes
    // without a saved session / known platform fall back to time-window only.
    const hosts: RunnerHost[] = []
    for (const pane of panes) {
      const sid = paneToSession.get(pane.id)
      if (!sid) continue
      let platform: string | null = null
      const savedId = pane.savedSession?.id
      if (savedId != null) {
        platform = await getSessionPlatform(savedId).catch(() => null)
      }
      hosts.push({ sessionId: sid, label: pane.label, platform })
    }
    showPanel = true
    await runner.run(hosts, command)
  }

  $effect(() => () => runner.reset())
</script>

<div class="grid-wrap">
  <div class="grid" style:grid-template-columns={`repeat(${cols}, minmax(0, 1fr))`}>
    {#each panes as pane (pane.id)}
      <div class="grid-cell">
        <Pane
          {theme}
          {pane}
          focused={focusedPaneId === pane.id}
          hasSiblings={panes.length > 1}
          canClose={true}
          onFocus={() => onFocus(pane.id)}
          onClosePane={() => onClosePane(pane.id)}
          onStatusChange={(s) => onStatusChange(pane.id, s)}
          {onGlobalShortcut}
        />
      </div>
    {/each}
  </div>

  <MasterInputBar {theme} sessionIds={liveSessionIds} onRunCompare={runCompare} />
</div>

{#if showPanel}
  <ComparisonPanel
    {theme}
    command={runner.command}
    hosts={runner.hosts}
    running={runner.running}
    onClose={() => {
      showPanel = false
      runner.reset()
    }}
  />
{/if}

<style>
  .grid-wrap {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }

  /* Match the split-view card gap so every layout breathes the same. */
  .grid {
    flex: 1;
    display: grid;
    gap: 10px;
    min-width: 0;
    min-height: 0;
    grid-auto-rows: 1fr;
  }

  .grid-cell {
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
