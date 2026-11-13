# Changelog

All notable changes to ZapTerm will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

ZapTerm continues ZAPX, which is frozen at 0.42.1; its history is in the
[ZAPX changelog](https://github.com/CarlosTejeiro/zapx/blob/main/CHANGELOG.md).

## [1.0.0] - 2026-11-13

### Changed
- **ZAPX is now ZapTerm.** Same features as ZAPX 0.42.1, under the new name
  and app identifier (`app.zapterm.client`), so ZapTerm keeps its own data
  folder and can be installed next to ZAPX.

### Added
- **ZAPX data is imported on the first start**: when ZapTerm has no data yet
  and ZAPX's data folder is found (its default folder, or the one set in
  ZAPX's "Data folder" setting), ZapTerm copies ZAPX's sessions database,
  `vault.key` and catalog overrides. ZAPX's files are only read. Session logs
  stay in ZAPX's folder.
- **Saved passwords come along**: a credential ZapTerm doesn't find in its own
  OS keyring entry is read from ZAPX's and copied over (ZAPX's entry stays, so
  ZAPX keeps working).
- **ZAPX scripts keep working**: `import zapx` is an alias of the new
  `zapterm` scripting module.

### Compatibility
- Backups (`.zapxb`), session and macro exports and highlight-rule files keep
  their formats, so files move freely between ZAPX and ZapTerm.
- Environment variables are now `ZAPTERM_*` (e.g. `ZAPTERM_DATA_DIR`), and the
  folder-sync file is `zapterm-sync.zapxb`, so ZapTerm and ZAPX don't
  overwrite each other in a shared sync folder.
