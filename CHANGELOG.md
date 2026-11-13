# Changelog

All notable changes to ZapTerm will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

ZapTerm continues ZAPX, which is frozen at 0.42.1; its history is in the
[ZAPX changelog](https://github.com/CarlosTejeiro/zapx/blob/main/CHANGELOG.md).

## [1.1.0] - 2026-11-13

### Added
- **Command lists you can edit.** The lists under the terminal are now JSON
  files (one per platform, with their metadata: label, lists, icons,
  colours, sections, commands and recipes). Settings → Command Lists →
  "Make my copy" writes your own copy to `<data folder>/cmdlists/`; edit it
  with any text editor and press "Reload". **Your copy is used instead of the
  built-in list and updates never change it**; "Use built-in" deletes it to
  go back. A file with an error is ignored (the built-in list is used) and
  Settings says exactly what is wrong. The folder has a README with the
  format.

### Changed
- Juniper Debug and Linux Capture lists: removed the "Ctrl+C" entries (just
  press Ctrl+C).

### Fixed
- A list with two identical commands or section titles no longer fails to
  open.

## [1.0.3] - 2026-11-13

### Fixed
- **Highlight rules work on serial consoles.** Output was highlighted piece by
  piece as it arrived, and a serial console delivers a few bytes at a time, so
  an IP address split across two reads was never matched and stayed white.
  ZapTerm now highlights whole lines; an unfinished line (a prompt, the keys
  you type) is shown after 25 ms if nothing else arrives.

### Changed
- FortiGate Debug list: removed the "Ctrl+C" entry (just press Ctrl+C).

## [1.0.2] - 2026-11-13

### Fixed
- **The Linux .deb installs next to ZAPX.** The executable was still called
  `app` (the internal crate name), so ZapTerm's package tried to overwrite
  ZAPX's files with the same names (`app.png` icons…) and `apt` refused to
  install it. The executable is now `zapterm`.

## [1.0.1] - 2026-11-13

### Fixed
- **No connection to an external font service on start.** The app's page
  still pointed at fonts.bunny.net (a preconnect plus a stylesheet the app's
  content security policy already blocked). The fonts were always bundled with
  the app, so nothing changes visually; ZapTerm just no longer reaches out.

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
