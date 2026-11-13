// zapx-option-b.jsx — Opción B · «Evolución»
// Chrome unificado: tabs viven en la barra de título (estilo Zed/VS Code),
// rail de iconos + panel de hosts con avatares (estilo Termius),
// terminal integrada a sangre con franja de contexto.

function OptB() {
  const S = {
    ink: 'var(--zx-ink)', muted: 'var(--zx-ink-muted)', dim: 'var(--zx-ink-dim)',
    line: 'var(--zx-line)', accent: 'var(--zx-accent)',
  };

  const railIcons = [
    { I: IconTerm, active: true, title: 'Sessions' },
    { I: IconBook, title: 'Snippets' },
    { I: IconClock, title: 'Logs' },
    { I: IconBolt, title: 'Highlight rules' },
  ];

  function Avatar({ h, size = 26 }) {
    const initials = h.name.split('-').map((w) => w[0]).join('').slice(0, 2).toUpperCase();
    return (
      <span style={{
        width: size, height: size, borderRadius: 7, flexShrink: 0,
        background: `color-mix(in srgb, ${h.color} 22%, var(--zx-paper))`,
        color: `color-mix(in srgb, ${h.color} 70%, var(--zx-ink))`,
        border: `1px solid color-mix(in srgb, ${h.color} 35%, transparent)`,
        fontSize: size * 0.36, fontWeight: 700, letterSpacing: '0.5px',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        fontFamily: 'var(--zx-mono)',
      }}>{initials}</span>
    );
  }

  return (
    <div style={{
      width: 1440, height: 900, background: 'var(--zx-paper)', display: 'flex',
      flexDirection: 'column', fontFamily: 'var(--zx-ui)', color: S.ink,
      overflow: 'hidden',
    }}>

      {/* ── Barra única: marca + tabs + acciones + controles ── */}
      <div style={{
        height: 44, display: 'flex', alignItems: 'center', flexShrink: 0,
        background: 'var(--zx-paper-2)', borderBottom: `1px solid ${S.line}`,
        gap: 6, paddingLeft: 4,
      }}>
        <div style={{ width: 44, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <ZapxMark size={17} />
        </div>

        {/* Tabs como pills */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          {TABS.map((t) => (
            <div key={t.id} style={{
              display: 'flex', alignItems: 'center', gap: 8, height: 32,
              padding: '0 13px', borderRadius: 'var(--zx-radius)',
              background: t.active ? 'var(--zx-paper)' : 'transparent',
              border: t.active ? `1px solid ${S.line}` : '1px solid transparent',
              boxShadow: t.active ? '0 1px 2px rgba(45,35,15,0.07)' : 'none',
              minWidth: 148,
            }}>
              <span style={{ width: 7, height: 7, borderRadius: '50%', background: t.status }}></span>
              <span style={{
                fontSize: 12.5, flex: 1, fontWeight: t.active ? 500 : 400,
                color: t.active ? S.ink : S.muted,
              }}>{t.label}</span>
              {t.active && <span style={{ color: S.dim }}><IconX size={11} /></span>}
            </div>
          ))}
          <span style={{ width: 30, height: 30, display: 'flex', alignItems: 'center', justifyContent: 'center', color: S.dim }}>
            <IconPlus size={14} />
          </span>
        </div>

        <span style={{ flex: 1 }}></span>

        {/* Palette-forward: buscador central */}
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8, height: 30, width: 280,
          padding: '0 12px', borderRadius: 'var(--zx-radius)',
          background: 'var(--zx-paper)', border: `1px solid ${S.line}`, color: S.dim,
        }}>
          <IconSearch size={13} />
          <span style={{ fontSize: 12 }}>Search or run a command…</span>
          <span style={{ marginLeft: 'auto', fontSize: 10.5, fontFamily: 'var(--zx-mono)', border: `1px solid ${S.line}`, borderRadius: 4, padding: '1px 5px' }}>⌘K</span>
        </div>

        <div style={{ display: 'flex', gap: 2, padding: '0 6px' }}>
          {[IconSplit, IconCast].map((I, i) => (
            <span key={i} style={{ width: 30, height: 30, display: 'flex', alignItems: 'center', justifyContent: 'center', borderRadius: 'var(--zx-radius)', color: S.dim }}>
              <I size={14} />
            </span>
          ))}
        </div>
        <WindowCtrls height={44} />
      </div>

      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>

        {/* ── Rail de iconos ── */}
        <div style={{
          width: 46, flexShrink: 0, display: 'flex', flexDirection: 'column',
          alignItems: 'center', gap: 4, padding: '10px 0',
          background: 'var(--zx-paper-2)', borderRight: `1px solid ${S.line}`,
        }}>
          {railIcons.map(({ I, active, title }) => (
            <span key={title} title={title} style={{
              width: 32, height: 32, display: 'flex', alignItems: 'center', justifyContent: 'center',
              borderRadius: 'var(--zx-radius)',
              color: active ? 'var(--zx-accent)' : S.dim,
              background: active ? 'color-mix(in srgb, var(--zx-accent) 12%, transparent)' : 'transparent',
            }}><I size={16} /></span>
          ))}
          <span style={{ flex: 1 }}></span>
          <span style={{ width: 32, height: 32, display: 'flex', alignItems: 'center', justifyContent: 'center', color: S.dim }}>
            <IconGear size={16} />
          </span>
        </div>

        {/* ── Panel de hosts ── */}
        <aside style={{
          width: 252, flexShrink: 0, display: 'flex', flexDirection: 'column',
          background: 'var(--zx-paper-15)', borderRight: `1px solid ${S.line}`,
        }}>
          <div style={{ display: 'flex', alignItems: 'center', padding: '14px 14px 10px' }}>
            <span style={{ fontSize: 13, fontWeight: 600 }}>Hosts</span>
            <span style={{ marginLeft: 8, fontSize: 11, color: S.dim, fontFamily: 'var(--zx-mono)' }}>9</span>
            <span style={{ flex: 1 }}></span>
            <span style={{ color: S.dim }}><IconFolder size={14} /></span>
            <span style={{ width: 10 }}></span>
            <span style={{
              width: 22, height: 22, borderRadius: 6, display: 'flex', alignItems: 'center',
              justifyContent: 'center', background: 'var(--zx-accent)', color: 'var(--zx-paper)',
            }}><IconPlus size={12} /></span>
          </div>

          <div style={{ padding: '0 8px' }}>
            {HOSTS.map((h, i) => {
              const active = i === 0;
              return (
                <div key={h.id} style={{
                  display: 'flex', alignItems: 'center', gap: 10, padding: '7px 8px',
                  borderRadius: 'var(--zx-radius)',
                  background: active ? 'color-mix(in srgb, var(--zx-accent) 10%, transparent)' : 'transparent',
                }}>
                  <Avatar h={h} />
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 1, minWidth: 0, flex: 1 }}>
                    <span style={{ fontSize: 12.5, fontWeight: active ? 500 : 400, color: active ? S.ink : S.muted, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{h.name}</span>
                    <span style={{ fontSize: 10.5, color: S.dim, fontFamily: 'var(--zx-mono)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{h.host}</span>
                  </span>
                  {active
                    ? <Led color="var(--zx-ok)" pulse />
                    : <span style={{ fontSize: 9, fontFamily: 'var(--zx-mono)', color: S.dim, letterSpacing: '0.5px' }}>{h.proto}</span>}
                </div>
              );
            })}
          </div>

          <div style={{ padding: '12px 14px 4px', display: 'flex', alignItems: 'center', gap: 6, color: S.dim }}>
            <IconChevron open size={10} />
            <span style={{ fontSize: 10.5, fontWeight: 600, letterSpacing: '1px' }}>DATACENTER MAD</span>
            <span style={{ fontSize: 10.5 }}>3</span>
          </div>
          <div style={{ padding: '0 8px' }}>
            {FOLDERS[0].hosts.map((h) => (
              <div key={h.id} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '6px 8px 6px 16px' }}>
                <Avatar h={h} size={22} />
                <span style={{ fontSize: 12.5, color: S.muted, flex: 1 }}>{h.name}</span>
                <span style={{ fontSize: 9, fontFamily: 'var(--zx-mono)', color: S.dim }}>{h.proto}</span>
              </div>
            ))}
          </div>
          <div style={{ padding: '10px 14px 4px', display: 'flex', alignItems: 'center', gap: 6, color: S.dim }}>
            <IconChevron size={10} />
            <span style={{ fontSize: 10.5, fontWeight: 600, letterSpacing: '1px' }}>LAB</span>
            <span style={{ fontSize: 10.5 }}>2</span>
          </div>
        </aside>

        {/* ── Terminal integrada ── */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, background: ZT.bg }}>

          {/* Franja de contexto */}
          <div style={{
            height: 34, display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0,
            padding: '0 16px', background: ZT.bgDeep,
            borderBottom: '1px solid rgba(255,255,255,0.06)',
          }}>
            <Led color={ZT.green} pulse />
            <span style={{ fontSize: 12, fontWeight: 500, color: ZT.fg }}>edge-router-01</span>
            <span style={{ fontSize: 11, fontFamily: 'var(--zx-mono)', color: ZT.dim }}>admin@10.0.12.1 · ssh 22</span>
            <span style={{ flex: 1 }}></span>
            <span style={{ fontSize: 10.5, fontFamily: 'var(--zx-mono)', color: ZT.dim }}>12 ms</span>
            <span style={{
              fontSize: 10, fontFamily: 'var(--zx-mono)', color: ZT.cyan, letterSpacing: '0.5px',
              border: `1px solid rgba(111,189,188,0.35)`, borderRadius: 4, padding: '1px 6px',
            }}>SSH</span>
          </div>

          <div style={{ flex: 1, minHeight: 0 }}>
            <TermClassic pad="16px 20px" />
          </div>

          {/* Status integrada en la zona oscura */}
          <div style={{
            height: 26, display: 'flex', alignItems: 'center', flexShrink: 0,
            padding: '0 12px', fontFamily: 'var(--zx-mono)', fontSize: 10.5,
            color: ZT.dim, background: ZT.bgDeep, borderTop: '1px solid rgba(255,255,255,0.05)',
          }}>
            <Seg>mss <span style={{ color: ZT.fg, opacity: 0.7 }}>1448↑ / 1460↓</span></Seg>
            <SegDiv />
            <Seg>up 2:14:09</Seg>
            <SegDiv />
            <Seg>↓ 1.2 KB/s</Seg>
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

window.OptB = OptB;
