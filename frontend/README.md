# ZapTerm frontend

Svelte 5 + Vite 6 + Tailwind CSS 4 frontend for ZapTerm.

## Dev loop

```sh
pnpm install        # install dependencies (first time)
pnpm dev            # start Vite dev server on :1420
pnpm type-check     # svelte-check + TypeScript
pnpm lint           # ESLint
pnpm test           # Vitest unit tests
pnpm build          # production build → dist/
```

## Stack

- **Svelte 5** — runes mode enforced globally (`compilerOptions.runes: true`)
- **Vite 6** — dev server on port 1420 (required by Tauri)
- **Tailwind CSS 4** — configured via `@tailwindcss/vite` plugin; no postcss config needed
- **TypeScript** — strict mode + `noUncheckedIndexedAccess`

## Bridge layer

`src/lib/bridge/` contains typed wrappers around `@tauri-apps/api`:

- `types.ts` — shared type definitions (SessionId, Folder, Session, ...)
- `commands.ts` — `invoke()` wrappers for Rust commands
- `events.ts` — `listen()` wrappers for Rust events

These stubs are expanded in Bloque 1 when the real Rust commands are implemented.
