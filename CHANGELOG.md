# Changelog

All notable changes to ZapTerm will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

ZapTerm continues ZAPX, which is frozen at 0.42.1; its history is in the
[ZAPX changelog](https://github.com/CarlosTejeiro/zapx/blob/main/CHANGELOG.md).

## [1.5.2] - 2026-11-14

### Fixed
- **Importing from MobaXterm**, checked against the published description
  of its session format:
  - files with accents or € in names (MobaXterm writes Windows-1252, not
    UTF-8) no longer fail the whole import;
  - sessions with "Display reconnection message" unchecked are no longer
    lost;
  - `<default>` is no longer imported as the user name;
  - the session type decides the protocol (an RDP session with the SSH icon
    is skipped, not imported as SSH);
  - SSH gateways become jump hosts (several hops chain), and a private key
    is used when the file exists on this computer.
- **Importing from SecureCRT**, checked against VanDyke's own scripts:
  - serial sessions are imported (the port is `Com Port`, not
    `Serial Port`);
  - the port is the protocol's own (`[SSH2] Port`, `[SSH1] Port` or
    `Port`), not whichever key ends in "Port";
  - `Firewall Name = Session:…` becomes the jump host;
  - files with a BOM and multi-line values are read correctly;
  - sessions without a usable key use password (prompted on connect)
    instead of the agent.
- PuTTY `.reg` exports in UTF-16 (as Windows can save them) are read.
- Import warnings are in English, like the rest of the app.

## [1.5.0] - 2026-11-14

### Removed
- **Snippets.** The Snippets dialog (Tools menu, command palette,
  `Ctrl+Shift+S`), the `Ctrl+Shift+1..9` quick-fire keys, the snippets
  ZapTerm created by itself for each platform and the snippet suggestions in
  the hints popup are gone: the command lists and ★ Custom do that job.
  **Macros are not affected** (sidebar, shortcuts, run on connect, export).

### Changed
- **Your snippets move to ★ Custom**, once, the first time 1.5.0 starts
  (only the ones you wrote, not the ones ZapTerm created). A one-line
  snippet becomes a command; one without a trailing Enter keeps waiting for
  you to finish it; one with several lines or `{{variables}}` becomes a
  recipe whose form asks for the variables. If `custom.json` has an error,
  nothing is overwritten and the move waits for the next start. The old
  rows stay in the database, so backups and sync with older versions keep
  working.

## [1.4.0] - 2026-11-14

### Added
- **Richer highlight rules out of the box.** Eleven new built-in rules:
  usage percentages in four colours (green below 70 %, yellow 70–89 %, red
  90–97 %, bold red 98–100 %), sizes (`15Gi`, `4.0K`, `1.2 MB`, `10Gbps`,
  `1000Mb/s`), dates, times, URLs, Unix paths and status words (bad:
  errors, failed, denied, timeout, unreachable…; warning: deprecated,
  disabled, retry…; good: ok, established, connected, running, full…).
  `df -h`, `ping`, `show interfaces` and logs now come out coloured. They
  are adapted from two open-source colourisers, grc (GPL-2.0-or-later) and
  ChromaTerm (MIT). Existing installs get them after their own rules,
  never twice, and like every rule they can be edited, disabled or deleted.

### Fixed
- **Local tabs are highlighted too.** Highlight rules only applied to SSH,
  Telnet and serial sessions; the local shell (and saved local/WSL
  sessions) showed plain output.
- The IPv6 rule coloured times such as `12:05:11`. It now needs `::` or all
  eight groups (only where you hadn't edited the rule).

## [1.3.0] - 2026-11-13

### Added
- **macOS command lists.** Local tabs on a Mac (zsh, the default shell, or
  bash) now get their own lists instead of the Linux ones: System (host,
  memory, disks, logins), Processes (ps, top, launchd jobs), Network
  (interfaces, ARP/NDP, DNS resolvers, listening ports), Routing, Diagnostics
  (ping, traceroute, reachability, DNS-SD, curl), Power (pmset, caffeinate)
  and Capture (tcpdump, including Apple's all-interface capture with packet
  metadata). 60 commands, each checked against the man page in Apple's
  open-source releases. Like every list it is a JSON file you can copy and
  edit in Settings → Command Lists.

## [1.2.0] - 2026-11-13

### Added
- **★ Custom: your own commands on every platform.** A new button next to
  Recent in the command bar opens a list that is the same for every session,
  whatever the platform (FortiGate, Cisco, Linux, local shell…). Add a command
  with "+ Add command", edit it (✎) or remove it (✕, asks first); click runs
  it, Type writes it without Enter, and `<name>` parts are left for you to
  fill in, like the built-in lists. Hover a Recent entry and press ☆ to keep
  it in Custom. It is saved as `custom.json` in the command lists folder and
  updates never change it. If you edit that file by hand and it has a
  mistake, ZapTerm shows the error and does not overwrite it.

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
