<script lang="ts">
  import type { PylonTheme } from '$lib/themes/index'
  import type { TcpMss } from '$lib/bridge/types'

  type ConnStatus = 'connecting' | 'connected' | 'error' | 'lost' | 'closed'

  interface Props {
    theme: PylonTheme
    status?: ConnStatus
    host?: string
    port?: number
    protocol?: string
    latencyMs?: number
    bytesPerSec?: number
    layout?: string
    /** TCP MSS snapshot for the focused SSH session; `null` for non-SSH or
     *  bastion (no direct TCP socket to query). Either direction may itself
     *  be `null` when the OS doesn't expose it. */
    mss?: TcpMss | null
  }

  const {
    theme,
    status = 'connecting',
    host = '',
    port,
    protocol = 'SSH',
    latencyMs,
    bytesPerSec,
    layout = '1×1',
    mss = null,
  }: Props = $props()

  /// Build the MSS display ("1448↑ / 1460↓"). Hides the whole segment when
  /// neither direction is known — most often a non-SSH session or a tunnel.
  const mssLabel = $derived(buildMssLabel(mss))

  function buildMssLabel(m: TcpMss | null | undefined): string {
    if (!m || (m.send == null && m.recv == null)) return ''
    const parts: string[] = []
    if (m.send != null) parts.push(`${m.send}↑`)
    if (m.recv != null) parts.push(`${m.recv}↓`)
    return parts.join(' / ')
  }

  function formatRate(bps: number): string {
    if (bps < 1024) return `${bps} B/s`
    if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`
    return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`
  }

  const statusLabel = $derived(
    status === 'connected'
      ? 'CONNECTED'
      : status === 'error'
        ? 'FAILED'
        : status === 'lost'
          ? 'CONNECTION LOST'
          : status === 'closed'
            ? 'DISCONNECTED'
            : 'CONNECTING…',
  )

  const statusColor = $derived(
    status === 'connected'
      ? theme.ok
      : status === 'error' || status === 'lost'
        ? theme.err
        : status === 'closed'
          ? theme.textDim
          : theme.warn,
  )

  /// Colour the latency reading by health so link quality reads at a glance:
  /// snappy (<80 ms) green, workable (<200 ms) neutral, sluggish (≥200 ms)
  /// amber. The handoff leaves latency uncoloured; this is a pure add.
  const latencyColor = $derived(
    latencyMs === undefined
      ? theme.textMuted
      : latencyMs < 80
        ? theme.ok
        : latencyMs < 200
          ? theme.textMuted
          : theme.warn,
  )

  let uptimeStr = $state('')

  $effect(() => {
    // Track status as a dependency; restart timer on each connect
    if (status === 'connected') {
      const startTime = Date.now()
      function tick() {
        const secs = Math.floor((Date.now() - startTime) / 1000)
        const h = Math.floor(secs / 3600)
        const m = Math.floor((secs % 3600) / 60)
        const s = secs % 60
        uptimeStr =
          h > 0
            ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
            : `${m}:${String(s).padStart(2, '0')}`
      }
      tick()
      const timer = setInterval(tick, 1000)
      return () => {
        clearInterval(timer)
        uptimeStr = ''
      }
    } else {
      uptimeStr = ''
    }
  })
</script>

<footer
  class="statusbar"
  style:background={theme.statusbarBg}
  style:border-top="1px solid {theme.border}"
  style:font-family={theme.fontMono}
  style:color={theme.textDim}
>
  <!-- Left segments -->
  <div class="sb-left">
    <span class="sb-seg">
      <!-- Static glow, no pulse: an infinite animation here ran for as long as
           a session stayed connected, forcing WebKitGTK to repaint every frame
           forever (constant WebKitWebProcess CPU on Linux, and the trigger for
           the DMABUF busy-repaint loop on affected GPU drivers). Same reason
           the cursor glow in app.css is static. -->
      <span
        class="sb-led"
        style:background={statusColor}
        style:box-shadow={status === 'connected' ? `0 0 5px ${statusColor}` : 'none'}
      ></span>
      <span style:color={statusColor}>{statusLabel}</span>
    </span>

    {#if host}
      <span class="sb-divider">│</span>
      <span class="sb-seg" style:color={theme.textMuted}>{host}</span>
    {/if}

    {#if port !== undefined}
      <span class="sb-divider">│</span>
      <span class="sb-seg">
        <span style:color={theme.textDim}>{protocol.toLowerCase()}</span>
        <span style:color={theme.textMuted}> {port}</span>
      </span>
    {/if}

    {#if mssLabel && status === 'connected'}
      <span class="sb-divider">│</span>
      <span
        class="sb-seg sb-mono"
        style:color={theme.textMuted}
        title="TCP MSS — bytes per segment {mss?.send != null
          ? `we send (${mss.send})`
          : ''}{mss?.send != null && mss?.recv != null ? ' / ' : ''}{mss?.recv != null
          ? `the peer sends (${mss.recv})`
          : ''}"
      >
        <span style:color={theme.textDim}>mss</span>
        <span>{mssLabel}</span>
      </span>
    {/if}

    {#if uptimeStr && status === 'connected'}
      <span class="sb-divider">│</span>
      <span class="sb-seg sb-mono" style:color={theme.textMuted}>up {uptimeStr}</span>
    {/if}

    {#if latencyMs !== undefined && status === 'connected'}
      <span class="sb-divider">│</span>
      <span class="sb-seg sb-mono" style:color={latencyColor} title="Round-trip latency"
        >{latencyMs}ms</span
      >
    {/if}

    {#if bytesPerSec !== undefined && status === 'connected'}
      <span class="sb-divider">│</span>
      <span class="sb-seg sb-mono" style:color={theme.textMuted}>
        ↓ {formatRate(bytesPerSec)}
      </span>
    {/if}
  </div>

  <!-- Right segments -->
  <div class="sb-right">
    <span class="sb-seg">utf-8</span>
    <span class="sb-divider">│</span>
    <span class="sb-seg">VT320</span>
    <span class="sb-divider">│</span>
    <span class="sb-seg">{layout}</span>
  </div>
</footer>

<style>
  .statusbar {
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    font-size: 11px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .sb-left,
  .sb-right {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .sb-seg {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 7px;
    white-space: nowrap;
  }

  .sb-divider {
    opacity: 0.25;
  }

  .sb-led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .sb-mono {
    font-variant-numeric: tabular-nums;
  }
</style>
