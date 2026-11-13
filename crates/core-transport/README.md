# core-transport

The `Transport` trait and per-protocol connection implementations for ZapTerm.

## Purpose

Owns the boundary between session management and the underlying network/serial protocols.
Protocol-specific code (SSH handshake details, Telnet option negotiation, baud rate configuration)
must not leak past this crate's public API.

## Public API

- `Transport` — async trait: `connect`, `disconnect`, `resize`.
- `error::Error` — typed error enum; all variants are `#[non_exhaustive]`.

Protocol implementations are added per-block:
- `local_pty` — Bloque 1 (Local PTY via `portable-pty`)
- `ssh` — Bloque 2 (SSH2 via `russh`)
- `telnet` / `serial` — Bloque 4

## Non-goals

- Does not own terminal emulation (that is `core-terminal`).
- Does not own session lifecycle state (that is `core-session`).
- Does not handle credentials (that is `core-vault`).
