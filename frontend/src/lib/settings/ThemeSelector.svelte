<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte'
  import { terminalSettings, colorSchemes, applyColorScheme } from '$lib/stores/settings.svelte'
  import type { ColorPalette } from '$lib/bridge/types'

  function palette(json: string): ColorPalette {
    try {
      return JSON.parse(json)
    } catch {
      return {} as ColorPalette
    }
  }
</script>

<div class="theme-grid">
  {#each colorSchemes as scheme (scheme.id)}
    {@const p = palette(scheme.palette_json)}
    <button
      class="theme-card"
      class:active={terminalSettings.activeColorScheme === scheme.name}
      onclick={() => applyColorScheme(scheme.name)}
      title={scheme.name}
    >
      <!-- mini color swatch -->
      <div class="swatch" style:background={p.background}>
        <span class="swatch-text" style:color={p.foreground}>A</span>
        <div class="swatch-dots">
          <span style:background={p.red}></span>
          <span style:background={p.green}></span>
          <span style:background={p.yellow}></span>
          <span style:background={p.blue}></span>
          <span style:background={p.magenta}></span>
          <span style:background={p.cyan}></span>
        </div>
      </div>
      <span class="theme-name">{scheme.name}</span>
      {#if terminalSettings.activeColorScheme === scheme.name}
        <span class="check"><Icon name="check" size={11} sw={2} /></span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .theme-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.4rem;
    padding: 0.5rem;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.4rem;
    border-radius: 0.375rem;
    border: 1px solid transparent;
    cursor: pointer;
    background: transparent;
    position: relative;
    transition: border-color 0.1s;
  }

  .theme-card:hover {
    border-color: var(--zx-border);
    background: var(--zx-hover-bg);
  }

  .theme-card.active {
    border-color: var(--zx-active-border);
    background: var(--zx-active-bg);
  }

  .swatch {
    width: 100%;
    height: 2.25rem;
    border-radius: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.1rem 0.35rem;
    overflow: hidden;
  }

  .swatch-text {
    font-size: 1rem;
    font-weight: bold;
    font-family: monospace;
    line-height: 1;
  }

  .swatch-dots {
    display: flex;
    gap: 2px;
  }

  .swatch-dots span {
    display: inline-block;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
  }

  .theme-name {
    font-size: 0.65rem;
    color: var(--zx-text-muted);
    text-align: center;
    line-height: 1.2;
  }

  .check {
    position: absolute;
    top: 0.2rem;
    right: 0.3rem;
    color: var(--zx-accent);
  }
</style>
