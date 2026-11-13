# Backlog

Out-of-scope ideas tracked per ADR-007. Features listed here are not implemented until the active
block that gates them is closed.

Add entries with format: `- **Feature name**: brief description.`

## Distribution & updates (gated on code signing)

- **Code signing**: Apple Developer ID for macOS (notarization — fixes the
  "ZapTerm is damaged" Gatekeeper prompt) and a Windows certificate. This is the
  blocker for both clean distribution and macOS auto-update. Until then, macOS
  users clear quarantine with `xattr -cr /Applications/ZapTerm.app`.
- **App updates**: the repo/releases are private, so GitHub-hosted auto-update
  isn't viable (no anonymous manifest, can't ship a token). Options when
  wanted: a **private update server** (`latest.json` + signed bundles you host;
  Windows/Linux self-update now, macOS after signing), or keep distribution
  manual. "Check for updates" currently just opens the releases page.

## Features

- **Importers: SecureCRT port forwards** (planned for 0.10): PuTTY
  (`PortForwardings`) and MobaXterm (`[PortForwarding]`) tunnel import are done.
  Session import already works (`importers/securecrt.rs` walks `Sessions/` and
  parses the typed `S:/D:/B:` `.ini` keys: protocol, host, port, user, identity,
  serial). What's missing is reading the **forward** keys into `session_forwards`
  — the rest of the pipeline (`session_forwards` → `apply_import`) is in place
  from PuTTY/MobaXterm.
  - Blocker: SecureCRT's forward storage format isn't publicly documented. The
    VanDyke "import PuTTY into SecureCRT" script only mentions it copies
    "tunnels (port forward settings)" without specifying the keys/encoding.
  - Need: a real session `.ini` that has forwards configured (ideally a Local
    `-L` and a Dynamic `-D`). Locations: Windows
    `%APPDATA%\VanDyke\Config\Sessions\<name>.ini`; macOS
    `~/Library/Application Support/VanDyke/SecureCRT/Config/Sessions/<name>.ini`;
    Linux `~/.vandyke/SecureCRT/Config/Sessions/<name>.ini`. Drop it in `temp/`
    (gitignored). Passwords are encrypted in the file — fine to share.
  - Then: reverse-engineer the forward keys, add parsing to `decode_session`,
    and add a test against the sample (mirror `imports_port_forwardings` in
    `importers/mobaxterm.rs`).
- **MobaXterm jump-host import**: a MobaXterm bookmark with "connect through
  SSH gateway" encodes the gateway inline (the `%2%host%port%user%` fields);
  could map to ZapTerm `via_session_id` (ProxyJump) on import. Sample available.
- **Regex in login-script expects**: the schema already carries `is_regex`
  (`crates/app/src/login_script.rs`); wire it so expect patterns can be regex,
  not just literal.
- **SFTP drag & drop upload (blocked)**: dropping OS files to upload needs
  Tauri's native drag-drop (`dragDropEnabled: true`) to get file paths, but
  that disables the HTML5 drag-drop the sidebar (session reorder) and button
  bar (reorder) rely on — and breaks them on Windows. Not worth trading two
  working features for it; the Upload button covers uploads. ("Edit remote
  file" shipped in 0.8.)

## Linux packaging

- **AUR / Flathub / Snap**: a from-source Arch `PKGBUILD` ships in
  `packaging/arch/` (clones the private repo over SSH at the tag — for own use,
  not the AUR). Still pending for *public* distribution: a `zapterm-bin` PKGBUILD
  on the AUR that repacks the released .deb, Flathub (manifest + review), and
  Snapcraft — all blocked by the private releases repo (source URLs need a
  reachable download location).

<!-- Add out-of-scope ideas below this line -->
