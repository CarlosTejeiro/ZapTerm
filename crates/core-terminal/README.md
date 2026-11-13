# core-terminal

Terminal emulator state (VT/ANSI sequence processing + cell grid) for ZapTerm.

## Purpose

Wraps `wezterm-term` to give the rest of the application a stable, protocol-agnostic view of the
terminal screen. Consumers get a rendered cell grid; they do not deal with raw escape sequences.

## Public API

_(Populated in Bloque 1)_

## Non-goals

- Does not own the byte stream from the network (that is `core-transport`).
- Does not render pixels or HTML — it owns the logical grid, not the visual output.
- Does not apply keyword highlighting (that is `core-highlight`).
