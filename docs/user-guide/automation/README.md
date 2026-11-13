# Automating network devices with ZapTerm

A step-by-step guide for people who have never used ZapTerm. By the end you will
have saved a device, run a ready-made Python script against it (first as a dry
run, then for real) and explored a device from a notebook.

| Part | What you'll do | Time |
|---|---|---|
| [1. Getting started](1-getting-started.md) | Install, save your first device, store its password in the vault, log in automatically. | 10 min |
| [2. Python scripts](2-scripts.md) | What a script is, run `hello.py`, then `sase_reset.py` step by step — dry run, read-only, then for real. | 15 min |
| [3. Notebooks](3-notebooks.md) | Connect VS Code or JupyterLab to ZapTerm and explore a device cell by cell. | 15 min |

## The idea in one picture

```text
 your script / notebook                        ZapTerm                       devices
 ┌──────────────────────┐   "run get system  ┌──────────────────┐  types  ┌──────────┐
 │ import zapterm          │ ─── status on ───▶ │ a real terminal  │ ──────▶ │ FortiGate│
 │ out = fw.run(...)    │     FGT-60F-01"    │ tab you can see  │         │  switch  │
 │ if "down" in out: …  │ ◀── the output ─── │                  │ ◀────── │  router  │
 └──────────────────────┘                    └──────────────────┘         └──────────┘
```

- **The script decides, ZapTerm does.** Your Python code never opens a connection
  itself: ZapTerm types every command in a normal tab, so you watch it happen and
  can take over at any moment.
- **Connections are ZapTerm's job.** Jump hosts, passwords from the vault, login
  banners and host keys are handled exactly as when you open the session by
  hand.
- **Passwords never reach your code.** A script can ask ZapTerm to type a vault
  password into a session, but it can't read it.

## Before you start

- **ZapTerm 0.26.0 or newer** (notebooks need 0.26.0; scripts 0.25.0).
- **Python 3.9 or newer** on the same computer, for parts 2 and 3:
  - Linux: usually already there — check with `python3 --version`.
  - Windows: install it from [python.org](https://www.python.org/downloads/)
    and tick **"Add python.exe to PATH"** in the installer.
  - macOS: install it from [python.org](https://www.python.org/downloads/).
- A device to try it on. The screenshots use two FortiGates called
  `FGT-60F-01` and `FGT-60F-02` in a folder called `SASE`; any device you can
  reach works for parts 1 and 3.

> All names, addresses and passwords in this guide are made up. IP addresses
> are from the ranges reserved for documentation (`192.0.2.0/24`,
> `203.0.113.0/24`, RFC 5737) or `127.0.0.1`; the screenshots come from a demo
> profile talking to simulated FortiGates.

## Reference

- [Python scripting reference](../scripting.md) — every `zapterm` call, how
  `run()` detects the end of a command, notebook details, security.
- [User guide](../README.md) — every ZapTerm feature (sessions, vault, macros,
  SFTP, splits, …).
