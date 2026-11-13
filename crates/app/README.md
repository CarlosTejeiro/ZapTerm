# app

The Tauri 2 application binary for ZapTerm. Hosts the frontend webview and the Rust command
handlers that bridge the UI to the core crates.

## Structure

- `src/lib.rs` — Tauri builder, command registration, app startup
- `src/commands/` — `#[tauri::command]` handlers grouped by domain
- `src/events.rs` — event emitters pushing data from Rust to the frontend
- `src/state.rs` — shared `AppState` managed by Tauri's state system
- `tauri.conf.json` — Tauri 2 configuration
- `capabilities/` — Tauri 2 permission declarations

## Dev loop

```sh
cargo xtask dev
```

Or manually:

```sh
cd frontend && pnpm dev &
cargo tauri dev --manifest-path crates/app/Cargo.toml
```
