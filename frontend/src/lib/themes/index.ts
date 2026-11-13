export interface PylonTheme {
  name: string

  // Fonts
  fontUi: string
  fontMono: string

  // Chrome layout
  appBg: string
  bodyBg: string
  sidebarBg: string
  titlebarBg: string
  statusbarBg: string
  tabBarBg: string

  // Tabs
  tabActiveBg: string

  // Interactive
  accent: string
  accent2: string
  /** Foreground for text/icons sitting on top of an `accent`-filled surface
   *  (e.g. primary buttons). Dark on light accents, light on dark accents. */
  onAccent: string
  itemHoverBg: string
  itemActiveBg: string
  itemActiveBorder: string

  // Text
  textPrimary: string
  textMuted: string
  textDim: string

  // Borders
  border: string
  radius: string

  // Status / connection
  ok: string
  warn: string
  err: string

  // Terminal
  terminal: {
    bg: string
    fg: string
    cursor: string
    dim: string
    ok: string
    warn: string
    err: string
    // ANSI
    black: string
    red: string
    green: string
    yellow: string
    blue: string
    magenta: string
    cyan: string
    white: string
    brightBlack: string
    brightRed: string
    brightGreen: string
    brightYellow: string
    brightBlue: string
    brightMagenta: string
    brightCyan: string
    brightWhite: string
  }

  // Effects
  glows: boolean
  windowShadow: string
}

/* ---------------------------------------------------------------------------
 * The 5 bundled themes: two light (Parchment, Porcelain) and three dark with
 * distinct characters (Midnight, Aurora, Ember). Except for Parchment, which
 * keeps its soft, low-contrast chrome, they use three clearly separated
 * surface levels — appBg/bodyBg (content), tabBarBg (one step towards the
 * sidebar), sidebarBg/titlebarBg/statusbarBg (frame) — and tabActiveBg is
 * the surface that stands out most from the tab bar. Tabs are
 * tinted with each session's own colour on top of these (see TabBar).
 * `itemActiveBg` is always accent at 10%; the active row is marked by
 * background + type weight (no border-left). Terminal palettes: black =
 * terminal bg lightened one step, brightBlack = dim, bright* = the base colour
 * at +10–15% lightness with the hue kept. ok/warn/err stay semantic (red
 * errors stay red in every theme).
 * ------------------------------------------------------------------------- */

// Bundled via @fontsource-variable (imported in main.ts); the plain names
// stay as fallbacks for environments where the webfonts fail to load.
const fontUi = '"Geist Variable", "Geist", "SF Pro Text", system-ui, sans-serif'
const fontMono = '"JetBrains Mono Variable", "JetBrains Mono", "Fira Code", ui-monospace, monospace'

/** Accent at 10% over transparent — shared recipe for the active-item fill. */
const activeMix = (accent: string) => `color-mix(in srgb, ${accent} 10%, transparent)`

// ── 1 · Parchment — warm light, soft ────────────────────────────────────────
export const parchment: PylonTheme = {
  name: 'parchment',
  fontUi,
  fontMono,

  appBg: '#f7f4ed',
  bodyBg: '#f7f4ed',
  sidebarBg: '#efebdf',
  titlebarBg: '#efebdf',
  statusbarBg: '#efebdf',
  tabBarBg: '#f7f4ed',

  tabActiveBg: '#efebdf',

  accent: '#564ca0',
  accent2: '#b3793a',
  onAccent: '#faf8f3',
  itemHoverBg: 'rgba(82,68,40,0.06)',
  itemActiveBg: activeMix('#564ca0'),
  itemActiveBorder: '#564ca0',

  textPrimary: '#2c2924',
  textMuted: '#6c6354',
  textDim: '#887d69',

  border: 'rgba(82,68,40,0.16)',
  radius: '7px',

  ok: '#3b885b',
  warn: '#a77924',
  err: '#b13a3a',

  terminal: {
    bg: '#20212b',
    fg: '#d6d8e0',
    cursor: '#9a91e8',
    dim: '#70748e',
    ok: '#5fb784',
    warn: '#d8a85e',
    err: '#e07474',
    black: '#2a2b37',
    red: '#e07474',
    green: '#5fb784',
    yellow: '#d8a85e',
    blue: '#5b8fc9',
    magenta: '#9a91e8',
    cyan: '#6fbdbc',
    white: '#eceef4',
    brightBlack: '#70748e',
    brightRed: '#e98c8c',
    brightGreen: '#74c697',
    brightYellow: '#e2b878',
    brightBlue: '#74a3d8',
    brightMagenta: '#b1aaf0',
    brightCyan: '#8acfce',
    brightWhite: '#ffffff',
  },

  glows: false,
  windowShadow: '0 24px 70px rgba(0,0,0,.15)',
}

// ── 2 · Porcelain — cool light, crisp blue accent ──────────────────────────
export const porcelain: PylonTheme = {
  name: 'porcelain',
  fontUi,
  fontMono,

  appBg: '#f3f6fa',
  bodyBg: '#f3f6fa',
  sidebarBg: '#e2e8f1',
  titlebarBg: '#dde4ee',
  statusbarBg: '#dde4ee',
  tabBarBg: '#ebeff5',

  tabActiveBg: '#fbfcfe',

  accent: '#2563d9',
  accent2: '#d4602f',
  onAccent: '#f7f9fb',
  itemHoverBg: 'rgba(40,60,90,0.07)',
  itemActiveBg: activeMix('#2563d9'),
  itemActiveBorder: '#2563d9',

  textPrimary: '#1b2028',
  textMuted: '#4a5563',
  textDim: '#677282',

  border: 'rgba(40,60,90,0.22)',
  radius: '7px',

  ok: '#23885a',
  warn: '#b5770f',
  err: '#c43a3a',

  terminal: {
    bg: '#1f242e',
    fg: '#d6dce4',
    cursor: '#8aa6e8',
    dim: '#6c7891',
    ok: '#5fb784',
    warn: '#d8a85e',
    err: '#e07474',
    black: '#29303c',
    red: '#e07474',
    green: '#5fb784',
    yellow: '#d8a85e',
    blue: '#82a8d8',
    magenta: '#b094cf',
    cyan: '#6fbdbc',
    white: '#eef1f6',
    brightBlack: '#6c7891',
    brightRed: '#e98c8c',
    brightGreen: '#74c697',
    brightYellow: '#e2b878',
    brightBlue: '#9abce4',
    brightMagenta: '#c2a8dd',
    brightCyan: '#8acfce',
    brightWhite: '#ffffff',
  },

  glows: false,
  windowShadow: '0 24px 70px rgba(0,0,0,.15)',
}

// ── 3 · Midnight — high-contrast dark, electric blue ───────────────────────
export const midnight: PylonTheme = {
  name: 'midnight',
  fontUi,
  fontMono,

  appBg: '#0b0f15',
  bodyBg: '#0b0f15',
  sidebarBg: '#131a23',
  titlebarBg: '#131a23',
  statusbarBg: '#131a23',
  tabBarBg: '#0f141c',

  tabActiveBg: '#1b2430',

  accent: '#4aa3ff',
  accent2: '#ffb347',
  onAccent: '#06111f',
  itemHoverBg: 'rgba(190,215,255,0.07)',
  itemActiveBg: activeMix('#4aa3ff'),
  itemActiveBorder: '#4aa3ff',

  textPrimary: '#f2f5f9',
  textMuted: '#a9b5c3',
  textDim: '#7b8796',

  border: 'rgba(190,215,255,0.18)',
  radius: '7px',

  ok: '#5fb784',
  warn: '#d8b16a',
  err: '#e07474',

  terminal: {
    bg: '#080b10',
    fg: '#d3dce6',
    cursor: '#8ab8e8',
    dim: '#5c6d81',
    ok: '#66bb8a',
    warn: '#d8b16a',
    err: '#e07474',
    black: '#1a1f27',
    red: '#e07474',
    green: '#66bb8a',
    yellow: '#d8b16a',
    blue: '#829cd8',
    magenta: '#b094cf',
    cyan: '#6fd8d8',
    white: '#eef3f8',
    brightBlack: '#5c6d81',
    brightRed: '#e98c8c',
    brightGreen: '#7ecca0',
    brightYellow: '#e2c184',
    brightBlue: '#9ab0e4',
    brightMagenta: '#c2a8dd',
    brightCyan: '#88e4e4',
    brightWhite: '#ffffff',
  },

  glows: false,
  windowShadow: '0 24px 70px rgba(0,0,0,.5)',
}

// ── 4 · Aurora — deep indigo with teal and pink ────────────────────────────
export const aurora: PylonTheme = {
  name: 'aurora',
  fontUi,
  fontMono,

  appBg: '#17122b',
  bodyBg: '#17122b',
  sidebarBg: '#211a3c',
  titlebarBg: '#211a3c',
  statusbarBg: '#211a3c',
  tabBarBg: '#1b1533',

  tabActiveBg: '#2c2350',

  accent: '#2fe0c0',
  accent2: '#ff5fa8',
  onAccent: '#0c1f1b',
  itemHoverBg: 'rgba(205,185,255,0.08)',
  itemActiveBg: activeMix('#2fe0c0'),
  itemActiveBorder: '#2fe0c0',

  textPrimary: '#f1ecff',
  textMuted: '#b6acd9',
  textDim: '#8a7fb5',

  border: 'rgba(205,185,255,0.2)',
  radius: '7px',

  ok: '#6fbf8d',
  warn: '#d4a85a',
  err: '#e07b8a',

  terminal: {
    bg: '#100c1f',
    fg: '#dcd8e8',
    cursor: '#b3a7f7',
    dim: '#6d638f',
    ok: '#6fbf8d',
    warn: '#d8b16a',
    err: '#e07b8a',
    black: '#1b1825',
    red: '#e07b8a',
    green: '#6fbf8d',
    yellow: '#d8b16a',
    blue: '#8a9de8',
    magenta: '#c9a3e8',
    cyan: '#7fd0ce',
    white: '#f0edf8',
    brightBlack: '#6d638f',
    brightRed: '#ea93a0',
    brightGreen: '#86cda1',
    brightYellow: '#e2c184',
    brightBlue: '#a2b2ef',
    brightMagenta: '#d7b8f0',
    brightCyan: '#96dcda',
    brightWhite: '#ffffff',
  },

  glows: false,
  windowShadow: '0 24px 70px rgba(0,0,0,.5)',
}

// ── 5 · Ember — warm dark, vivid orange ────────────────────────────────────
export const ember: PylonTheme = {
  name: 'ember',
  fontUi,
  fontMono,

  appBg: '#1a1410',
  bodyBg: '#1a1410',
  sidebarBg: '#271e17',
  titlebarBg: '#271e17',
  statusbarBg: '#271e17',
  tabBarBg: '#1f1813',

  tabActiveBg: '#33281f',

  accent: '#ff8a3d',
  accent2: '#ffd166',
  onAccent: '#1f1206',
  itemHoverBg: 'rgba(255,205,160,0.07)',
  itemActiveBg: activeMix('#ff8a3d'),
  itemActiveBorder: '#ff8a3d',

  textPrimary: '#f7ede2',
  textMuted: '#c4af9a',
  textDim: '#917d69',

  border: 'rgba(255,205,160,0.18)',
  radius: '7px',

  ok: '#6fbf8d',
  warn: '#d8b16a',
  err: '#e07b6d',

  terminal: {
    bg: '#120d09',
    fg: '#ddd5c4',
    cursor: '#cf8a46',
    dim: '#746a58',
    ok: '#7dbb82',
    warn: '#d8a85e',
    err: '#e07b6d',
    black: '#1f1b15',
    red: '#e07b6d',
    green: '#7dbb82',
    yellow: '#d8b75e',
    blue: '#7a9ab8',
    magenta: '#c490a8',
    cyan: '#8fb8b7',
    white: '#f2ecdf',
    brightBlack: '#746a58',
    brightRed: '#ea9285',
    brightGreen: '#93cc97',
    brightYellow: '#e2c678',
    brightBlue: '#92aec8',
    brightMagenta: '#d3a6bb',
    brightCyan: '#a5c8c7',
    brightWhite: '#fbf7ee',
  },

  glows: false,
  windowShadow: '0 24px 70px rgba(0,0,0,.5)',
}

export const themes: Record<string, PylonTheme> = {
  parchment,
  porcelain,
  midnight,
  aurora,
  ember,
}

/** Themes renamed in 0.29.0 → their replacement, so a saved choice keeps a
 *  similar look instead of falling back to the default. */
export const themeAliases: Record<string, string> = {
  fjord: 'midnight',
  nocturne: 'aurora',
  oxide: 'ember',
}

/** Display labels for menus / palette / about, keyed like `themes`. */
export const themeLabels: Record<string, string> = {
  parchment: 'Parchment',
  porcelain: 'Porcelain',
  midnight: 'Midnight',
  aurora: 'Aurora',
  ember: 'Ember',
}

/** 'dark' or 'light' for a `#rrggbb` background, by its relative luminance
 *  (WCAG). Sets the root `color-scheme`, which native controls follow: without
 *  it a dark theme gets light-mode `<select>` popups that can render blank. */
export function colorSchemeFor(bg: string): 'dark' | 'light' {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(bg.trim())
  if (!m) return 'light'
  const lin = (h: string) => {
    const c = parseInt(h, 16) / 255
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  const l = 0.2126 * lin(m[1]!) + 0.7152 * lin(m[2]!) + 0.0722 * lin(m[3]!)
  return l < 0.18 ? 'dark' : 'light'
}
