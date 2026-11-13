<script lang="ts">
  import { ICON_PATHS, type IconName } from './icons'

  interface Props {
    name: IconName
    size?: number
    /** Stroke width — 1.5 per the design system. */
    sw?: number
    /** For `chevron`: rotates 90° when open (folder/section expanded). */
    open?: boolean
  }

  const { name, size = 14, sw = 1.5, open = false }: Props = $props()
</script>

<svg
  class="zx-icon"
  class:open
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width={sw}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <!-- eslint-disable-next-line svelte/no-at-html-tags — static, app-defined markup -->
  {@html ICON_PATHS[name]}
</svg>

<style>
  .zx-icon {
    flex-shrink: 0;
    display: block;
    transition: transform 0.12s ease;
  }

  .zx-icon.open {
    transform: rotate(90deg);
  }
</style>
