# core-logging

Per-session log writers with rotation and search for ZapTerm.

## Purpose

Writes session output to timestamped files in the OS app-data directory. Handles rotation by
size and date, and provides a simple search interface over the log buffer. Distinct from the
app's own diagnostic logging (that uses `tracing`).

## Public API

_(Populated in Bloque 6)_

## Non-goals

- Does not own app-level diagnostic logs (those go through `tracing`).
- Does not store logs inside the database — files on disk only.
