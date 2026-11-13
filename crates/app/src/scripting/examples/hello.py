"""Smallest ZapTerm script: look at the focused tab and run one command.

Run it from Tools → Run Python script…, as a Dry run first: nothing is sent,
the panel lists what would be. Then choose Run. (Read-only runs it for real on
a FortiGate tab, where it types `get system status`; on another tab its
`uname -a` isn't in the default get/show list, so it is only logged.)
"""

import zapterm

s = zapterm.session()  # the tab that was focused when the script started
zapterm.log(f"Tab: {s.name} — platform: {s.platform or 'unknown'}")

# run() types the command + Enter, waits for the prompt and returns the output.
out = s.run("get system status" if s.platform == "fortigate" else "uname -a")

first_line = out.splitlines()[0] if out else ("(dry run: no output)" if zapterm.dry_run else "(no output)")
zapterm.report(s.name, "OK", first_line)
