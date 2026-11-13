// zapx-shared.jsx — tokens, iconos SVG consistentes y contenido de terminal
// compartido por las tres opciones de rediseño.

// ── Paleta terminal (var-driven: cada tema puede sobreescribir --zt-*) ──────
const ZT = {
  bg: 'var(--zt-bg, #20212b)',
  bgDeep: 'var(--zt-deep, #1a1b24)',
  fg: 'var(--zt-fg, #d6d8e0)',
  dim: 'var(--zt-dim, #5c5f74)',
  cyan: 'var(--zt-cyan, #6fbdbc)',
  green: 'var(--zt-green, #5fb784)',
  red: 'var(--zt-red, #e07474)',
  amber: 'var(--zt-amber, #d8a85e)',
  violet: 'var(--zt-cursor, #9a91e8)',
  white: 'var(--zt-white, #eceef4)',
};

// ── Iconos: set único de SVG stroke 1.5, 14×14 ──────────────────────────────
function Ic({ d, size = 14, sw = 1.5, children, style }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none"
      stroke="currentColor" strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round"
      style={{ flexShrink: 0, display: 'block', ...style }}>
      {d ? <path d={d} /> : children}
    </svg>
  );
}
const IconSearch = (p) => <Ic {...p}><circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5 L14 14" /></Ic>;
const IconPlus = (p) => <Ic {...p} d="M8 3v10M3 8h10" />;
const IconFolder = (p) => <Ic {...p} d="M2 4.5h4l1.5 2H14v6.5H2z" />;
const IconChevron = ({ open, ...p }) => (
  <Ic {...p} style={{ transform: open ? 'rotate(90deg)' : 'none', transition: 'transform .12s', ...(p.style || {}) }} d="M6 3.5 L10.5 8 L6 12.5" />
);
const IconPencil = (p) => <Ic {...p} d="M3 13l.8-3L11 2.8a1.2 1.2 0 0 1 1.7 0l.5.5a1.2 1.2 0 0 1 0 1.7L6 12.2z" />;
const IconX = (p) => <Ic {...p} d="M4 4l8 8M12 4l-8 8" />;
const IconSplit = (p) => <Ic {...p}><rect x="2" y="2.5" width="5" height="11" rx="1.2" /><rect x="9" y="2.5" width="5" height="11" rx="1.2" /></Ic>;
const IconCast = (p) => <Ic {...p}><circle cx="8" cy="8" r="1.4" /><path d="M4.8 11.2a4.5 4.5 0 0 1 0-6.4M11.2 4.8a4.5 4.5 0 0 1 0 6.4" /><path d="M2.8 13.2a7.2 7.2 0 0 1 0-10.4M13.2 2.8a7.2 7.2 0 0 1 0 10.4" /></Ic>;
const IconGear = (p) => <Ic {...p}><circle cx="8" cy="8" r="2.2" /><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M12.4 3.6 11 5M5 11l-1.4 1.4" /></Ic>;
const IconTerm = (p) => <Ic {...p}><path d="M3 5l3.5 3L3 11" /><path d="M8.5 12H13" /></Ic>;
const IconBook = (p) => <Ic {...p}><path d="M3 2.5h7.5a2 2 0 0 1 2 2V13.5H5a2 2 0 0 0-2 2z" /><path d="M3 13.5a2 2 0 0 1 2-2h7.5" /></Ic>;
const IconClock = (p) => <Ic {...p}><circle cx="8" cy="8" r="5.8" /><path d="M8 4.8V8l2.2 1.6" /></Ic>;
const IconBolt = (p) => <Ic {...p} d="M8.8 1.5 3.5 9h3.4l-.7 5.5L11.5 7H8.1z" />;
const IconKey = (p) => <Ic {...p}><circle cx="5.5" cy="10.5" r="3" /><path d="M7.8 8.2 13.5 2.5M11 5l2 2M9.5 6.5l1.5 1.5" /></Ic>;
const IconMin = (p) => <Ic {...p} d="M3.5 8h9" />;
const IconMax = (p) => <Ic {...p}><rect x="3.5" y="3.5" width="9" height="9" rx="1" /></Ic>;

// ── Marca ZAPX — propuestas ──────────────────────────────────────────────────
const BOLT_PATH = 'M 5 2 L 12 2 L 9 7 L 13 7 L 3 14 L 8 9 L 4 9 Z';

// Actual del repo: dos rayos cruzados, un solo color
function ZapxMark({ size = 16, color = 'var(--zx-accent)' }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <g fill={color}>
        <path d={BOLT_PATH} transform="rotate(45 8 8)" />
        <path d={BOLT_PATH} transform="rotate(-45 8 8)" />
      </g>
    </svg>
  );
}

// Propuesta 1 · «X bitono»: misma geometría, dos tintas (acento + cobre)
function MarkDuotone({ size = 16 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <path d={BOLT_PATH} transform="rotate(45 8 8)" fill="var(--zx-accent-2, #b3793a)" opacity="0.85" />
      <path d={BOLT_PATH} transform="rotate(-45 8 8)" fill="var(--zx-accent)" stroke="var(--zx-paper, #f7f4ed)" strokeWidth="0.8" />
    </svg>
  );
}

// Propuesta 2 · «Tile»: rayo único sobre tesela redondeada (estilo app icon)
function MarkTile({ size = 16 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <rect x="0.5" y="0.5" width="15" height="15" rx="4.2" fill="var(--zx-accent)" />
      <path d="M 9.1 2.6 L 5 8.6 H 7.6 L 6.6 13.4 L 11.4 6.8 H 8.6 Z" fill="var(--zx-paper, #f7f4ed)" />
    </svg>
  );
}

// Propuesta 3 · «Prompt»: chevron de terminal + rayo como cursor
function MarkPrompt({ size = 16 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <path d="M2.5 3.5 L7.5 8 L2.5 12.5" stroke="var(--zx-accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M 12.1 2.8 L 8.8 7.6 H 10.9 L 10.1 11.4 L 13.9 6.2 H 11.7 Z" fill="var(--zx-accent-2, #b3793a)" transform="translate(0.6 2)" />
    </svg>
  );
}

const MARKS = { actual: ZapxMark, duotone: MarkDuotone, tile: MarkTile, prompt: MarkPrompt };

// ── Controles de ventana (SVG, no glifos de texto) ───────────────────────────
function WindowCtrls({ color = 'var(--zx-ink-dim)', height = 36 }) {
  const btn = {
    width: 44, height, display: 'flex', alignItems: 'center',
    justifyContent: 'center', color,
  };
  return (
    <div style={{ display: 'flex', flexShrink: 0, height }}>
      <div style={btn}><IconMin size={13} /></div>
      <div style={btn}><IconMax size={12} /></div>
      <div style={btn}><IconX size={13} /></div>
    </div>
  );
}

// ── Datos de sesiones de ejemplo ─────────────────────────────────────────────
const HOSTS = [
  { id: 1, name: 'edge-router-01', host: 'admin@10.0.12.1', proto: 'SSH', color: '#5eb3b2', status: 'ok' },
  { id: 2, name: 'core-sw-lab', host: 'admin@10.0.20.4', proto: 'SSH', color: '#c89b6b', status: 'ok' },
  { id: 3, name: 'fw-branch-2', host: 'ops@172.16.8.1', proto: 'SSH', color: '#9a91e8', status: 'idle' },
  { id: 4, name: 'oob-console', host: 'COM3 · 9600 8N1', proto: 'SERIAL', color: '#d8a85e', status: 'idle' },
];
const FOLDERS = [
  { name: 'Datacenter MAD', count: 3, hosts: [
    { id: 5, name: 'spine-01', host: 'admin@10.1.0.1', proto: 'SSH', color: '#5eb3b2' },
    { id: 6, name: 'spine-02', host: 'admin@10.1.0.2', proto: 'SSH', color: '#5eb3b2' },
    { id: 7, name: 'leaf-101', host: 'admin@10.1.1.1', proto: 'TELNET', color: '#e08c8c' },
  ]},
  { name: 'Lab', count: 2, hosts: [] },
];
const TABS = [
  { id: 1, label: 'edge-router-01', status: ZT.green, active: true },
  { id: 2, label: 'core-sw-lab', status: ZT.green, active: false },
  { id: 4, label: 'oob-console', status: ZT.amber, active: false },
];

// ── Contenido de terminal Cisco con keyword highlighting ────────────────────
function Prompt({ host = 'edge-router-01', cmd }) {
  return (
    <span>
      <span style={{ color: ZT.cyan }}>{host}</span>
      <span style={{ color: ZT.dim }}>#</span>{' '}
      <span style={{ color: ZT.white, fontWeight: 500 }}>{cmd}</span>
    </span>
  );
}

function ShowBriefOut() {
  const up = <span style={{ color: ZT.green }}>up</span>;
  const down = <span style={{ color: ZT.red }}>down</span>;
  const admDown = <span style={{ color: ZT.red }}>administratively down</span>;
  const ip = (s) => <span style={{ color: ZT.amber }}>{s}</span>;
  const ifc = (s) => <span style={{ color: ZT.cyan }}>{s}</span>;
  return (
    <span>
      <span style={{ color: ZT.dim }}>Interface              IP-Address      OK? Method Status                Protocol</span>{'\n'}
      {ifc('GigabitEthernet0/0')}     {ip('10.0.12.1')}       YES NVRAM  {up}                    {up}{'\n'}
      {ifc('GigabitEthernet0/1')}     {ip('10.0.13.1')}       YES NVRAM  {up}                    {up}{'\n'}
      {ifc('GigabitEthernet0/2')}     unassigned      YES NVRAM  {admDown} {down}{'\n'}
      {ifc('Serial0/0/0')}            {ip('192.168.5.2')}     YES manual {up}                    {up}
    </span>
  );
}

function PingOut() {
  return (
    <span>
      Type escape sequence to abort.{'\n'}
      Sending 5, 100-byte ICMP Echos to <span style={{ color: ZT.amber }}>10.0.13.2</span>, timeout is 2 seconds:{'\n'}
      <span style={{ color: ZT.green }}>!!!!!</span>{'\n'}
      Success rate is <span style={{ color: ZT.green }}>100 percent</span> (5/5), round-trip min/avg/max = 1/2/4 ms
    </span>
  );
}

// Terminal "clásica": scroll continuo (Opciones A y B)
function TermClassic({ pad = '14px 18px', fontSize = 13 }) {
  return (
    <pre style={{
      margin: 0, padding: pad, fontFamily: 'var(--zx-mono)', fontSize,
      lineHeight: 1.6, color: ZT.fg, whiteSpace: 'pre', overflow: 'hidden',
      height: '100%',
    }}>
      <Prompt cmd="show ip interface brief" />{'\n'}
      <ShowBriefOut />{'\n'}
      <Prompt cmd="show version | include uptime" />{'\n'}
      edge-router-01 uptime is 24 weeks, 3 days, 11 hours, 2 minutes{'\n'}
      <Prompt cmd="ping 10.0.13.2" />{'\n'}
      <PingOut />{'\n'}
      <Prompt cmd="" />
      <span style={{
        display: 'inline-block', width: 8, height: 16, background: ZT.violet,
        verticalAlign: 'text-bottom', borderRadius: 1,
      }}></span>
    </pre>
  );
}

// Barra segmento de status (mono)
function Seg({ children, color = 'inherit' }) {
  return <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5, padding: '0 7px', color, whiteSpace: 'nowrap' }}>{children}</span>;
}
function SegDiv() {
  return <span style={{ opacity: 0.25 }}>│</span>;
}
function Led({ color, pulse }) {
  return <span style={{
    width: 6, height: 6, borderRadius: '50%', background: color, flexShrink: 0,
    boxShadow: pulse ? `0 0 5px ${color}` : 'none',
  }}></span>;
}

Object.assign(window, {
  ZT, ZapxMark, MarkDuotone, MarkTile, MarkPrompt, MARKS,
  WindowCtrls, HOSTS, FOLDERS, TABS,
  Prompt, ShowBriefOut, PingOut, TermClassic, Seg, SegDiv, Led,
  IconSearch, IconPlus, IconFolder, IconChevron, IconPencil, IconX, IconSplit,
  IconCast, IconGear, IconTerm, IconBook, IconClock, IconBolt, IconKey,
  IconMin, IconMax,
});
