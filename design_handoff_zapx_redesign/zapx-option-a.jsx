// zapx-option-a.jsx — Opción A · «Pulido»
// Misma estructura que la app actual, ejecución refinada: iconografía SVG
// unificada, ritmo base-8, terminal como tarjeta flotante, tabs sin trucos
// de borde, status bar más legible.

function OptA({ Mark = ZapxMark }) {
  const S = {
    ink: 'var(--zx-ink)', muted: 'var(--zx-ink-muted)', dim: 'var(--zx-ink-dim)',
    line: 'var(--zx-line)', accent: 'var(--zx-accent)',
  };

  const menuItems = ['File', 'View', 'Session', 'Tools', 'Help'];

  return (
    <div style={{
      width: 1440, height: 900, background: 'var(--zx-paper)', display: 'flex',
      flexDirection: 'column', fontFamily: 'var(--zx-ui)', color: S.ink,
      overflow: 'hidden', position: 'relative',
    }}>

      {/* ── Titlebar ── */}
      <div style={{
        height: 38, display: 'flex', alignItems: 'center', flexShrink: 0,
        background: 'var(--zx-paper-2)', borderBottom: `1px solid ${S.line}`,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 14px', whiteSpace: 'nowrap', flexShrink: 0 }}>
          <Mark size={15} />
          <span style={{ fontSize: 12, fontWeight: 700, letterSpacing: '1.5px' }}>ZAPX</span>
          <span style={{ color: S.dim, fontSize: 12 }}>/</span>
          <span style={{ fontSize: 12, color: S.muted }}>edge-router-01</span>
        </div>
        <div style={{ flex: 1 }}></div>
        <nav style={{ display: 'flex', alignItems: 'center' }}>
          {menuItems.map((m, i) => (
            <span key={m} style={{
              fontSize: 12, padding: '4px 10px', borderRadius: 5,
              color: i === 1 ? S.ink : S.muted,
              background: i === 1 ? 'var(--zx-hover)' : 'transparent',
            }}>{m}</span>
          ))}
        </nav>
        <div style={{ width: 10 }}></div>
        <WindowCtrls height={38} />
      </div>

      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>

        {/* ── Sidebar ── */}
        <aside style={{
          width: 248, flexShrink: 0, display: 'flex', flexDirection: 'column',
          background: 'var(--zx-paper-2)', borderRight: `1px solid ${S.line}`,
        }}>
          {/* Search */}
          <div style={{ padding: '12px 12px 8px' }}>
            <div style={{
              display: 'flex', alignItems: 'center', gap: 8, height: 30,
              padding: '0 10px', borderRadius: 'var(--zx-radius)',
              background: 'var(--zx-paper)', border: `1px solid ${S.line}`,
              color: S.dim,
            }}>
              <IconSearch size={13} />
              <span style={{ fontSize: 12.5, whiteSpace: 'nowrap', overflow: 'hidden' }}>Search sessions…</span>
              <span style={{ marginLeft: 'auto', fontSize: 10.5, fontFamily: 'var(--zx-mono)', border: `1px solid ${S.line}`, borderRadius: 4, padding: '1px 5px' }}>⌘K</span>
            </div>
          </div>

          {/* Section header */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '8px 14px 4px', color: S.dim }}>
            <IconChevron open size={11} />
            <span style={{ fontSize: 10.5, fontWeight: 600, letterSpacing: '1px' }}>SESSIONS</span>
            <span style={{ fontSize: 10.5 }}>4</span>
            <span style={{ flex: 1 }}></span>
            <IconFolder size={13} />
            <IconPlus size={13} />
          </div>

          {/* Rows */}
          <div style={{ padding: '2px 8px' }}>
            {HOSTS.map((h, i) => {
              const active = i === 0;
              return (
                <div key={h.id} style={{
                  display: 'flex', alignItems: 'center', gap: 9, height: 32,
                  padding: '0 8px', borderRadius: 'var(--zx-radius)',
                  background: active ? 'color-mix(in srgb, var(--zx-accent) 10%, transparent)' : 'transparent',
                }}>
                  <span style={{ width: 7, height: 7, borderRadius: '50%', background: h.color, flexShrink: 0 }}></span>
                  <span style={{
                    fontSize: 13, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                    color: active ? S.ink : S.muted, fontWeight: active ? 500 : 400,
                  }}>{h.name}</span>
                  {active
                    ? <span style={{ display: 'flex', gap: 8, color: S.dim }}><IconPencil size={12} /><IconX size={12} /></span>
                    : <span style={{
                        fontSize: 9.5, fontFamily: 'var(--zx-mono)', color: S.dim,
                        border: `1px solid ${S.line}`, borderRadius: 4, padding: '1px 5px', letterSpacing: '0.5px',
                      }}>{h.proto}</span>}
                </div>
              );
            })}
          </div>

          {/* Folders */}
          <div style={{ padding: '6px 8px' }}>
            {FOLDERS.map((f, fi) => (
              <div key={f.name}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, height: 28, padding: '0 6px', color: S.dim }}>
                  <IconChevron open={fi === 0} size={11} />
                  <IconFolder size={13} />
                  <span style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.8px', textTransform: 'uppercase', whiteSpace: 'nowrap' }}>{f.name}</span>
                  <span style={{ fontSize: 10.5 }}>{f.count}</span>
                </div>
                {fi === 0 && f.hosts.map((h) => (
                  <div key={h.id} style={{ display: 'flex', alignItems: 'center', gap: 9, height: 30, padding: '0 8px 0 26px' }}>
                    <span style={{ width: 6, height: 6, borderRadius: '50%', background: h.color, flexShrink: 0 }}></span>
                    <span style={{ fontSize: 12.5, color: S.muted, flex: 1 }}>{h.name}</span>
                    {h.proto !== 'SSH' && <span style={{
                      fontSize: 9.5, fontFamily: 'var(--zx-mono)', color: S.dim,
                      border: `1px solid ${S.line}`, borderRadius: 4, padding: '1px 5px',
                    }}>{h.proto}</span>}
                  </div>
                ))}
              </div>
            ))}
          </div>

          <div style={{ flex: 1 }}></div>

          {/* Footer */}
          <div style={{
            display: 'flex', alignItems: 'center', gap: 9, padding: '10px 14px',
            borderTop: `1px solid ${S.line}`,
          }}>
            <span style={{
              width: 24, height: 24, borderRadius: 7, background: 'var(--zx-accent)',
              color: 'var(--zx-paper)', fontSize: 10, fontWeight: 700,
              display: 'flex', alignItems: 'center', justifyContent: 'center', letterSpacing: '0.5px',
            }}>AD</span>
            <span style={{ fontSize: 12, color: S.muted, flex: 1 }}>admin</span>
            <span style={{ color: S.dim }}><IconKey size={14} /></span>
            <span style={{ color: S.dim }}><IconGear size={14} /></span>
          </div>
        </aside>

        {/* ── Main ── */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>

          {/* Tab bar */}
          <div style={{
            height: 42, display: 'flex', alignItems: 'center', flexShrink: 0,
            padding: '0 10px', gap: 4, background: 'var(--zx-paper)',
            borderBottom: `1px solid ${S.line}`,
          }}>
            {TABS.map((t) => (
              <div key={t.id} style={{
                display: 'flex', alignItems: 'center', gap: 8, height: 30,
                padding: '0 12px', borderRadius: 'var(--zx-radius)',
                background: t.active ? 'var(--zx-paper-2)' : 'transparent',
                border: t.active ? `1px solid ${S.line}` : '1px solid transparent',
                boxShadow: t.active ? `inset 0 -2px 0 var(--zx-accent)` : 'none',
                minWidth: 140,
              }}>
                <span style={{ width: 7, height: 7, borderRadius: '50%', background: t.status }}></span>
                <span style={{
                  fontSize: 12.5, flex: 1, color: t.active ? S.ink : S.muted,
                  fontWeight: t.active ? 500 : 400,
                }}>{t.label}</span>
                {t.active && <span style={{ color: S.dim }}><IconX size={11} /></span>}
              </div>
            ))}
            <span style={{
              width: 28, height: 28, display: 'flex', alignItems: 'center',
              justifyContent: 'center', borderRadius: 'var(--zx-radius)', color: S.dim,
            }}><IconPlus size={14} /></span>

            <span style={{ flex: 1 }}></span>

            {[{ ic: IconSplit, label: 'Split' }, { ic: IconCast, label: 'Multi' }].map(({ ic: I, label }) => (
              <span key={label} style={{
                display: 'flex', alignItems: 'center', gap: 6, height: 28,
                padding: '0 10px', borderRadius: 'var(--zx-radius)', fontSize: 11.5,
                fontWeight: 500, color: S.muted, border: `1px solid ${S.line}`,
              }}><I size={13} />{label}</span>
            ))}
          </div>

          {/* Terminal flotante */}
          <div style={{ flex: 1, padding: 14, minHeight: 0 }}>
            <div style={{
              height: '100%', borderRadius: 'calc(var(--zx-radius) + 4px)',
              background: ZT.bg, overflow: 'hidden', position: 'relative',
              boxShadow: '0 1px 2px rgba(45,35,15,0.10), 0 8px 28px rgba(45,35,15,0.10)',
            }}>
              <TermClassic />
              {/* pista de scrollbar */}
              <div style={{ position: 'absolute', top: 12, right: 5, bottom: 12, width: 4, borderRadius: 2, background: 'rgba(255,255,255,0.04)' }}>
                <div style={{ position: 'absolute', bottom: 0, width: 4, height: '38%', borderRadius: 2, background: 'rgba(255,255,255,0.14)' }}></div>
              </div>
            </div>
          </div>

          {/* Status bar */}
          <div style={{
            height: 28, display: 'flex', alignItems: 'center', flexShrink: 0,
            padding: '0 12px', fontFamily: 'var(--zx-mono)', fontSize: 11,
            color: S.dim, background: 'var(--zx-paper-2)', borderTop: `1px solid ${S.line}`,
          }}>
            <Seg color="var(--zx-ok)"><Led color="var(--zx-ok)" pulse /> CONNECTED</Seg>
            <SegDiv />
            <Seg color={S.muted}>admin@10.0.12.1</Seg>
            <SegDiv />
            <Seg>ssh <span style={{ color: S.muted }}>22</span></Seg>
            <SegDiv />
            <Seg>mss <span style={{ color: S.muted }}>1448↑ / 1460↓</span></Seg>
            <SegDiv />
            <Seg color={S.muted}>up 2:14:09</Seg>
            <SegDiv />
            <Seg color={S.muted}>12 ms</Seg>
            <span style={{ flex: 1 }}></span>
            <Seg>utf-8</Seg>
            <SegDiv />
            <Seg>VT320</Seg>
            <SegDiv />
            <Seg>1×1</Seg>
          </div>
        </div>
      </div>
    </div>
  );
}

window.OptA = OptA;
