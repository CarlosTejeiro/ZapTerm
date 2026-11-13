# core-vault

Credential storage for ZapTerm.

## Purpose

- **OS keyring** (`Vault`): Windows Credential Manager / macOS Keychain /
  Linux Secret Service via the `keyring` crate, built with its native
  backends (`apple-native`, `windows-native`, `sync-secret-service`; Linux
  builds need `libdbus-1-dev`).
- **Local fallback** (`encrypt_with_seed` / `decrypt_with_seed`): AES-256-GCM
  under the per-install random `vault.key`. The app keeps such a copy in the
  database only when the keyring can't hold a secret, or on portable
  installs; when the keyring works, a stored copy is moved into it. The
  policy lives in the app's `secrets` module.
- **Sealed backups** (`seal` / `open`): Argon2id + AES-256-GCM for `.zapxb`
  files and the sync bundle.

Secrets are never written to logs or error messages.

## Non-goals

- Does not encrypt the database as a whole — that is `core-persistence`.
