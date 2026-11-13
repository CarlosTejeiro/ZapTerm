<script lang="ts">
  import { fly } from 'svelte/transition'
  import Icon from '$lib/icons/Icon.svelte'
  import type { IconName } from '$lib/icons/icons'
  import { toasts, dismissToast, type Toast } from './toast-store.svelte'

  function colorFor(kind: Toast['kind']): {
    fg: string
    bg: string
    border: string
    icon: IconName
  } {
    switch (kind) {
      case 'success':
        return {
          fg: 'var(--zx-ok)',
          bg: 'color-mix(in srgb, var(--zx-ok) 12%, var(--zx-surface))',
          border: 'color-mix(in srgb, var(--zx-ok) 45%, transparent)',
          icon: 'check',
        }
      case 'warning':
        return {
          fg: 'var(--zx-warn)',
          bg: 'color-mix(in srgb, var(--zx-warn) 12%, var(--zx-surface))',
          border: 'color-mix(in srgb, var(--zx-warn) 45%, transparent)',
          icon: 'alert',
        }
      case 'error':
        return {
          fg: 'var(--zx-err)',
          bg: 'color-mix(in srgb, var(--zx-err) 12%, var(--zx-surface))',
          border: 'color-mix(in srgb, var(--zx-err) 45%, transparent)',
          icon: 'x',
        }
      default:
        return {
          fg: 'var(--zx-accent)',
          bg: 'color-mix(in srgb, var(--zx-accent) 12%, var(--zx-surface))',
          border: 'color-mix(in srgb, var(--zx-accent) 40%, transparent)',
          icon: 'info',
        }
    }
  }
</script>

<div class="stack">
  {#each toasts as toast (toast.id)}
    {@const c = colorFor(toast.kind)}
    <div
      class="toast"
      style:background={c.bg}
      style:border-color={c.border}
      transition:fly={{ x: 40, duration: 200 }}
      role="status"
      aria-live="polite"
    >
      <span class="icon" style:color={c.fg}><Icon name={c.icon} size={15} /></span>
      <div class="body">
        <div class="title">{toast.title}</div>
        {#if toast.detail}<div class="detail">{toast.detail}</div>{/if}
      </div>
      <button type="button" class="close" onclick={() => dismissToast(toast.id)} aria-label="Close"
        ><Icon name="x" size={12} /></button
      >
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    top: 56px;
    right: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 950;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
    background: var(--zx-surface);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid;
    border-radius: 8px;
    padding: 0.55rem 0.7rem 0.55rem 0.65rem;
    min-width: 260px;
    max-width: 360px;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    font-size: 0.78rem;
    box-shadow: var(--zx-shadow);
    pointer-events: auto;
  }

  .icon {
    display: flex;
    margin-top: 1px;
    flex-shrink: 0;
  }

  .body {
    flex: 1;
    line-height: 1.4;
  }

  .title {
    font-weight: 600;
    color: var(--zx-text);
  }

  .detail {
    margin-top: 2px;
    color: var(--zx-text-muted);
    font-size: 0.72rem;
  }

  .close {
    background: transparent;
    border: 0;
    color: var(--zx-text-dim);
    cursor: pointer;
    padding: 0 0.2rem;
    font-size: 0.85rem;
  }
  .close:hover {
    color: var(--zx-text);
  }
</style>
