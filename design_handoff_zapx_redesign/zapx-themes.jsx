// zapx-themes.jsx — Temas propuestos para ZAPX (overrides de tokens --zx-* / --zt-*)
// y tablero de propuestas de marca.

const THEMES = {
  parchment: {
    label: 'Parchment · claro cálido (actual, refinado)',
    vars: {}, // los defaults del :root
  },

  oxide: {
    label: 'Oxide · oscuro cálido, acento cobre',
    vars: {
      '--zx-paper': '#1d1a16', '--zx-paper-15': '#211d18', '--zx-paper-2': '#262119',
      '--zx-ink': '#eae3d4', '--zx-ink-muted': '#a2967f', '--zx-ink-dim': '#6f6452',
      '--zx-line': 'rgba(255,230,180,0.11)', '--zx-hover': 'rgba(255,230,180,0.05)',
      '--zx-accent': '#cf8a46', '--zx-accent-2': '#7aa893', '--zx-accent-soft': '#e6b88a',
      '--zx-ok': '#6fbf8d', '--zx-warn': '#d8b16a', '--zx-err': '#e07b6d',
      '--zt-bg': '#15120e', '--zt-deep': '#100d0a', '--zt-fg': '#ddd5c4', '--zt-dim': '#5d5546',
      '--zt-cyan': '#8fb8a4', '--zt-green': '#7dbb82', '--zt-red': '#e07b6d',
      '--zt-amber': '#d8a85e', '--zt-cursor': '#cf8a46', '--zt-white': '#f2ecdf',
    },
  },

  fjord: {
    label: 'Fjord · oscuro frío, acento hielo',
    vars: {
      '--zx-paper': '#161a21', '--zx-paper-15': '#181d25', '--zx-paper-2': '#1d232c',
      '--zx-ink': '#dee5ee', '--zx-ink-muted': '#91a0b2', '--zx-ink-dim': '#5a6878',
      '--zx-line': 'rgba(180,210,255,0.11)', '--zx-hover': 'rgba(180,210,255,0.05)',
      '--zx-accent': '#5ca3d6', '--zx-accent-2': '#d68a6e', '--zx-accent-soft': '#9ec9e8',
      '--zx-ok': '#5fb784', '--zx-warn': '#d8b16a', '--zx-err': '#e07474',
      '--zt-bg': '#11141a', '--zt-deep': '#0c0f14', '--zt-fg': '#d3dce6', '--zt-dim': '#4d5b6c',
      '--zt-cyan': '#6fc3d8', '--zt-green': '#66bb8a', '--zt-red': '#e07474',
      '--zt-amber': '#d8b16a', '--zt-cursor': '#8ab8e8', '--zt-white': '#eef3f8',
    },
  },

  porcelain: {
    label: 'Porcelain · claro frío, acento índigo acero',
    vars: {
      '--zx-paper': '#f3f5f7', '--zx-paper-15': '#eef1f4', '--zx-paper-2': '#e8ecf0',
      '--zx-ink': '#252a31', '--zx-ink-muted': '#5e6873', '--zx-ink-dim': '#98a2ad',
      '--zx-line': 'rgba(40,60,90,0.15)', '--zx-hover': 'rgba(40,60,90,0.05)',
      '--zx-accent': '#4a66a0', '--zx-accent-2': '#b06a4f', '--zx-accent-soft': '#94aacf',
      '--zx-ok': '#3e8f60', '--zx-warn': '#b88528', '--zx-err': '#b13a3a',
      '--zt-bg': '#1f242e', '--zt-deep': '#191d26', '--zt-fg': '#d6dce4', '--zt-dim': '#566074',
      '--zt-cyan': '#6fbdbc', '--zt-green': '#5fb784', '--zt-red': '#e07474',
      '--zt-amber': '#d8a85e', '--zt-cursor': '#8aa6e8', '--zt-white': '#eef1f6',
    },
  },

  phosphor: {
    label: 'Phosphor · verde sobre negro (CRT clásico)',
    vars: {
      '--zx-paper': '#0c100c', '--zx-paper-15': '#0e130e', '--zx-paper-2': '#121812',
      '--zx-ink': '#cfe8cf', '--zx-ink-muted': '#7da87d', '--zx-ink-dim': '#4a6b4a',
      '--zx-line': 'rgba(80,255,120,0.13)', '--zx-hover': 'rgba(80,255,120,0.05)',
      '--zx-accent': '#36d97c', '--zx-accent-2': '#9dd87a', '--zx-accent-soft': '#7ee8a8',
      '--zx-ok': '#3ddc84', '--zx-warn': '#d8b16a', '--zx-err': '#e07474',
      '--zt-bg': '#050905', '--zt-deep': '#030603', '--zt-fg': '#8ae89a', '--zt-dim': '#2f5a3a',
      '--zt-cyan': '#6fd8b0', '--zt-green': '#57e389', '--zt-red': '#e07474',
      '--zt-amber': '#d8c16a', '--zt-cursor': '#57e389', '--zt-white': '#c9f5d0',
    },
  },

  amber: {
    label: 'Amber · ámbar sobre negro (CRT clásico)',
    vars: {
      '--zx-paper': '#120e07', '--zx-paper-15': '#151009', '--zx-paper-2': '#1a140b',
      '--zx-ink': '#f0dcb8', '--zx-ink-muted': '#ad9468', '--zx-ink-dim': '#715f42',
      '--zx-line': 'rgba(255,200,100,0.13)', '--zx-hover': 'rgba(255,200,100,0.05)',
      '--zx-accent': '#e8a33c', '--zx-accent-2': '#c97c4a', '--zx-accent-soft': '#f5c878',
      '--zx-ok': '#8fb96a', '--zx-warn': '#f0c060', '--zx-err': '#e07b5e',
      '--zt-bg': '#0a0703', '--zt-deep': '#070502', '--zt-fg': '#ecb45c', '--zt-dim': '#5c4a2c',
      '--zt-cyan': '#f0cd8a', '--zt-green': '#8fb96a', '--zt-red': '#e07b5e',
      '--zt-amber': '#f0c060', '--zt-cursor': '#ffb84d', '--zt-white': '#f8e2b8',
    },
  },

  nocturne: {
    label: 'Nocturne · oscuro violeta, acento periwinkle',
    vars: {
      '--zx-paper': '#181520', '--zx-paper-15': '#1b1825', '--zx-paper-2': '#201c2c',
      '--zx-ink': '#e6e2f0', '--zx-ink-muted': '#9b93b4', '--zx-ink-dim': '#655d80',
      '--zx-line': 'rgba(215,200,255,0.11)', '--zx-hover': 'rgba(215,200,255,0.05)',
      '--zx-accent': '#9d92ea', '--zx-accent-2': '#5eb3b2', '--zx-accent-soft': '#c4bcf5',
      '--zx-ok': '#6fbf8d', '--zx-warn': '#d4a85a', '--zx-err': '#e07b8a',
      '--zt-bg': '#121019', '--zt-deep': '#0d0b13', '--zt-fg': '#dcd8e8', '--zt-dim': '#4f4868',
      '--zt-cyan': '#7fd0ce', '--zt-green': '#6fbf8d', '--zt-red': '#e07b8a',
      '--zt-amber': '#d8b16a', '--zt-cursor': '#b3a7f7', '--zt-white': '#f0edf8',
    },
  },
};

function ThemeScope({ theme, children }) {
  return <div style={{ width: '100%', height: '100%', ...THEMES[theme].vars }}>{children}</div>;
}

// ── Tablero de marca ─────────────────────────────────────────────────────────
function BrandCard({ Mark, name, desc }) {
  return (
    <div style={{
      flex: 1, display: 'flex', flexDirection: 'column', gap: 14,
      padding: '20px 20px 16px', borderRadius: 10,
      background: 'var(--zx-paper)', border: '1px solid var(--zx-line)',
    }}>
      <div style={{
        height: 84, display: 'flex', alignItems: 'center', justifyContent: 'center',
        borderRadius: 8, background: 'var(--zx-paper-2)',
      }}>
        <Mark size={44} />
      </div>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--zx-ink)', marginBottom: 3 }}>{name}</div>
        <div style={{ fontSize: 11.5, lineHeight: 1.45, color: 'var(--zx-ink-muted)' }}>{desc}</div>
      </div>
      {/* En contexto: titlebar a tamaño real */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8, height: 34, padding: '0 12px',
        borderRadius: 7, background: 'var(--zx-paper-2)', border: '1px solid var(--zx-line)',
        whiteSpace: 'nowrap', overflow: 'hidden',
      }}>
        <Mark size={15} />
        <span style={{ fontSize: 12, fontWeight: 700, letterSpacing: '1.5px', color: 'var(--zx-ink)' }}>ZAPX</span>
        <span style={{ color: 'var(--zx-ink-dim)', fontSize: 12 }}>/</span>
        <span style={{ fontSize: 12, color: 'var(--zx-ink-muted)' }}>edge-router-01</span>
      </div>
    </div>
  );
}

function BrandBoard() {
  return (
    <div style={{
      width: 1180, padding: 24, display: 'flex', gap: 16,
      background: 'var(--zx-paper-15)', fontFamily: 'var(--zx-ui)',
    }}>
      <BrandCard Mark={ZapxMark} name="Actual" desc="Dos rayos cruzados, una tinta. Referencia para comparar." />
      <BrandCard Mark={MarkDuotone} name="1 · X bitono" desc="Misma geometría, dos tintas (acento + cobre). Gana profundidad sin cambiar la marca." />
      <BrandCard Mark={MarkTile} name="2 · Tile" desc="Rayo único sobre tesela redondeada. Funciona como icono de app y favicon; máxima legibilidad a 16px." />
      <BrandCard Mark={MarkPrompt} name="3 · Prompt" desc="Chevron de terminal + rayo como cursor. Cuenta qué es el producto: una terminal con chispa." />
    </div>
  );
}

Object.assign(window, { THEMES, ThemeScope, BrandBoard });
