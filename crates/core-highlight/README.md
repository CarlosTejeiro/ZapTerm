# core-highlight

Keyword highlighting engine for ZapTerm.

## Purpose

Evaluates regex-based highlight rules against the terminal cell grid after VT processing
(ADR-006). Manages rule priority ordering, global vs. per-session scope, and efficient
application over large outputs.

## Public API

_(Populated in Bloque 5)_

## Non-goals

- Does not parse VT escape sequences (that is `core-terminal`).
- Does not own the rule storage/CRUD (that is `core-persistence` + `core-config`).
