/// <reference types="svelte" />
/// <reference types="vite/client" />

// Side-effect CSS imports — Vite handles these at build time, TS needs
// a stub declaration so `import '@xterm/xterm/css/xterm.css'` typechecks.
declare module '*.css'
