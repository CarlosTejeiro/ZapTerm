# core-persistence

SQLite storage, at-rest encryption, and schema migrations for ZapTerm.

## Purpose

Provides the database layer: bundled SQLite via `rusqlite`, numbered migrations, and optional
at-rest encryption of the database file. Credentials are never stored here — only opaque keyring
references (see `core-vault`).

## Public API

_(Populated in Bloque 3)_

## Non-goals

- Does not store plaintext passwords or key material. Ever.
- Does not own session files (logs live on disk managed by `core-logging`).
