# Handoff: Rediseño de ZAPX — Opción A «Pulido» + icono Tile + 7 temas

## Overview

Rediseño de la ventana principal de **ZAPX** (cliente de terminal multi-protocolo SSH/Telnet/Serial,
repo `CarlosTejeiro/zapx`, Tauri + Svelte 5 + xterm.js + Tailwind CSS 4).

Decisiones tomadas con el usuario:

1. **Dirección elegida: Opción A «Pulido»** — misma estructura de layout que la app actual,
   ejecución refinada (iconografía SVG unificada, ritmo de espaciado base-8, terminal como
   tarjeta flotante, tabs y status bar más limpias). Bajo riesgo: solo CSS/markup, sin tocar
   lógica ni estado.
2. **Icono de marca elegido: «Tile»** — rayo único sobre tesela redondeada rellena de acento.
   Sustituye al actual de dos rayos cruzados.
3. **7 temas** que sustituyen/amplían los actuales: Parchment (refinado), Oxide, Fjord,
   Nocturne, Porcelain, Phosphor y Amber.

Las opciones B y C del primer lienzo quedaron descartadas, pero se incluyen como referencia
de ideas futuras (rail de iconos, bloques de comando, composer).

## About the Design Files

Los ficheros de este paquete son **referencias de diseño hechas en HTML/JSX (React)** — son
prototipos visuales, **no código para copiar tal cual**. La tarea es **recrear este diseño en
el frontend Svelte 5 existente** del repo, siguiendo sus patrones actuales:

- Temas como objetos `PylonTheme` en `frontend/src/lib/themes/index.ts`, threaded por props.
- Componentes en `frontend/src/lib/pylon/` (`TitleBar.svelte`, `TabBar.svelte`,
  `Sidebar.svelte`, `StatusBar.svelte`, `Pane.svelte`).
- xterm.js recibe la paleta de terminal desde `theme.terminal`.

Para ver los diseños: abrir `ZAPX Marca y Temas.html` (icono + 7 temas sobre la Opción A)
y `ZAPX Rediseño.html` (las 3 direcciones originales A/B/C) en un navegador con conexión
(usa React/Babel por CDN). La fuente de verdad del layout de la Opción A es
`zapx-option-a.jsx`; la de los temas, `zapx-themes.jsx`; iconos y marca, `zapx-shared.jsx`.

## Fidelity

**High-fidelity (hifi).** Colores, tipografía, espaciados y tamaños son finales y deben
recrearse con precisión de píxel. Los valores exactos están en este README y en los `.jsx`.

## Pantalla: Ventana principal (Opción A)

Estructura general (1440×900 de referencia, todo fluido):

```
┌─ TitleBar (38px) ──────────────────────────────────────────────┐
├─ Sidebar (248px) ─┬─ TabBar (42px) ────────────────────────────┤
│                   ├─ Zona terminal (padding 14px)              │
│                   │   └─ Tarjeta terminal (radius 11, sombra)  │
│                   ├─ StatusBar (28px)                          │
└───────────────────┴────────────────────────────────────────────┘
```

Tokens usados abajo: ver tabla «Design Tokens». `radius` base = 7px.

### TitleBar (`TitleBar.svelte`)

- Altura **38px**, fondo `paper2`, borde inferior 1px `line`.
- Izquierda (padding 0 14px, gap 8, nowrap): icono Tile 15px · wordmark `ZAPX`
  (12px, weight 700, letter-spacing 1.5px, color `ink`) · separador `/` (`inkDim`) ·
  nombre de sesión activa (12px, `inkMuted`).
- Centro: región de arrastre (flex 1).
- Menús (File View Session Tools Help): 12px, padding 4px 10px, radius 5;
  reposo `inkMuted`, hover/abierto: fondo `hover` + color `ink`.
- Controles de ventana: **SVG stroke 1.5px** (minimizar = línea, maximizar = rect
  redondeado, cerrar = X), 44px de ancho cada uno, color `inkDim`;
  hover cerrar: fondo `#e81123`, icono blanco (mantener comportamiento actual).

### Sidebar (`Sidebar.svelte`)

- Ancho **248px**, fondo `paper2`, borde derecho 1px `line`.
- **Búsqueda** (padding contenedor 12px 12px 8px): caja 30px de alto, fondo `paper`,
  borde 1px `line`, radius `radius`, icono lupa SVG 13px, placeholder 12.5px `inkDim`,
  kbd `⌘K` a la derecha (10.5px mono, borde 1px `line`, radius 4, padding 1px 5px).
- **Cabecera de sección** `SESSIONS`: 10.5px, weight 600, letter-spacing 1px, `inkDim`,
  con chevron SVG (rota 90° al expandir, transición .12s), contador, y a la derecha
  iconos SVG carpeta + más (sustituyen a los emojis 📁 y al «+» de texto).
- **Fila de sesión**: alto 32px, padding 0 8px, radius `radius`, gap 9.
  Punto de color 7px (colores `SESSION_COLORS` existentes) · nombre 13px
  (activa: `ink` weight 500; reposo: `inkMuted` 400) · tag de protocolo (9.5px mono,
  `inkDim`, borde 1px `line`, radius 4, padding 1px 5px — solo si no es `local`/SSH activo).
  - **Activa**: fondo `color-mix(in srgb, accent 10%, transparent)`.
    ⚠️ Eliminar el `border-left` de 2px actual (desplaza el contenido); el fondo + peso
    tipográfico ya marcan el estado.
  - **Hover**: aparecen iconos lápiz y X (SVG 12px, `inkDim`) en lugar de ✎ ✕.
- **Carpetas**: fila 28px con chevron + icono carpeta SVG + nombre en mayúsculas
  (11px, weight 600, ls 0.8px) + contador. Hijas indentadas (padding-left 26px), 30px.
- **Footer** (borde superior 1px `line`, padding 10px 14px): chip de usuario 24px
  (radius 7, fondo `accent`, iniciales 10px weight 700 color `paper`) + nombre 12px
  `inkMuted` + iconos llave y engranaje SVG 14px `inkDim`.

### TabBar (`TabBar.svelte`)

- Altura **42px**, fondo `paper` (zona de contenido), borde inferior 1px `line`,
  padding 0 10px, gap 4. Tabs **centradas verticalmente** (no pegadas abajo).
- **Tab**: alto 30px, padding 0 12px, radius `radius`, min-width 140px, gap 8.
  Punto de estado 7px (conectada `ok`, conectando `warn`, error `err`, cerrada `inkDim`).
  Label 12.5px.
  - **Activa**: fondo `paper2`, borde 1px `line`, subrayado interior
    `box-shadow: inset 0 -2px 0 accent`, label `ink` weight 500, X de cierre visible
    (SVG 11px). Sin trucos de borde superpuesto ni margin-bottom negativo.
  - **Reposo**: transparente, label `inkMuted`; X solo en hover.
- Botón «+»: 28×28, radius `radius`, icono SVG 14px `inkDim`.
- Derecha: botones **Split** y **Multi** — alto 28px, padding 0 10px, borde 1px `line`,
  radius `radius`, icono SVG 13px + label 11.5px weight 500 `inkMuted`.
  Activo: color/fondo de acento como en la app actual.

### Zona de terminal (`Pane.svelte` / `SplitTree.svelte`)

- La zona tiene fondo `paper` y **padding 14px**.
- El xterm vive dentro de una **tarjeta**: fondo `terminal.bg`, radius **11px**
  (`radius + 4`), `overflow: hidden`, padding interno 14px 18px,
  sombra `0 1px 2px rgba(45,35,15,0.10), 0 8px 28px rgba(45,35,15,0.10)`.
- Tipografía terminal: `fontMono` 13px, line-height 1.6.
- Scrollbar: track 4px, `rgba(255,255,255,0.04)`; thumb `rgba(255,255,255,0.14)`, radius 2.
- En split, cada panel repite la tarjeta con gap 10px entre ellas.

### StatusBar (`StatusBar.svelte`)

- Altura **28px** (antes 24), fondo `paper2`, borde superior 1px `line`,
  `fontMono` 11px, `font-variant-numeric: tabular-nums`, padding 0 12px.
- Segmentos con gap interno 5px y padding 0 7px; separador `│` con opacidad 0.25.
- Izquierda: LED 6px + `CONNECTED` en color `ok` (LED con glow suave al estar conectado) ·
  `admin@10.0.12.1` (`inkMuted`) · `ssh 22` · `mss 1448↑ / 1460↓` · `up 2:14:09` · `12 ms`.
- Derecha: `utf-8 │ VT320 │ 1×1` en `inkDim`.

## Marca: icono «Tile»

Sustituir el SVG actual (dos rayos cruzados) en `TitleBar.svelte` y en los assets de la app
(icono de ventana/instalador) por:

```svg
<svg viewBox="0 0 16 16" fill="none">
  <rect x="0.5" y="0.5" width="15" height="15" rx="4.2" fill="ACCENT" />
  <path d="M 9.1 2.6 L 5 8.6 H 7.6 L 6.6 13.4 L 11.4 6.8 H 8.6 Z" fill="PAPER" />
</svg>
```

- `ACCENT` = `theme.accent`; `PAPER` = `theme.appBg` (el rayo se recorta en el color de fondo).
- En titlebar se usa a 15–16px; escala bien a 32/44px para icono de app.
- Generar también los assets del bundle Tauri (ico/icns/png) a partir de este SVG.

## Interacciones y comportamiento

Sin cambios funcionales: se conservan todos los handlers, atajos y drag & drop actuales.
Detalles visuales:

- Transiciones hover/active: `background .1s, color .1s` (como ahora).
- Chevrons de carpeta/sección: `transform .12s`.
- LED de conexión: pulso de opacidad 2s (reutilizar `pylon-glow-pulse`); respetar
  `prefers-reduced-motion` (ya implementado en `app.css`).
- El indicador de drop en drag & drop del sidebar pasa a usar `accent` con radius 2.

## State Management

Sin estado nuevo. Único cambio de datos: añadir los 7 temas a
`themes: Record<string, PylonTheme>` y a los menús/ThemeSelector.

## Design Tokens

### Comunes

| Token | Valor |
|---|---|
| `fontUi` | `"Geist", "SF Pro Text", system-ui, sans-serif` |
| `fontMono` | `"JetBrains Mono", "Fira Code", ui-monospace, monospace` |
| `radius` | `7px` (tarjeta terminal: 11px; kbd/tags: 4px; chips: 5px) |
| Espaciado | base-8 ya existente (`--zx-space-*`) |
| Sombra tarjeta terminal | `0 1px 2px rgba(45,35,15,0.10), 0 8px 28px rgba(45,35,15,0.10)` |

### Mapeo a `PylonTheme`

Cada tema define abajo: `paper` (→ `appBg`, `bodyBg`, `tabBarBg`), `paper2` (→ `sidebarBg`,
`titlebarBg`, `statusbarBg`, `tabActiveBg`), `ink` (→ `textPrimary`), `inkMuted`
(→ `textMuted`), `inkDim` (→ `textDim`), `line` (→ `border`, `tabBorder`), `hover`
(→ `itemHoverBg`), `accent`/`accent2`, `ok`/`warn`/`err` y la paleta `terminal`.
`itemActiveBg` = `color-mix(in srgb, accent 10%, transparent)` · `itemActiveBorder` = accent
(ya no se pinta como border-left) · `onAccent` = `paper` en temas oscuros / blanco roto en claros.

### Los 7 temas

#### 1 · Parchment — claro cálido (refinado del actual)
`paper #f7f4ed` · `paper2 #efebdf` · `ink #2c2924` · `inkMuted #6c6354` · `inkDim #a09684`
· `line rgba(82,68,40,0.16)` · `hover rgba(82,68,40,0.06)` · `accent #564ca0` ·
`accent2 #b3793a` · `ok #3e8f60` · `warn #b88528` · `err #b13a3a`
Terminal: `bg #20212b` · `deep #1a1b24` · `fg #d6d8e0` · `dim #5c5f74` · `cursor #9a91e8` ·
`cyan #6fbdbc` · `green #5fb784` · `red #e07474` · `yellow #d8a85e` · `blue #5b8fc9` ·
`magenta #9a91e8` · `white #eceef4`

#### 2 · Oxide — oscuro cálido, cobre
`paper #1d1a16` · `paper2 #262119` · `ink #eae3d4` · `inkMuted #a2967f` · `inkDim #6f6452`
· `line rgba(255,230,180,0.11)` · `hover rgba(255,230,180,0.05)` · `accent #cf8a46` ·
`accent2 #7aa893` · `ok #6fbf8d` · `warn #d8b16a` · `err #e07b6d`
Terminal: `bg #15120e` · `deep #100d0a` · `fg #ddd5c4` · `dim #5d5546` · `cursor #cf8a46` ·
`cyan #8fb8a4` · `green #7dbb82` · `red #e07b6d` · `yellow #d8a85e` · `blue #7a9ab8` ·
`magenta #c490a8` · `white #f2ecdf`

#### 3 · Fjord — oscuro frío, hielo
`paper #161a21` · `paper2 #1d232c` · `ink #dee5ee` · `inkMuted #91a0b2` · `inkDim #5a6878`
· `line rgba(180,210,255,0.11)` · `hover rgba(180,210,255,0.05)` · `accent #5ca3d6` ·
`accent2 #d68a6e` · `ok #5fb784` · `warn #d8b16a` · `err #e07474`
Terminal: `bg #11141a` · `deep #0c0f14` · `fg #d3dce6` · `dim #4d5b6c` · `cursor #8ab8e8` ·
`cyan #6fc3d8` · `green #66bb8a` · `red #e07474` · `yellow #d8b16a` · `blue #82a8d8` ·
`magenta #b094cf` · `white #eef3f8`

#### 4 · Nocturne — oscuro violeta, periwinkle
`paper #181520` · `paper2 #201c2c` · `ink #e6e2f0` · `inkMuted #9b93b4` · `inkDim #655d80`
· `line rgba(215,200,255,0.11)` · `hover rgba(215,200,255,0.05)` · `accent #9d92ea` ·
`accent2 #5eb3b2` · `ok #6fbf8d` · `warn #d4a85a` · `err #e07b8a`
Terminal: `bg #121019` · `deep #0d0b13` · `fg #dcd8e8` · `dim #4f4868` · `cursor #b3a7f7` ·
`cyan #7fd0ce` · `green #6fbf8d` · `red #e07b8a` · `yellow #d8b16a` · `blue #8a9de8` ·
`magenta #c9a3e8` · `white #f0edf8`

#### 5 · Porcelain — claro frío, índigo acero
`paper #f3f5f7` · `paper2 #e8ecf0` · `ink #252a31` · `inkMuted #5e6873` · `inkDim #98a2ad`
· `line rgba(40,60,90,0.15)` · `hover rgba(40,60,90,0.05)` · `accent #4a66a0` ·
`accent2 #b06a4f` · `ok #3e8f60` · `warn #b88528` · `err #b13a3a`
Terminal: `bg #1f242e` · `deep #191d26` · `fg #d6dce4` · `dim #566074` · `cursor #8aa6e8` ·
`cyan #6fbdbc` · `green #5fb784` · `red #e07474` · `yellow #d8a85e` · `blue #82a8d8` ·
`magenta #b094cf` · `white #eef1f6`

#### 6 · Phosphor — verde sobre negro (CRT clásico)
`paper #0c100c` · `paper2 #121812` · `ink #cfe8cf` · `inkMuted #7da87d` · `inkDim #4a6b4a`
· `line rgba(80,255,120,0.13)` · `hover rgba(80,255,120,0.05)` · `accent #36d97c` ·
`accent2 #9dd87a` · `ok #3ddc84` · `warn #d8b16a` · `err #e07474`
Terminal: `bg #050905` · `deep #030603` · `fg #8ae89a` · `dim #2f5a3a` · `cursor #57e389` ·
`cyan #6fd8b0` · `green #57e389` · `red #e07474` · `yellow #d8c16a` · `blue #6fb8a8` ·
`magenta #8fd8a8` · `white #c9f5d0` · `glows: true` (glow del cursor en verde)

#### 7 · Amber — ámbar sobre negro (CRT clásico)
`paper #120e07` · `paper2 #1a140b` · `ink #f0dcb8` · `inkMuted #ad9468` · `inkDim #715f42`
· `line rgba(255,200,100,0.13)` · `hover rgba(255,200,100,0.05)` · `accent #e8a33c` ·
`accent2 #c97c4a` · `ok #8fb96a` · `warn #f0c060` · `err #e07b5e`
Terminal: `bg #0a0703` · `deep #070502` · `fg #ecb45c` · `dim #5c4a2c` · `cursor #ffb84d` ·
`cyan #f0cd8a` · `green #8fb96a` · `red #e07b5e` · `yellow #f0c060` · `blue #c9a85e` ·
`magenta #d89a6a` · `white #f8e2b8` · `glows: true` (glow del cursor en ámbar)

Para los slots ANSI restantes (`black`, `bright*`): `black` = `terminal.bg` aclarado un paso;
`brightBlack` = `dim`; el resto = versión +10–15% de luminosidad del color base, manteniendo
el matiz (usar oklch). Mantener semántica de `ok/warn/err` para el keyword highlighting de
Cisco/IOS incluso en Phosphor/Amber (el rojo de error debe seguir siendo rojo).

## Sistema de iconos

Crear `frontend/src/lib/icons/Icon.svelte` (o set de componentes) con SVG stroke **1.5px**,
`stroke-linecap/linejoin: round`, viewBox `0 0 16 16`, tamaños 11–16px. Paths exactos en
`zapx-shared.jsx` (componentes `Icon*`): search, plus, folder, chevron, pencil, x, split,
cast/multi, gear, terminal, book, clock, bolt, key, min, max.
**Eliminar todos los glifos de texto/emoji**: ✎ ✕ 📁 ▸ ● ─ □ en `Sidebar.svelte`,
`TitleBar.svelte`, `TabBar.svelte` y diálogos.

## Assets

- Sin imágenes externas. Fuentes: Geist y JetBrains Mono (Google Fonts / bundle local).
- Icono Tile: SVG inline (arriba) — generar ico/icns/png para Tauri.

## Screenshots

En `screenshots/` — referencia visual de cómo debe quedar cada vista:

| Fichero | Vista |
|---|---|
| `01-marca-tile.png` | Propuestas de icono (el elegido es **2 · Tile**) |
| `02-tema-parchment.png` … `08-tema-amber.png` | Ventana principal (Opción A) en cada uno de los 7 temas |

## Files

| Fichero | Contenido |
|---|---|
| `ZAPX Marca y Temas.html` | Lienzo: icono elegido + 7 temas sobre la Opción A |
| `ZAPX Rediseño.html` | Lienzo original: opciones A/B/C + notas |
| `zapx-option-a.jsx` | **Fuente de verdad del layout** de la Opción A (medidas exactas) |
| `zapx-themes.jsx` | Definición de los 7 temas (variables CSS) |
| `zapx-shared.jsx` | Iconos SVG (paths exactos), marca Tile, paleta terminal |
| `zapx-option-b.jsx` / `zapx-option-c.jsx` | Direcciones descartadas (referencia futura) |
| `design-canvas.jsx` / `tweaks-panel.jsx` | Infraestructura de los lienzos (ignorar) |
