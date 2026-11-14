# ZapTerm

> Modern terminal for network engineers — SSH · Telnet · Serial in one app.

**Website:** <https://carlostejeiro.github.io/ZapTerm/>

**Free software (GNU GPL v3 or later).**
**Status: alpha — functional but not yet hardened for production.**

ZapTerm is a fast, modern, multi-protocol terminal client for network engineers —
a lightweight open-source alternative to SecureCRT, MobaXterm or PuTTY. It runs
on Windows, macOS and Linux, keeps your sessions and credentials organized, and
automates the repetitive parts of working on network gear.

**Coming from ZAPX?** ZapTerm is its continuation: ZAPX is frozen at 0.42.1
and new work happens here. On its first start, ZapTerm imports ZAPX's
sessions, settings and saved credentials (ZAPX itself is left as it was);
ZAPX backups (`.zapxb`), exports and scripts (`import zapx`) keep working.

---

## Table of contents

- [What ZapTerm does](#what-zapterm-does)
  - [Connect to anything](#connect-to-anything)
  - [Organize your sessions](#organize-your-sessions)
  - [Work faster in the terminal](#work-faster-in-the-terminal)
  - [Automate logins and tasks](#automate-logins-and-tasks)
  - [Keep credentials safe](#keep-credentials-safe)
  - [Transfer files](#transfer-files)
  - [Make it yours](#make-it-yours)
  - [Take it anywhere](#take-it-anywhere)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Installation](#installation)
- [Building from source](#building-from-source)
- [Architecture](#architecture)
- [Security](#security)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

---

## What ZapTerm does

### Connect to anything

One app for every way you reach a device:

- **SSH2** — password, public key, SSH agent, and 2FA keyboard-interactive.
- **Telnet** and **Serial** (COM / TTY) for console and legacy gear.
- **Local shell** tabs, right alongside your remote sessions.
- **Jump hosts & tunnels** — ProxyJump through saved sessions, plus `-L` local,
  `-D` SOCKS5 dynamic and `-R` remote port forwarding.
- **Rock-solid links** — configurable SSH keepalive, auto-reconnect on drop with
  a one-click banner, and reconnect / clear / save-output right in the toolbar.

### Organize your sessions

Never hunt for a host again:

- **Session manager** with a folder tree, drag-and-drop, and instant search.
- **Tabs** you can rename, colour and duplicate; a **resizable sidebar**.
- **Broadcast groups** — open N hosts at once as a grid with a single master
  input bar and type to all of them together.
- **Splits & grid view** — recursive split panes and output compare, so you can
  watch several devices side by side.
- **Command palette** (`Ctrl+K`) to fuzzy-launch any session, group, action or
  theme without touching the mouse.
- **Window memory** — remembers size, position and maximized state; sizes to
  your monitor on first launch.

### Work faster in the terminal

The little things that add up across a long day:

- **Command hints** — autocomplete from your own history plus vendor catalogs
  for **12 platforms**, with automatic platform detection from the prompt.
- **Snippets** — per-platform command snippets fired on `Ctrl+Shift+1..9`.
- **Command lists per vendor** — grouped commands and recipes under the
  terminal for the detected platform (local tabs get the Windows, macOS or
  Linux lists for their shell). They are JSON files: make your own copy
  in **Settings → Command Lists** and edit it; updates never change your copy.
- **★ Custom commands** — your own command list, the same on every platform,
  next to Recent in the command bar (add, edit, remove, or ☆ a Recent entry).
- **Keyword highlighting** — per-rule regex in true-color ANSI, on every
  terminal (local, SSH, Telnet, serial). Built-in rules colour IPs, MACs,
  usage percentages by level, sizes, dates, times, URLs, paths and good/bad
  status words; edit them or add your own, and export/import them as JSON.
- **Triggers** — per-session regex/literal matches on output that notify you,
  auto-send a reply, or ring the bell.
- **In-terminal search** (`Ctrl+F`) with match count, highlight-all and
  case / word / regex options.
- **Session logging** — plain or raw capture with 50 MB rotation and a
  per-session history panel.

### Automate logins and tasks

- **Login automation** — expect/send login scripts per saved session, with
  regex expects.
- **Macros** — a sidebar library of expect / send / wait macros you run on the
  focused session: folders, drag-to-reorder, edit-as-JSON, run-on-connect, and
  JSON export/import (plus MobaXterm import).

### Keep credentials safe

- **Credential vault** — reusable named credentials stored in the OS keyring
  (with an AES-256-GCM database fallback). Attach one to a session, or reference
  it from a macro as `{{vault:Name.Password}}`.
- Secrets are resolved **in the backend** at run time — they never enter the UI,
  the JSON, exports or logs.
- Passwords **typed at a prompt** are never recorded to history or recents.

### Transfer files

- **SFTP file browser** with streaming up/downloads, live progress and
  cancellation.
- **Edit remote files** with your local editor over SFTP.

### Make it yours

- **5 full themes** — Parchment and Porcelain (light), Midnight, Aurora and Ember (dark) — each
  restyling both the UI chrome and the terminal ANSI palette, with bundled
  Geist / JetBrains Mono / Cascadia Code fonts.
- **Customizable shortcuts** for every action (**Settings → Shortcuts**).

### Take it anywhere

- **Encrypted backup** — one `.zapxb` file with *everything*, credentials
  included, sealed under a passphrase (Argon2id + AES-256-GCM). Drop it in any
  cloud-synced folder and restore it on another device (**Settings → Backup**).
- **Sync folder** — point ZapTerm at a folder your cloud client already syncs and
  every install stays up to date: local changes are published automatically,
  other devices' changes are offered for merging, conflicts are yours to
  settle. No provider account, no server: the provider only sees ciphertext.
- **Export / import** — one JSON file with your sessions, folders, groups,
  snippets and highlight rules (never passwords, never the vault). Import is
  idempotent, so it's safe for migration and team sharing.
- **Import from what you already use** — `~/.ssh/config`, PuTTY, MobaXterm and
  SecureCRT.
- **Portable mode** — a Windows portable `.exe` and a user-selectable data
  folder let your whole setup travel with the binary.

## Keyboard shortcuts

All customizable in **Settings → Shortcuts**. Defaults:

| Shortcut | Action |
|---|---|
| `Ctrl+K` | Command palette |
| `Ctrl+N` | New saved session |
| `Ctrl+Shift+N` | Quick Connect (ad-hoc, nothing saved) |
| `Ctrl+T` | New local shell tab |
| `Ctrl+W` | Close current tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+Shift+H` / `Ctrl+Shift+K` | Split pane horizontally / vertically |
| `Ctrl+Shift+M` | Toggle multi-exec broadcast |
| `Ctrl+Shift+S` | Snippets |
| `Ctrl+Shift+1..9` | Fire snippet 1..9 |
| `Ctrl+F` | In-terminal search — match count, highlight-all, case/word/regex (`Enter` / `Shift+Enter` to step) |
| `Ctrl+,` | Settings |
| `Escape` | Close search / close dialog |

## Installation

Download the installer for your platform from the
[releases page](https://github.com/CarlosTejeiro/zapterm/releases).

### macOS: "ZapTerm is damaged and can't be opened"

The macOS bundles are **not yet signed/notarized** (an Apple Developer ID is
on the roadmap), so Gatekeeper quarantines the downloaded app and reports it
as damaged. The app is fine — clear the quarantine flag after dragging it to
Applications:

```sh
xattr -cr /Applications/ZapTerm.app
```

or right-click the app → **Open** → **Open** on first launch.

### Windows portable

`ZapTerm_x64_portable.exe` runs from any folder without installation (it needs
the WebView2 runtime, preinstalled on Windows 11 and updated Windows 10).
To make it fully self-contained — sessions, settings, snippet catalogs and
logs travelling with the exe — create an empty file named `portable` next to
it; everything is then kept in a `data/` folder beside the executable. In
portable mode credentials are stored AES-256-GCM-encrypted in the database
instead of the OS keyring, under a random key kept in `vault.key` (mode 0600)
beside the database. Because both files live in the same `data/` folder,
anyone with access to that folder can use the credentials — treat the folder
like a physical key.

### Custom data folder

Where ZapTerm keeps its data (`zapterm.db`, `session_logs/`, `catalogs/`) can be
changed in **Settings → Data** (migrates your data and restarts), or forced
with the `ZAPTERM_DATA_DIR` environment variable / `--data-dir <path>` CLI flag.

## Building from source

Prerequisites: Rust stable ≥ 1.89, Node.js LTS, pnpm.

```sh
# Install frontend dependencies (first time only)
cd frontend && pnpm install

# Start the Tauri dev server (compiles Rust + starts Vite)
cargo tauri dev
```

### Produce a release binary / installer

```sh
cargo tauri build
```

The MSI installer (Windows) is written to `target/release/bundle/msi/`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development setup.

## Architecture

```
zapterm/
├── crates/
│   ├── app/               # Tauri shell — commands, state, IPC bridge, data-dir resolver
│   ├── core-transport/    # SSH, Telnet, Serial, local PTY, SFTP, port forwarding
│   ├── core-persistence/  # SQLite via rusqlite — sessions, rules, logs, settings
│   ├── core-vault/        # OS keyring + AES-256-GCM portable fallback
│   ├── core-highlight/    # Regex keyword highlighting, ANSI injection
│   ├── core-hints/        # Command suggestions: history, vendor catalogs, snippets
│   ├── core-terminal/     # VT100/xterm escape-sequence parser
│   ├── core-logging/      # Session log writer with 50 MB rotation
│   ├── core-session/      # Session lifecycle types
│   └── core-config/       # Config helpers
├── frontend/              # Svelte 5 + xterm.js + Tailwind CSS 4
└── xtask/                 # Developer automation (cargo xtask dev, etc.)
```

## Security

- Passwords are stored in the **OS keyring** (Windows Credential Manager, macOS Keychain,
  libsecret on Linux) — never in SQLite in cleartext and never in logs. When the keyring
  can't return a secret (e.g. unsigned builds) or in portable mode, an **AES-256-GCM**
  copy encrypted in the database is used as a fallback.
- **Vault** secrets and session passwords are resolved in the backend at run time and
  written straight to the session; a macro or export only ever holds the `{{vault:…}}`
  placeholder — never the secret.
- **Passwords typed at a prompt are not recorded** to command history, recents or hints.
- Host key verification with SHA-256 fingerprints and known_hosts.
- Session logs capture raw terminal bytes only; credentials are not echoed by the PTY.
- `#![forbid(unsafe_code)]` in all core crates.

## Documentation

- **[User Guide](docs/user-guide/)** — full walkthrough of sessions, the credential
  vault, macros, SFTP, splits, settings and more.
- Per-version notes live in [docs/releases/](docs/releases/) and the full history in
  [CHANGELOG.md](CHANGELOG.md).

## Roadmap

- [ ] Code signing: Apple Developer ID (macOS notarization, fixes the "damaged" prompt) + Windows certificate
- [ ] App updates: private update server (Windows/Linux self-update; macOS gated on signing)
- [ ] Importers: SecureCRT port forwards (PuTTY + MobaXterm already supported)
- [x] Linux packaging: from-source Arch `PKGBUILD` (`packaging/arch/`) — AUR/Flathub/Snap pending public distribution
- [x] Regex matching in login-script expects (0.8)
- [x] Edit remote files with a local editor over SFTP (0.8) — OS drag & drop upload deferred

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
development setup and coding standards. By submitting a pull request you agree
that your contribution is licensed under the project's GPL-3.0-or-later terms.

## License

ZapTerm is free software: you can redistribute it and/or modify it under the terms
of the **GNU General Public License** as published by the Free Software
Foundation, either **version 3** of the License, or (at your option) any later
version. See [LICENSE](LICENSE) for the full text.

ZapTerm is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License for more details.

Third-party open-source components are under their own permissive licenses; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
