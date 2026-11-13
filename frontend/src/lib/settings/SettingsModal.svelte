<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon from '$lib/icons/Icon.svelte'
  import TerminalSettingsPanel from './TerminalSettingsPanel.svelte'
  import HighlightRulesPanel from '$lib/sessions/HighlightRulesPanel.svelte'
  import HintsPanel from './HintsPanel.svelte'
  import LoggingPanel from './LoggingPanel.svelte'
  import SshPanel from './SshPanel.svelte'
  import DataPanel from './DataPanel.svelte'
  import BackupPanel from './BackupPanel.svelte'
  import {
    SHORTCUT_ACTIONS,
    bindings,
    eventToCombo,
    resetBinding,
    saveBinding,
    type ShortcutAction,
  } from '$lib/stores/keybindings.svelte'

  interface Props {
    onClose: () => void
  }

  let { onClose }: Props = $props()

  type Tab =
    | 'appearance'
    | 'highlight'
    | 'hints'
    | 'logging'
    | 'ssh'
    | 'data'
    | 'backup'
    | 'shortcuts'
  let activeTab = $state<Tab>('appearance')

  /// While recording, swallow keys instead of dispatching them globally.
  let recordingAction = $state<ShortcutAction | null>(null)

  function beginRecord(id: ShortcutAction) {
    recordingAction = id
  }

  async function onRecordKeydown(e: KeyboardEvent) {
    if (!recordingAction) return
    // Escape exits recording without changing the binding.
    if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      e.preventDefault()
      e.stopPropagation()
      recordingAction = null
      return
    }
    const combo = eventToCombo(e)
    if (!combo) return // modifier-only press, keep waiting
    e.preventDefault()
    e.stopPropagation()
    const id = recordingAction
    recordingAction = null
    await saveBinding(id, combo)
  }

  function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose()
  }

  function handleKeydown(e: KeyboardEvent) {
    if (recordingAction) return // recorder handles its own keys
    if (e.key === 'Escape') onClose()
  }
</script>

<svelte:window onkeydown={onRecordKeydown} />

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Settings"
  tabindex="-1"
  onclick={handleOverlayClick}
  onkeydown={handleKeydown}
  transition:fade={{ duration: 140 }}
>
  <div class="modal" transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}>
    <div class="modal-header">
      <h2 class="modal-title">Settings</h2>
      <button class="close-btn" onclick={onClose} aria-label="Close"
        ><Icon name="x" size={13} /></button
      >
    </div>

    <div class="modal-body">
      <nav class="tab-nav">
        <button
          class="tab-btn"
          class:active={activeTab === 'appearance'}
          onclick={() => (activeTab = 'appearance')}
        >
          Appearance
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'highlight'}
          onclick={() => (activeTab = 'highlight')}
        >
          Highlight Rules
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'hints'}
          onclick={() => (activeTab = 'hints')}
        >
          Hints
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'logging'}
          onclick={() => (activeTab = 'logging')}
        >
          Logging
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'ssh'}
          onclick={() => (activeTab = 'ssh')}
        >
          SSH
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'data'}
          onclick={() => (activeTab = 'data')}
        >
          Data
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'backup'}
          onclick={() => (activeTab = 'backup')}
        >
          Backup
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === 'shortcuts'}
          onclick={() => (activeTab = 'shortcuts')}
        >
          Shortcuts
        </button>
      </nav>

      <div class="tab-content">
        {#if activeTab === 'appearance'}
          <TerminalSettingsPanel />
        {:else if activeTab === 'highlight'}
          <HighlightRulesPanel />
        {:else if activeTab === 'hints'}
          <HintsPanel />
        {:else if activeTab === 'logging'}
          <LoggingPanel />
        {:else if activeTab === 'ssh'}
          <SshPanel />
        {:else if activeTab === 'data'}
          <DataPanel />
        {:else if activeTab === 'backup'}
          <BackupPanel />
        {:else}
          <div class="shortcuts-list">
            <p class="hint">
              Click a binding to record a new keystroke. Press <kbd class="kbd">Esc</kbd> to cancel.
            </p>
            {#each SHORTCUT_ACTIONS as a (a.id)}
              <div class="shortcut-row">
                <span class="shortcut-desc">{a.label}</span>
                {#if recordingAction === a.id}
                  <kbd class="kbd recording">press a key…</kbd>
                {:else}
                  <button class="kbd kbd-btn" onclick={() => beginRecord(a.id)}>
                    {bindings[a.id] || '(unbound)'}
                  </button>
                {/if}
                <button
                  class="reset-btn"
                  title="Reset to default ({a.default})"
                  onclick={() => resetBinding(a.id)}>↺</button
                >
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .modal {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: var(--zx-radius);
    width: 42rem;
    max-width: 95vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--zx-shadow);
    font-family: var(--zx-font-ui);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--zx-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
  }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--zx-text-muted);
    font-size: 1rem;
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-family: inherit;
  }
  .close-btn:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }

  .modal-body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .tab-nav {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.5rem;
    border-right: 1px solid var(--zx-border);
    min-width: 9rem;
    flex-shrink: 0;
  }

  .tab-btn {
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    padding: 0.35rem 0.6rem;
    color: var(--zx-text-muted);
    font-size: 0.8rem;
    cursor: pointer;
    font-family: inherit;
  }
  .tab-btn:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }
  .tab-btn.active {
    color: var(--zx-text);
    background: var(--zx-active-bg);
    box-shadow: inset 2px 0 0 var(--zx-accent);
  }

  .tab-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .shortcuts-list {
    padding: 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.25rem 0;
    border-bottom: 1px solid var(--zx-border);
  }
  .shortcut-row:last-child {
    border-bottom: none;
  }

  .kbd {
    font-family: var(--zx-font-mono);
    font-size: 0.72rem;
    background: var(--zx-surface-2);
    border: 1px solid var(--zx-border);
    border-radius: 0.2rem;
    padding: 0.15rem 0.4rem;
    color: var(--zx-text);
    white-space: nowrap;
    min-width: 7rem;
    text-align: center;
    flex-shrink: 0;
  }

  .shortcut-desc {
    font-size: 0.78rem;
    color: var(--zx-text-muted);
    line-height: 1.4;
    flex: 1;
  }

  .hint {
    font-size: 0.74rem;
    color: var(--zx-text-muted);
    margin: 0 0 0.5rem;
  }

  .kbd-btn {
    cursor: pointer;
    font-family: var(--zx-font-mono);
  }

  .kbd-btn:hover {
    background: var(--zx-hover-bg);
    border-color: var(--zx-border);
  }

  .kbd.recording {
    background: var(--zx-active-bg);
    border-color: var(--zx-active-border);
    color: var(--zx-text);
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.6;
    }
  }

  .reset-btn {
    background: transparent;
    border: none;
    color: var(--zx-text-dim);
    font-size: 0.95rem;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 0.2rem;
    flex-shrink: 0;
    font-family: inherit;
  }

  .reset-btn:hover {
    color: var(--zx-text);
    background: var(--zx-hover-bg);
  }
</style>
