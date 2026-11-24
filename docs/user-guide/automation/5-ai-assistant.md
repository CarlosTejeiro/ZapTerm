# 5. Connect an AI assistant (MCP)

[← 4. Add devices in bulk](4-bulk-add.md) · [Guide index](README.md)

ZapTerm includes an **MCP server**: a small program an AI assistant (Claude
Desktop, Claude Code or any other MCP client) starts to work on your devices
*through ZapTerm*. The assistant can list your tabs and saved sessions, open a
session and run commands — and ZapTerm decides what is actually sent.

> **What your devices print is sent to the assistant you connect**, and so to
> its provider: configurations, addresses, host names. ZapTerm itself still
> sends nothing anywhere; this happens only if you set it up, and only while
> the connection is on.

## How it is kept safe

| | |
|---|---|
| **You see everything** | Each command is typed in a real tab, as with scripts and notebooks, and kept in the [activity record](#what-it-did-automation-activity). |
| **Read-only by default** | Only commands starting with `get` or `show` (your list) reach a device. Anything else — including a display command chained with `;`, `|` or `&` — is refused, and the assistant is told it was not sent. |
| **Ask me first** | With Read-only off, ZapTerm shows every other command and waits for your **Yes** before sending it. |
| **No passwords** | ZapTerm logs in from its vault. No tool gives the assistant a password or lets it type one. |
| **Enforced by ZapTerm** | These rules live in the app, not in the MCP server or the assistant, so neither can skip them. |
| **Off by default** | Nothing can connect until you turn the connection on; it turns itself off after 60 minutes without calls. |

A device's output can contain text written by someone else (a banner, an
interface description). The tools tell the assistant to treat output as data,
never as instructions — and Read-only and Ask me first hold even if it doesn't.

## Set it up

You need Python 3.9 or newer on the same computer.

1. In ZapTerm: **Tools → Allow notebook connection…** (the assistant uses the
   same local connection as notebooks; one client at a time).
2. In the panel, open **Connect an AI assistant (MCP)** and **Copy** the
   configuration. It looks like this, with the path of your own installation:

   ```json
   {
     "mcpServers": {
       "zapterm": {
         "command": "python3",
         "args": ["/home/you/.local/share/app.zapterm.client/scripting/zapterm_mcp.py"]
       }
     }
   }
   ```

3. Add it to your MCP client's configuration and restart the client.

## What the assistant can do

| Tool | What it does |
|---|---|
| `list_tabs` | The tabs open in ZapTerm, with the detected platform. |
| `list_saved_sessions` | Names of your saved sessions, optionally of one folder. |
| `open_session` | Open a saved session in a new tab (jump host, vault and login script handled by ZapTerm). |
| `run_command` | Type one command line in a tab and return its output. |
| `get_mode` | Whether ZapTerm is in read-only mode. |

## Making changes

Untick **Read-only** in the panel. **Ask me first** appears, ticked: for each
command that is not a display command, ZapTerm shows it and waits.

![ZapTerm asking before a command](../images/mcp-approve.png)

**No** tells the assistant the command was declined. Untick *Ask me first*
only for work you would also hand to a script without watching.

## What it did: Automation activity

Everything the assistant sent — or tried to send — is kept, with what a
notebook or a script did: **Tools → Automation activity…** (or the pulse
button in the panel).

![Automation activity](../images/activity.png)

- **Each line**: the time, the tab and the device's address, the command, and
  what became of it — *Sent*, *Blocked · read-only*, *Declined by you*,
  *Approved · sent*, *No prompt in time* or *Error* — and how long it took.
- **One card per visit**: from the moment the assistant (shown by the name it
  gives), a notebook or a script connected until it left.
- **On top**: counters and the last 14 days; click one to see only that. Then
  filters by source and device, and a search box. Click a line to copy its
  command.
- **It is a text file on your computer**: `automation/activity-YYYY-MM.log`
  in the logs folder, one file per month, one line per event with
  tab-separated fields, so it opens in any editor or as a table in a
  spreadsheet. **Open file** and **Folder** take you there; delete old months
  whenever you like. Nothing is sent anywhere.
- **Only the command line is kept**, never what the device printed. To keep
  the output too, record the tab (the ● button in the pane header,
  [session logging](../README.md#11-session-logging)).
- **Passwords are masked**: the value after a secret keyword is written as
  `******` (`set password ******`, `secret 0 ******`, `snmp-server community
  ****** RO`, `--password=******`…), and what is typed in answer to a
  password prompt is masked whole. This is best effort: a password in a place
  no rule knows is kept as typed.
- **"Unidentified client"** means the assistant app (or notebook kernel) was
  already running when ZapTerm was updated and still uses the older files:
  quit it completely and open it again.

[← 4. Add devices in bulk](4-bulk-add.md) · [Guide index](README.md) · [Scripting reference](../scripting.md)
