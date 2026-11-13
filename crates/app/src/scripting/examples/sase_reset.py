"""Reset the SASE IPsec tunnels found down on a group of FortiGates.

Edit the settings below, then run it from Tools → Run Python script…:
- Dry run (first time): nothing is sent or opened; the panel lists every
  command it would type.
- Read-only: logs in and reads the real tunnel state (`get` commands only),
  then reports which tunnels a real run would reset — nothing is changed.
- Run: does it for real.

The devices are ZapTerm saved sessions (all those in FOLDER, or the names in
DEVICES). ZapTerm handles the connection like for any saved session: jump host,
password from the vault, login script (e.g. answering a login disclaimer), host
keys. This script only holds the logic.

On each device:
1. `get system status` + `get vpn ipsec tunnel summary`. A device with VDOMs,
   or without every tunnel in TUNNELS, is left untouched (`edit` of a missing
   name would create it).
2. Only for the tunnels found down: `set status down` on their interfaces,
   flush the tunnel and clear its IKE gateway, wait, then `set status up`
   (always, even if a step before failed).
3. Check `get vpn ipsec tunnel summary` every VERIFY_EVERY s, up to VERIFY_FOR s.
A tunnel is down when not every selector is up: `selectors(total,up): 1/0`.
"""

import re

import zapterm

# ── settings ────────────────────────────────────────────────────────────────
FOLDER = "SASE"           # saved sessions in this folder (and its sub-folders)…
DEVICES: list[str] = []   # …or list saved-session names here instead
TUNNELS = ("SASE_INET", "SASE_INET2")
AFTER_DOWN = 1            # seconds after `set status down`
AFTER_FLUSH = 30          # seconds after the flush / IKE clear
VERIFY_FOR = 60           # check the tunnels for up to this long…
VERIFY_EVERY = 5          # …every this many seconds
CLOSE_TABS = True         # close each device's tab when done


def selectors(summary: str, tunnel: str):
    """(total, up) for `tunnel` in a tunnel summary, or None if absent."""
    m = re.search(rf"'{re.escape(tunnel)}'.*?selectors\(total,up\):\s*(\d+)/(\d+)", summary)
    return (int(m.group(1)), int(m.group(2))) if m else None


def is_up(summary: str, tunnel: str) -> bool:
    sel = selectors(summary, tunnel)
    return bool(sel) and sel[0] > 0 and sel[0] == sel[1]


def describe(summary: str) -> str:
    return ", ".join(f"{t} {selectors(summary, t) or '?'}" for t in TUNNELS)


def set_status(s: zapterm.Session, tunnels, status: str) -> None:
    s.run("config system interface")
    for t in tunnels:
        s.run(f"edit {t}")
        s.run(f"set status {status}")
        s.run("next")
    s.run("end")


def reset_device(name: str) -> None:
    s = zapterm.open(name)
    try:
        status = s.run("get system status")
        if not zapterm.dry_run and "Virtual domain configuration: disable" not in status:
            zapterm.report(name, "LEFT UNTOUCHED", "VDOMs enabled (or unexpected status)")
            return
        summary = s.run("get vpn ipsec tunnel summary")
        missing = [t for t in TUNNELS if selectors(summary, t) is None]
        if missing and not zapterm.dry_run:
            zapterm.report(name, "LEFT UNTOUCHED", f"no tunnel {', '.join(missing)}")
            return
        down = [t for t in TUNNELS if not is_up(summary, t)]
        if not down:
            zapterm.report(name, "OK — already up, nothing sent", describe(summary))
            return
        if zapterm.read_only:
            # Real data, no changes: say what a real run would reset.
            zapterm.report(name, "READ-ONLY — nothing changed", f"would reset {', '.join(down)}")
            return

        set_status(s, down, "down")
        try:
            zapterm.sleep(AFTER_DOWN)
            for t in down:
                s.run(f"diagnose vpn tunnel flush {t}")
                s.run(f"diagnose vpn ike gateway clear name {t}")
            zapterm.sleep(AFTER_FLUSH)
        finally:
            set_status(s, down, "up")  # always bring them back up

        ok, last = s.poll(
            "get vpn ipsec tunnel summary",
            until=lambda out: all(is_up(out, t) for t in TUNNELS),
            every=VERIFY_EVERY,
            max=VERIFY_FOR,
        )
        if zapterm.dry_run:
            zapterm.report(name, "DRY RUN — nothing sent", f"would reset {', '.join(down)}")
        else:
            zapterm.report(name, "OK" if ok else f"NOT UP after {VERIFY_FOR} s", describe(last))
    finally:
        if CLOSE_TABS:
            s.close()


devices = DEVICES or zapterm.sessions(folder=FOLDER)
if not devices:
    raise SystemExit(f'No devices: put saved sessions in folder "{FOLDER}" or fill DEVICES.')
zapterm.log(f"{len(devices)} device(s): {', '.join(devices)}")
if not zapterm.confirm(f"Reset the SASE tunnels found down on {len(devices)} device(s)?"):
    raise SystemExit("Cancelled.")

for name in devices:
    try:
        reset_device(name)
    except zapterm.Error as e:  # one device failing doesn't stop the others
        zapterm.report(name, "FAILED", str(e))
