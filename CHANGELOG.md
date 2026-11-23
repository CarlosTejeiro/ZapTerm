# Changelog

All notable changes to ZapTerm will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

ZapTerm continues ZAPX, which is frozen at 0.42.1; its history is in the
[ZAPX changelog](https://github.com/CarlosTejeiro/zapx/blob/main/CHANGELOG.md).

## [1.8.10-beta] - beta of the automation work (branch `claude/automation`, not on main yet)

Betas are the 1.8.x versions: 1.8.0, 1.8.1… tagged `v1.8.0-beta`, `v1.8.1-beta`…
(1.6.0 was the first beta, before the encrypted vault reached main as 1.7.)
The app version itself carries no suffix, because the MSI and RPM bundlers
disagree on which suffixes they accept.

Includes everything up to 1.7.18 (the sections below), plus:

### Changed
- **Run & compare shows the differences side by side.** The two outputs face
  each other, each with its own line numbers; removed and added lines are
  paired across, and identical runs stay folded. **Split / Unified** next to
  the legend switches back to the single-column view; Split is the default.
- **"Page through --More--" also works with Enter in the SEND TO ALL bar**
  (1.8.8 only paged during Run & compare, so a line sent with Enter still
  stopped at `--More--`). After the line goes out, each session that stops at
  a pager prompt gets a space, until it has been quiet for 10 s, or after 200
  pages. What you type straight into a pane is left alone.

### Added
- **Run & compare pages through `--More--` for you.** A device that stopped at
  a pager prompt (`--More--`, `---(more)---`, `<--- More --->`) left that pane
  waiting and its output was compared cut short. Now Run & compare presses
  Space in that session only, while it is capturing, until the prompt comes
  back (up to 200 pages; the time limit starts again with each page). The pager
  prompts and the way devices erase them are left out of the comparison. On by
  default; **Page through --More--** in the SEND TO ALL bar turns it off.

### Security
- **`cookie` updated to 0.18.3** for
  [RUSTSEC-2026-0594](https://rustsec.org/advisories/RUSTSEC-2026-0594), as
  in 1.7.17 on main.

### Changed
- **Run & compare says what it found.** The result was a modal with "2 hosts
  identical" and a "diff" button even when there was nothing to compare. Now:
  - It is a panel under the terminals, and each pane's header shows its result:
    **=** same output everywhere, **A** / **B**… which version the pane printed
    (A is the most common, or on a tie the left-most pane's), **…** waiting,
    **–** no reply, **!** possibly incomplete.
  - The status reads "Same output on both panes", "Output differs: 2
    versions"… and the panel says what is compared (the text between the
    command and the prompt, spaces at line ends ignored).
  - With two versions the differences show straight away, only the changed
    lines and a few around them; with more, each version can be compared
    with A in one click. No more picking two groups.
  - Panes with the same name are told apart ("FWF-40F (pane 2)"), and panes
    connected to the same device (same host and port) are pointed out: the
    same output is then expected.
  - It warns when an output stopped at a `--More--` prompt (turn paging off
    and run again; the pane is still waiting there) and when no prompt came
    back within 10 s.
  - A device that pauses mid-output is no longer cut short: if more output
    arrives after a quiet second, the capture carries on.

- **Multi is per tab and needs two or more sessions.** It was one global
  switch: it stayed on when you moved to another tab, so typing there could
  reach several devices unnoticed, and in a tab with a single session it did
  nothing at all. Now each tab has its own Multi; with fewer than two
  sessions the button is greyed out and explains how to get there (split the
  tab or open a group), and a tab left with one session turns it off. While
  it is on, the button reads "Multi · N", the tab shows a broadcast icon and
  every pane in it has a coloured border.
- **The "MASTER" bar is now "SEND TO ALL"**, and its tooltip says how many
  sessions of the tab (or group) receive what is typed there.
- **"live" in the bar is now "Send keys as I type"**, with a tooltip
  saying what it does: off (the default), nothing is sent until Enter; on,
  every key goes to all the sessions as you type it, for what a device
  answers before Enter (such as `?` for context help on Cisco IOS). The box's
  placeholder says which mode is on.

### Fixed
- **Run & compare with "live" on sent the command twice.** Live mode had
  already typed it on every device, and Run & compare typed it again
  (`show versionshow version`). It now only presses Enter.
- **"live" ignored Backspace.** Characters deleted in the MASTER bar stayed
  on the devices' command line; they are now erased there too.

### Added
- **Add devices in bulk** (File menu and command palette). Paste one device
  per line (`host`, `host:port`, `name host`, `name host port`) or open a CSV
  file whose first line names the columns (name, host, port, protocol,
  username, folder, tags, notes, credential, jump_host — commas, semicolons
  or tabs). Defaults fill what the list leaves out; folders are created from
  a `DC/Madrid` path; the credential is a vault entry and the jump host a
  saved SSH session, both by name, so no password is ever in the list. A
  preview marks every row New, Exists or Error; devices already in the
  inventory are skipped and nothing is added while any row has an error.
- **MCP server: connect an AI assistant.** `zapterm_mcp.py` (plain Python, no
  packages) lets an MCP client list tabs and saved sessions, open a session
  and run commands through ZapTerm, over the notebook connection. ZapTerm
  enforces the rules: Read-only refuses anything but display commands and now
  tells the client it was not sent; no tool exposes or types passwords. The
  notebook panel shows the configuration to copy.
- **Ask me first.** With the notebook connection's Read-only off, ZapTerm
  shows each command that isn't a display command and waits for your Yes
  (for notebooks too; on by default, can be switched off in the panel).
  Refused, declined and approved commands are logged in the panel.
- `Session.run(..., strict=True)` raises instead of returning an empty string
  when read-only keeps a command from being sent.
## [1.7.18] - 2026-11-23

### Added
- **Drag a session from the sidebar onto an empty split pane** to open it
  there. Until now an empty pane was filled only by clicking a session while
  it was focused. The pane shows a dashed outline while a session is over it;
  the session stays where it is in the sidebar.

## [1.7.17] - 2026-11-23

### Security
- **`cookie` updated to 0.18.3**, for
  [RUSTSEC-2026-0594](https://rustsec.org/advisories/RUSTSEC-2026-0594)
  ("private cookie nonces can repeat in processes that fork"). ZapTerm gets
  `cookie` through Tauri; nothing in ZapTerm's own code changes.

## [1.7.16] - 2026-11-22

### Added
- **Help → Check for updates tells you what's new and installs it.** It used
  to open the releases page, where a beta could be listed first. Now it asks
  GitHub for the newest release *for your copy* — stable versions for a
  stable copy; a beta copy is also offered newer betas — and shows it with
  its release notes. "Download and install" fetches the installer that
  matches how ZapTerm was installed (setup EXE, MSI, portable EXE, DMG, DEB,
  RPM or AppImage, for your CPU) into your Downloads folder and checks it
  against the SHA-256 GitHub publishes for it; a file that doesn't match is
  deleted. The setup EXE and the MSI then install with one click (ZapTerm
  closes so the installer can replace it); a DMG, DEB or RPM opens in its
  installer; a portable EXE or AppImage is shown in its folder to replace
  the old one. A copy from the Microsoft Store is pointed to the Store.
- Nothing is checked in the background: GitHub is only contacted when you
  open the dialog (the privacy policy and the Q&A say so).

## [1.7.15] - 2026-11-22

### Security
- **`url` updated to 2.5.9** (and `form_urlencoded` to 1.2.3), for
  [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592): a
  stack overflow in `Url::origin()` on nested `blob:` URLs. ZapTerm gets
  `url` through Tauri; nothing in ZapTerm's own code changes.

## [1.7.14] - 2026-11-21

### Changed
- **A highlight rule's pattern can be edited.** Editing a rule used to lock the
  pattern and the "Regular expression" box; changing what a rule matches meant
  deleting it and adding another. Both are now editable. A pattern that
  another rule already has is refused. If you sync between computers, the
  other ones see a changed pattern as a new rule and keep the old one too —
  delete it there.

### Fixed
- **Built-in rules that start with `(?i)` could not be saved after editing.**
  The form checked the pattern with the browser's regex syntax, which is not
  the one the terminal uses and rejects `(?i)`: "Status: good", "Status:
  warning" and "Status: bad" showed "Invalid regex" and would not save even a
  color change. The form now asks the same engine that colors the terminal,
  so the check and the "Try it on sample output" preview match what you will
  see.
- A new rule with an invalid pattern is refused when saving, also when it
  comes from outside the form.

## [1.7.13] - 2026-11-21

### Changed
- **"enable" and "disable" are highlighted.** The built-in highlight rules
  coloured `enabled` and `disabled` but not the short forms FortiOS and others
  print — `Private Encryption: Disable`, `FIPS-CC mode: disable`,
  `set status enable`. Now `enable` / `Enable` is green like `enabled`, and
  `disable` / `Disable` yellow like `disabled`. The two rules ("Status: good"
  and "Status: warning") are updated only if you haven't edited them.
- The database moves to schema 29 for this; older ZapTerm versions cannot
  open a data folder once this one has.

## [1.7.12] - 2026-11-21

### Changed
- **A session that uses a vault entry logs in as the entry's user.** Until
  now choosing a vault entry only copied its username into the session;
  changing the username in the vault later left the sessions on the old one.
  Now the vault decides: change the user there and every session that uses
  the entry follows. An entry without a username (a password shared by
  several users) still leaves the user to each session.
  **If a session uses a vault entry but had a different username typed in,
  it now connects with the entry's.**
- **The session dialog shows what comes from the vault.** With a vault entry
  chosen, Username is greyed out showing the entry's user, and Password is
  shown locked ("taken from the vault entry") instead of hidden. To give the
  session its own password, choose "— type a password —".

## [1.7.11] - 2026-11-21

### Fixed
- **Editing a vault credential can no longer be dismissed by accident.** With
  the edit form open, a click that landed outside the dialog closed it, and
  Escape threw away what had been typed. Now a click outside does nothing
  while the form is open, and Escape is ignored once something has been
  typed: the form goes away only with Cancel, Save or Close.
- The vault dialog said passwords "stay in the OS keyring"; since 1.7.0 they
  are in the encrypted vault, and the text says so.

## [1.7.10] - 2026-11-19

### Added
- **FortiGate commands by patch, from the release notes.** The *New
  features* and *Changes in CLI* sections of every FortiOS patch (7.2.1 to
  7.2.14, 7.4.1 to 7.4.13, 7.6.1 to 7.6.8) were read for operational
  commands. New in the lists, each shown from the patch that introduced it:
  - `diagnose sys ha checksum show-nonsync` (7.2.1)
  - `diagnose sys sdwan health-check remote <name> <seq_num>` (7.2.4)
  - `diagnose switch-controller telemetry show mac-stats switch <serial_number>` (7.2.4)
  - `diagnose nodejs logs list` (7.2.4)
  - `diagnose sys tcp-auth-options` (7.4.2)

### Changed
- **The SD-WAN rules command changes at 7.4.4, not 7.4.0.** The 7.4.4 release
  notes date the split of `diagnose sys sdwan service` into `service4` and
  `service6`: a device on 7.4.0–7.4.3 now gets the old command.

## [1.7.9] - 2026-11-18

### Security
- **Configuration commands with a secret in them are no longer remembered.**
  The command history behind *Recent* and the typing hints skipped lines
  like `password: …` or `token=…`, but not a secret given as an argument,
  which is how devices take it: `set psksecret …`, `set password …`,
  `enable secret …`, `username admin password …`,
  `neighbor … password …`, `snmp-server community …`, `tacacs-server key …`,
  `key-string …`, Junos `authentication-key …` / `pre-shared-key …`,
  `--password …`, `Authorization: Bearer …`. Those are now left out.
  Commands that only read (`show run | include password`, `get …`,
  `diagnose …`, `grep …`) are still remembered.
- **Secrets already in the history are removed.** On start, ZapTerm deletes
  from the history, and from the learned command sequences, every line the
  new rule would not have stored. The history never left the computer: it is
  not part of backups, sync or exports.

## [1.7.8] - 2026-11-17

### Added
- **Command lists can follow the device's OS release.** ZapTerm now reads the
  version a FortiGate or FortiWiFi states in `get system status`
  (`Version: FortiGate-60F v7.4.3,…`), remembers it for the session, and
  shows the command lists written for that release — switching as soon as
  the command is run, and again after an upgrade. When there are no lists for
  the exact release it uses the newest ones written for an older one.
  Your own files can use it too: see "Lists for another OS release" in the
  command lists README (`"osVersion"`, `"variantOf"`).
- **The FortiGate lists now cover FortiOS 7.2, 7.4 and 7.6.** They were
  checked against Fortinet's 7.4 and 7.2 documentation (administration guide
  and CLI reference 7.4.9 and 7.2.13, and the 7.4.0 troubleshooting cheat
  sheet). 7.4 uses the same commands as 7.6. On a 7.2 device:
  `diagnose sys sdwan service` replaces `service4` / `service6`, the IKE
  debug filters with `dst-addr4` instead of `rem-addr4`, and the commands the
  7.2 documentation doesn't show in full (17 of them, mostly numbered test
  levels) are left out rather than assumed.
- **Commands can depend on the exact release.** Inside a list, a command or
  recipe can state the first or last release that has it (`"since": "7.6.5"`,
  `"until": "7.6.4"`): a device on 7.6.4 sees the old form and one on 7.6.8
  the new one. Before the version is known the newest form is shown, with
  the release it needs in its description.
- **A FortiGate first taken for Linux corrects itself.** That same status
  line proves what the device is: a session whose platform was a guess
  (none, generic or Linux) becomes FortiGate. A platform you chose yourself
  is never changed.

### Changed
- The database gains a column for the version (schema 28). Older ZapTerm
  versions cannot open a data folder once this one has.

## [1.7.7] - 2026-11-17

### Added
- **Junos is detected without its login banner.** The role line Junos
  prints above the prompt (`{master:0}`, `{backup:1}`, `{master}`,
  `{primary:node0}`…) now identifies a Juniper device, so an EX/QFX virtual
  chassis, a dual routing-engine MX or an SRX cluster shows the Junos
  command lists even when the banner has scrolled away. A device that
  prints neither is detected after `show version`, as before.

## [1.7.6] - 2026-11-17

Fixes from a review of the app.

### Security
- **SFTP "Edit" no longer runs the file.** The remote file was opened with
  its default application, which on Windows *runs* a `.bat`, `.vbs`, `.js`
  or `.py` instead of showing it, and the file name went through `cmd`,
  where a `&` in it could start another command. Now nothing goes through a
  shell: macOS opens the default text editor, Windows opens Notepad for
  anything that would run (the default application for the rest), and the
  local copy's name is cleaned of characters a shell acts on.
- The temporary copy of a file being edited is readable only by you
  (Linux and macOS).

### Fixed
- **The unlock screen really locks the keyboard.** With the vault locked,
  shortcuts (Ctrl+K, new tab, macros…) still reached the app behind the
  unlock screen. They no longer do.
- **The unlock screen can be moved, minimised and closed.** It covered the
  title bar, so with the vault locked the window had no controls.
- **Deleting a session removes its typed password.** A password typed in a
  pane and kept for the session stayed in the vault (encrypted) after the
  session was deleted. Same when a session is switched to a vault entry,
  a key or the agent, and when a vault entry is deleted.
- **Opening the vault no longer asks the system keyring once per saved
  session.** It looked there for passwords of older versions under names
  the keyring never held.

## [1.7.5] - 2026-11-17

### Removed
- **Seven platforms that had no command list**: MikroTik, Check Point Gaia,
  Brocade Fabric OS, Palo Alto, F5 BIG-IP, Aruba and HPE Comware. They only
  had prompt detection, command hints and a few example macros, and are gone
  from the *Platform* choice. The platforms left are the ones with a command
  list: Cisco IOS, Cisco IOS XR, Arista EOS, Juniper Junos, FortiGate,
  FortiManager, FortiAnalyzer, Linux, macOS and Windows. A session that had
  one of the removed platforms saved uses the default platform; those
  devices are no longer detected, and are not taken for another platform.

## [1.7.4] - 2026-11-17

### Fixed
- **Restoring a backup or syncing no longer replaces this computer's vault.**
  Since 1.7.0 a backup (and the synced copy) also carried the settings that
  describe the vault of the computer it was made on: its id and its key
  wrapped under the master password. Restoring it on another computer, or
  syncing two computers, overwrote those on the receiving side, so after the
  next start the saved passwords there could no longer be decrypted.
  Restoring on the same computer could bring back an older master password.
  Those settings now stay out of backups and are ignored when a backup made
  by 1.7.0–1.7.3 is restored. The saved passwords themselves always
  travelled, and still travel, inside the backup under its passphrase.

## [1.7.3] - 2026-11-17

### Fixed
- **FortiWiFi and FortiGate are recognised from their prompt.** A device
  that still has its model as hostname (`FWF40F # `, `FGT60F (root) # `) is
  now detected as FortiGate on its own, without waiting for a banner that
  FortiOS doesn't print. "FortiWiFi" in the output also counts.
- **A Fortinet prompt is no longer taken for Linux.** If a word such as
  "kernel" appeared in the output, a prompt like `branch-fw # ` was detected
  as Linux and the Linux command lists were shown. A session already saved
  as Linux keeps that platform: change it to FortiGate in the session's
  settings.

## [1.7.2] - 2026-11-17

### Security
- **Removed the unused update plugin.** In-app updates were never switched on
  (ZapTerm doesn't contact an update server; *Check for updates* opens the
  releases page), but the plugin was still built in and brought a ZIP library
  with two newly published advisories (RUSTSEC-2026-0545, RUSTSEC-2026-0548).
  Removing it removes that library from the app.
- The four earlier exceptions for that ZIP library in the dependency audit
  are gone with it.

## [1.7.1] - 2026-11-16

### Fixed
- **Switching tabs puts the keyboard in the terminal.** After clicking a tab
  (or switching with the keyboard shortcut, or closing the tab you were in),
  you had to click inside the terminal before typing; the tab button kept the
  focus.

## [1.7.0] - 2026-11-16

**Make an encrypted backup before updating.** This version upgrades the
database; an older ZapTerm can no longer open it afterwards.

**On macOS**, the first start after updating may ask for your keychain password
several times — once for each saved password while it is read and removed
from the keychain. Choose *Always Allow*. It doesn't happen again afterwards.

### Added
- **Encrypted vault with a master password.** Every saved password is now
  encrypted (AES-256-GCM) under a random vault key that only your master
  password opens (Argon2id). ZapTerm asks you to create it the first time it
  starts: at least 14 characters, no recovery if you forget it.
- **Remember on this computer** (installed copies): the vault key is kept in
  the OS keyring so ZapTerm opens without asking. The keyring holds that one
  key and no passwords.
- **Settings → Vault**: remember on/off, lock now, change the master password
  (no password is re-encrypted).

### Changed
- **Passwords are no longer stored in the OS keyring** nor in the database
  under `vault.key`. What older versions stored is moved into the vault the
  first time it opens — written, read back, then removed from where it was —
  and `vault.key` is deleted once nothing depends on it.
- **Portable copies** ask for the master password at every start and store
  nothing in the keyring of the computer they run on; what an older portable
  copy left there is removed. A copy of the `data/` folder is useless without
  the master password.
- **New backup passphrases need 14 characters** (was 8). Existing backups and
  a sync folder another device already set up keep their passphrase.

### Security
- Copying a portable `data/` folder no longer gives access to the saved
  passwords.
- The predictable key older versions fell back to when `vault.key` could not
  be created is no longer used to encrypt anything.
- A stored secret is bound to its own name: copied onto another entry, it does
  not decrypt.

## [1.5.9] - 2026-11-15

### Added
- **The ★ Custom list and your command list copies are in the backup and in
  sync.** They are files, and neither the encrypted backup nor sync carried
  them, so moving to another computer lost them. Restoring writes the lists
  this computer doesn't have and keeps any of yours that differ (the notice
  says how many were kept); sync also brings edits made on another device,
  under the same rule as everything else. Backups made before this version
  restore as before. The plain sessions export still leaves them out.

## [1.5.8] - 2026-11-15

### Fixed
- **Export, import, backup and sync say "macros", and count them.** Their
  messages still said "snippets", retired in 1.5.0, and counted leftover
  plain rows with them. The export confirmation now lists what the file
  really holds (sessions with their port forwards and triggers, folders,
  groups, macros, highlight rules) and what it doesn't (passwords, the
  ★ Custom list and your copies of the command lists, which are files).
- The backup description no longer says "everything": the ★ Custom list and
  your command list copies are not part of a backup.
- A macro shortcut can use Ctrl+Shift+1…9 again: it was refused as "already
  fires the first nine bar buttons", which stopped being true in 1.5.0.
- User guide: the bottom bar section describes command lists, ★ Custom and
  Recent instead of snippets, and the shortcut table drops the two snippet
  shortcuts that no longer exist.

## [1.5.7] - 2026-11-14

### Fixed
- **Gaps inside words in a tab opened in the background** ("C isco I O S
  X E"). The terminal now redraws its screen when a hidden tab is shown and
  once the terminal font has loaded, so text drawn with stale letter
  spacing is corrected instead of staying until it scrolls away.

## [1.5.6] - 2026-11-14

### Fixed
- **The last Spanish texts are now in English**, like the rest of the app:
  the "Connected" notice when a session connects, the "Theme · …" entries
  and "No results for …" in the command palette, the notice after opening the
  hint catalogs folder, the messages when moving the data folder (Settings →
  Data), and the error for an export file made by a newer ZapTerm.

## [1.5.5] - 2026-11-14

### Fixed
- **An Arista switch is no longer taken for Linux.** EOS is Linux-based and
  logs in with "Last login: …" before a bare `switch#` prompt; that was
  enough to detect Linux, and the detection never changed afterwards, so the
  tab got the Linux command lists and suggestions. "Last login:" now only
  counts together with a shell-style prompt (`user@host:~$`,
  `[root@host ~]#`); a bare `host#` waits for a real clue such as
  `show version`. A Linux host with a bare `host#` prompt and no distribution
  name in its banner is detected a little later too, on the first output that
  names it.
- **Scripts see the platform saved on the session.** `session.platform` gave
  the live detection priority over the platform already stored for a saved
  session; the stored one now wins.

## [1.5.4] - 2026-11-14

### Added
- **FortiManager and FortiAnalyzer command lists.** FortiManager: System,
  Network, Devices (ADOMs, managed devices, DVM database and locks, FGFM
  tunnel), FortiGuard (FDS servers, scheduled updates, bandwidth), HA and a
  bounded packet capture. FortiAnalyzer: System, Network, Logging (fortilogd
  rates and volumes, device log usage), SQL & reports and the same capture.
  Read-only commands only, each checked against the 7.6.0 CLI reference;
  commands that repair, reset, delete or reboot (such as `diagnose dvm task
  repair`) and `diagnose dvm check-integrity`, which offers to change the
  device database, are left out.
- **FortiAnalyzer detection and suggestions.** A session whose banner or
  output names FortiAnalyzer is detected as FortiAnalyzer (not FortiGate)
  and gets its own command suggestions.

## [1.5.3] - 2026-11-14

### Changed
- **Safer debugs and captures.** Commands that can load a device now show a
  ⚠ badge and ask before sending, with the vendor's own warning: every
  FortiGate real-time debug and the packet sniffer, the Cisco crypto and BGP
  debugs and the Cisco packet capture, and Junos `monitor traffic`. Typing a
  command without Enter doesn't ask (you press Enter yourself).
- **Captures stop by themselves.** The Cisco packet capture now has a time
  limit (60 s by default) and stops at 1000 packets, Junos `monitor traffic`
  at 100 packets, and saving a tcpdump capture to a file (Linux, macOS) at
  10,000 packets, so a forgotten capture can't fill the disk.
- **Suggestions no longer offer commands that can't be undone or that need
  someone on site**: factory reset, formatting a disk or flash, deleting
  all logs, halting or shutting down the system, clearing every BGP session
  or route, `rm -rf`, and the unfiltered IPS debug. Suggested FortiGate
  sniffers now stop after 100 packets. Reboots are still suggested.
- **Debugs stop by themselves too.** Every FortiGate real-time debug recipe
  sets how long it runs (5 minutes by default, 30 at most), and number
  fields such as packet counts or seconds only accept a whole number in
  range (1–1000 packets, 1–300 seconds), so a 0 that would mean "no limit"
  can't be sent.
- Your own command lists can mark commands with `"caution": "why"` to get
  the same warning, and give number fields a `"min"` and `"max"`.

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

## [1.5.1] - 2026-11-14

### Changed
- **More room for the command lists.** ★ Custom and Recent now sit one
  above the other at the right of the bar, so the lists get the rest of
  the width: they stay on one line when they fit and only wrap to a second
  line when they don't.

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
