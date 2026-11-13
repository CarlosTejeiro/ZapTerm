// zapx-option-c.jsx — Opción C · «Audaz»
// Repensar la terminal: dock de hosts como chips circulares, tabs como
// segmento flotante, terminal-tarjeta con bloques de comando (prompt +
// salida + metadatos) y composer inferior con snippets y ghost text.

function OptC() {
  const S = {
    ink: 'var(--zx-ink)', muted: 'var(--zx-ink-muted)', dim: 'var(--zx-ink-dim)',
    line: 'var(--zx-line)', accent: 'var(--zx-accent)',
  };

  function HostChip({ h, active }) {
    const initials = h.name.split('-').map((w) => w[0]).join('').slice(0, 2).toUpperCase();
    return (
      <div style={{ position: 'relative', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        {active && <span style={{
          position: 'absolute', left: -13, width: 3, height: 22, borderRadius: 2,
          background: 'var(--zx-accent)',
        }}></span>}
        <span style={{
          width: 38, height: 38, borderRadius: active ? 12 : 19, flexShrink: 0,
          background: `color-mix(in srgb, ${h.color} 24%, var(--zx-paper))`,
          color: `color-mix(in srgb, ${h.color} 72%, var(--zx-ink))`,
          border: `1px solid color-mix(in srgb, ${h.color} 40%, transparent)`,
          fontSize: 12, fontWeight: 700, fontFamily: 'var(--zx-mono)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          transition: 'border-radius .15s',
        }}>{initials}</span>
        {h.status === 'ok' && <span style={{
          position: 'absolute', right: -1, bottom: -1, width: 10, height: 10,
          borderRadius: '50%', background: 'var(--zx-ok)', border: '2px solid var(--zx-paper)',
        }}></span>}
      </div>
    );
  }

  function Block({ cmd, time, dur, children, last }) {
    return (
      <div style={{
        position: 'relative', padding: '12px 18px 14px 22px',
        borderBottom: last ? 'none' : '1px solid rgba(255,255,255,0.05)',
      }}>
        {last && <span style={{
          position: 'absolute', left: 0, top: 10, bottom: 10, width: 3,
          borderRadius: '0 2px 2px 0', background: 'var(--zx-accent)',
        }}></span>}
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, marginBottom: 6 }}>
          <span style={{ fontFamily: 'var(--zx-mono)', fontSize: 13 }}>
            <Prompt cmd={cmd} />
          </span>
          <span style={{ flex: 1 }}></span>
          {dur && <span style={{
            fontSize: 10, fontFamily: 'var(--zx-mono)', color: ZT.dim,
            border: '1px solid rgba(255,255,255,0.09)', borderRadius: 4, padding: '1px 6px',
          }}>{dur}</span>}
          <span style={{ fontSize: 10.5, fontFamily: 'var(--zx-mono)', color: ZT.dim }}>{time}</span>
        </div>
        <pre style={{
          margin: 0, fontFamily: 'var(--zx-mono)', fontSize: 12.5, lineHeight: 1.6,
          color: ZT.fg, whiteSpace: 'pre', overflow: 'hidden',
        }}>{children}</pre>
      </div>
    );
  }

  return (
    <div style={{
      width: 1440, height: 900, background: 'var(--zx-paper)', display: 'flex',
      flexDirection: 'column', fontFamily: 'var(--zx-ui)', color: S.ink, overflow: 'hidden',
    }}>

      {/* ── Barra superior mínima ── */}
      <div style={{ height: 46, display: 'flex', alignItems: 'center', flexShrink: 0, padding: '0 0 0 16px', gap: 14 }}>
        <ZapxMark size={17} />

        <span style={{ flex: 1 }}></span>

        {/* Tabs como segmento flotante */}
        <div style={{
          display: 'flex', alignItems: 'center', gap: 2, padding: 3,
          borderRadius: 'calc(var(--zx-radius) + 4px)', background: 'var(--zx-paper-2)',
          border: `1px solid ${S.line}`,
        }}>
          {TABS.map((t) => (
            <span key={t.id} style={{
              display: 'flex', alignItems: 'center', gap: 7, height: 28, padding: '0 13px',
              borderRadius: 'var(--zx-radius)', fontSize: 12.5,
              fontWeight: t.active ? 500 : 400,
              color: t.active ? S.ink : S.muted,
              background: t.active ? 'var(--zx-paper)' : 'transparent',
              boxShadow: t.active ? '0 1px 3px rgba(45,35,15,0.10)' : 'none',
            }}>
              <span style={{ width: 6, height: 6, borderRadius: '50%', background: t.status }}></span>
              {t.label}
            </span>
          ))}
          <span style={{ width: 26, height: 26, display: 'flex', alignItems: 'center', justifyContent: 'center', color: S.dim }}>
            <IconPlus size={13} />
          </span>
        </div>

        <span style={{ flex: 1 }}></span>

        <div style={{ display: 'flex', gap: 2 }}>
          {[IconSearch, IconSplit, IconCast, IconGear].map((I, i) => (
            <span key={i} style={{ width: 30, height: 30, display: 'flex', alignItems: 'center', justifyContent: 'center', borderRadius: 'var(--zx-radius)', color: S.dim }}>
              <I size={14} />
            </span>
          ))}
        </div>
        <WindowCtrls height={46} />
      </div>

      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>

        {/* ── Dock de hosts ── */}
        <div style={{
          width: 68, flexShrink: 0, display: 'flex', flexDirection: 'column',
          alignItems: 'center', gap: 12, padding: '8px 0 16px',
        }}>
          {HOSTS.map((h, i) => <HostChip key={h.id} h={h} active={i === 0} />)}
          <span style={{
            width: 38, height: 38, borderRadius: 19, display: 'flex', alignItems: 'center',
            justifyContent: 'center', color: S.dim, border: `1.5px dashed ${S.line}`,
          }}><IconPlus size={15} /></span>
          <span style={{ flex: 1 }}></span>
          <span style={{ color: S.dim }}><IconBook size={16} /></span>
        </div>

        {/* ── Terminal-tarjeta con bloques ── */}
        <div style={{ flex: 1, padding: '2px 20px 18px 4px', minWidth: 0 }}>
          <div style={{
            height: '100%', display: 'flex', flexDirection: 'column',
            borderRadius: 'calc(var(--zx-radius) + 8px)', background: ZT.bg, overflow: 'hidden',
            boxShadow: '0 2px 6px rgba(45,35,15,0.10), 0 18px 50px rgba(45,35,15,0.16)',
          }}>

            {/* Cabecera de la tarjeta */}
            <div style={{
              height: 38, display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0,
              padding: '0 18px', borderBottom: '1px solid rgba(255,255,255,0.06)',
            }}>
              <Led color={ZT.green} pulse />
              <span style={{ fontSize: 12.5, fontWeight: 500, color: ZT.white }}>edge-router-01</span>
              <span style={{ fontSize: 11, fontFamily: 'var(--zx-mono)', color: ZT.dim }}>admin@10.0.12.1</span>
              <span style={{
                fontSize: 10, fontFamily: 'var(--zx-mono)', color: ZT.cyan, letterSpacing: '0.5px',
                border: '1px solid rgba(111,189,188,0.35)', borderRadius: 4, padding: '1px 6px',
              }}>SSH</span>
              <span style={{ flex: 1 }}></span>
              <span style={{ fontSize: 10.5, fontFamily: 'var(--zx-mono)', color: ZT.dim }}>up 2:14:09 · 12 ms</span>
            </div>

            {/* Bloques */}
            <div style={{ flex: 1, overflow: 'hidden', minHeight: 0 }}>
              <Block cmd="show ip interface brief" time="14:02:11" dur="42 ms">
                <ShowBriefOut />
              </Block>
              <Block cmd="ping 10.0.13.2" time="14:03:48" dur="2.1 s">
                <PingOut />
              </Block>
              <Block cmd="show version | include uptime" time="14:05:02" dur="38 ms" last>
                edge-router-01 uptime is 24 weeks, 3 days, 11 hours, 2 minutes
              </Block>
            </div>

            {/* Composer */}
            <div style={{ flexShrink: 0, padding: '0 14px 14px' }}>
              <div style={{
                borderRadius: 'calc(var(--zx-radius) + 4px)', background: ZT.bgDeep,
                border: '1px solid rgba(255,255,255,0.08)', padding: '10px 14px 9px',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 9, fontFamily: 'var(--zx-mono)', fontSize: 13 }}>
                  <span style={{ color: 'var(--zx-accent-soft)' }}>❯</span>
                  <span style={{ color: ZT.white }}>show run</span>
                  <span style={{ color: ZT.dim }}>ning-config | section bgp</span>
                  <span style={{ width: 8, height: 16, background: ZT.violet, borderRadius: 1 }}></span>
                  <span style={{ flex: 1 }}></span>
                  <span style={{ fontSize: 10, color: ZT.dim, border: '1px solid rgba(255,255,255,0.10)', borderRadius: 4, padding: '1px 6px' }}>TAB</span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 9 }}>
                  {['wr mem', 'sh ip bgp sum', 'sh log | inc DOWN'].map((s) => (
                    <span key={s} style={{
                      fontSize: 10.5, fontFamily: 'var(--zx-mono)', color: ZT.cyan,
                      background: 'rgba(111,189,188,0.10)', border: '1px solid rgba(111,189,188,0.22)',
                      borderRadius: 5, padding: '2px 8px',
                    }}>{s}</span>
                  ))}
                  <span style={{ flex: 1 }}></span>
                  <span style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 10.5, color: ZT.dim, fontFamily: 'var(--zx-mono)' }}>
                    <IconCast size={12} /> MULTI OFF
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

window.OptC = OptC;
