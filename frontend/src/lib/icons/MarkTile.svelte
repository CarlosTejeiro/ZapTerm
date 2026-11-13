<script lang="ts">
  /**
   * ZapTerm brand mark «Tile» — single lightning bolt knocked out of a rounded
   * accent tile. The bolt is cut in the paper colour so it reads as part of
   * the chrome behind it. Mirrors the bundled app icon.
   */
  interface Props {
    size?: number
    /** Tile fill — defaults to the active theme accent. */
    accent?: string
    /** Bolt colour — defaults to the active theme paper/app background. */
    paper?: string
  }

  const { size = 15, accent = 'var(--zx-accent)', paper = 'var(--zx-app-bg)' }: Props = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  aria-hidden="true"
  style:flex-shrink="0"
>
  <rect x="0.5" y="0.5" width="15" height="15" rx="4.2" fill={accent} />
  <path d="M 9.1 2.6 L 5 8.6 H 7.6 L 6.6 13.4 L 11.4 6.8 H 8.6 Z" fill={paper} />
</svg>
