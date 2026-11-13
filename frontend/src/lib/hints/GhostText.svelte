<script lang="ts">
  import type { HintController } from './controller.svelte'

  interface Props {
    controller: HintController
    color?: string
    fontFamily?: string
    fontSize?: number
  }

  let { controller, color = 'rgba(255,255,255,0.32)', fontFamily, fontSize }: Props = $props()
</script>

{#if controller.ghost}
  <div
    class="ghost"
    style:left="{controller.cursor.left}px"
    style:top="{controller.cursor.top}px"
    style:line-height="{controller.cursor.cellHeight}px"
    style:height="{controller.cursor.cellHeight}px"
    style:color
    style:font-family={fontFamily}
    style:font-size={fontSize ? `${fontSize}px` : undefined}
  >
    {controller.ghost}
  </div>
{/if}

<style>
  .ghost {
    position: absolute;
    pointer-events: none;
    white-space: pre;
    font-family: inherit;
    font-variant-ligatures: none;
    user-select: none;
    z-index: 4;
    /* Subtle fade-in so the suggestion doesn't pop */
    animation: ghost-in 90ms ease-out;
  }

  @keyframes ghost-in {
    from {
      opacity: 0;
      transform: translateX(-2px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
</style>
