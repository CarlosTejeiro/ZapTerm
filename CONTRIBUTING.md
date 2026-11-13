# Contributing to ZapTerm

> **ZapTerm is free software, licensed under the GNU GPL v3 or later.**
> Contributions are welcome. By submitting a pull request you agree that your
> contribution is licensed under the same GPL-3.0-or-later terms as the rest
> of the project. This document is the development guide.

## Development setup

1. **Rust** ≥ 1.89 via [rustup](https://rustup.rs). Components: `rustfmt`, `clippy`.
2. **Node.js** LTS + **pnpm** (`corepack enable && corepack prepare pnpm@latest --activate`).
3. **MSVC Build Tools** on Windows (required by `rusqlite` and `portable-pty`).
4. Clone the repo and run:

```sh
cargo xtask dev
```

This starts the Vite dev server and the Tauri app together.

## Project structure

The codebase is a Cargo workspace:

- `crates/app/` — the Tauri binary (commands, events, app state).
- `crates/core-*/` — domain crates: transport (SSH/Telnet/Serial/PTY), persistence, vault,
  session, terminal, logging, highlight, config, hints.
- `frontend/` — Svelte 5 + Vite + Tailwind UI, with a thin bridge layer over the Tauri commands.
- `xtask/` — Cargo-based automation (`cargo xtask dev`, `cargo xtask build`).
- `assets/` — fonts, themes, and highlight rules bundled into the app.

## Coding standards

- Format with `rustfmt` and keep `cargo clippy` clean (no warnings) before opening a PR.
- Prefer making illegal states unrepresentable; use `thiserror` for library error types and
  newtype IDs over bare primitives.
- Never log secrets; zeroize sensitive material. Avoid `unsafe` except for unavoidable FFI.

## Commit conventions

Conventional Commits are mandatory (`feat:`, `fix:`, `docs:`, `refactor:`, `perf:`, `test:`,
`chore:`, `build:`, `ci:`). Commit messages explain *why*, not *what* — the diff shows the what.

## Pull request process

1. Self-review your diff before opening the PR.
2. Fill in the PR template (motivation, approach, test plan, follow-ups).
3. CI must be green on all three platforms before merge.
4. Keep PRs small (~400 lines diff soft cap).

## Reporting issues

Use the GitHub issue templates. One issue per topic. Out-of-scope feature ideas go to
[docs/backlog.md](docs/backlog.md), not to the issue tracker.
