"""ZapTerm scripting API.

ZapTerm runs your script with Python 3 and puts this module on its path. Every
call below is carried out by ZapTerm on a real terminal tab — you see each
command and its output there — while your script only decides what to do.

    import zapterm

    s = zapterm.session()                       # the tab focused when the script started
    out = s.run("get system status")         # send, wait for the prompt, return the output
    if "HA mode: standalone" in out:
        zapterm.report(s.name, "OK", "standalone")

Passwords never reach the script: ``send_secret("Name.Password")`` asks ZapTerm to
type a credential from its vault straight into the session.

In **Dry run** mode, ``zapterm.dry_run`` is True: nothing is sent or
opened, ``run()``/``expect()`` return an empty string and every action is
logged as "would …" instead.

In **Read-only** mode (``zapterm.read_only`` is True) sessions are opened and
display commands (``get``/``show`` by default) run for real; anything else is
skipped and logged as "would run", and ``run()`` returns an empty string for
it. ZapTerm enforces this, not the script.

From a notebook (VS Code, JupyterLab) or a Python prompt you started yourself,
turn on **Tools → Allow notebook connection** in ZapTerm, then::

    import zapterm
    zapterm.connect()
    fw = zapterm.tab("FGT-60F-01")             # a tab already open, or zapterm.open(...)
    print(fw.run("get vpn ipsec tunnel summary"))
"""

from __future__ import annotations

import json
import os
import random
import socket
import sys
import threading
import time
from pathlib import Path
from typing import Callable, List, Optional, Tuple

__all__ = [
    "Error",
    "Timeout",
    "Session",
    "dry_run",
    "read_only",
    "connect",
    "session",
    "tab",
    "tabs",
    "open",
    "sessions",
    "report",
    "confirm",
    "sleep",
    "log",
]

#: True when ZapTerm started the script in dry-run mode.
dry_run: bool = os.environ.get("ZAPTERM_DRY_RUN") == "1"

_env_read_only: bool = os.environ.get("ZAPTERM_READ_ONLY") == "1"


def __getattr__(name: str):
    """``zapterm.read_only``: True in read-only mode — only display commands
    (``get``/``show`` by default) are typed; ZapTerm skips the rest and logs them
    as "would run", and ``run()`` returns an empty string for those. For a
    script it is fixed when ZapTerm starts it; from a notebook it is asked on each
    read, so it follows the connection's Read-only box in the panel."""
    if name == "read_only":
        if _notebook is not None:
            return bool((_call("mode") or {}).get("read_only"))
        return _env_read_only
    raise AttributeError(f"module 'zapterm' has no attribute {name!r}")


class Error(Exception):
    """A request ZapTerm could not carry out (no session, unknown name, …)."""


class Timeout(Error):
    """``run()``/``expect()``/``open()`` did not finish in time.

    ``output`` holds whatever the session printed before the deadline.
    """

    def __init__(self, message: str, output: str = "") -> None:
        super().__init__(message)
        self.output = output


# ── transport ────────────────────────────────────────────────────────────────

_lock = threading.Lock()
_sock: Optional[socket.socket] = None
_reader = None
# Random start: replies left over from an interrupted call (or an earlier
# connection) can never be taken for a new call's reply.
_next_id = random.randrange(1, 2**31) * 1000
#: How to reconnect after the connection drops (notebook mode): the arguments
#: of the last ``connect()``, or None for a script started by ZapTerm.
_notebook: Optional[dict] = None


def _open(port: int, token: str) -> None:
    """Open the channel and present the token; raises Error if refused."""
    global _sock, _reader
    # Replacing our own connection (a notebook re-running its connect() cell):
    # ZapTerm may not have seen the old one close yet and would refuse us as a
    # second notebook, so retry that refusal for a moment.
    replacing = _sock is not None
    _close()
    deadline = time.monotonic() + (3 if replacing else 0)
    while True:
        try:
            sock = socket.create_connection(("127.0.0.1", int(port)), timeout=10)
        except OSError:
            raise Error(
                "ZapTerm isn't accepting notebooks (is it open, with Tools → Allow "
                "notebook connection on?)"
            ) from None
        reader = sock.makefile("r", encoding="utf-8", newline="\n")
        sock.sendall((json.dumps({"token": token}) + "\n").encode("utf-8"))
        line = reader.readline()
        hello = json.loads(line) if line.strip() else {}
        if hello.get("hello") is True:
            break
        sock.close()
        message = hello.get("error") or (
            "ZapTerm refused the connection (token out of date? turn the "
            "notebook connection off and on, then connect() again)"
        )
        if "already connected" in message and time.monotonic() < deadline:
            time.sleep(0.1)
            continue
        raise Error(message)
    sock.settimeout(None)
    _sock, _reader = sock, reader


def _close() -> None:
    global _sock, _reader
    if _sock is not None:
        try:
            _sock.close()
        except OSError:
            pass
    _sock = _reader = None


def _connection_candidates(path: Optional[str]) -> List[Path]:
    if path:
        return [Path(path).expanduser()]
    found = []
    if os.environ.get("ZAPTERM_CONNECTION"):
        found.append(Path(os.environ["ZAPTERM_CONNECTION"]).expanduser())
    # This file is a copy ZapTerm keeps in <data dir>/scripting (and /examples).
    here = Path(__file__).resolve().parent
    found += [here / "connection.json", here.parent / "connection.json"]
    # ZapTerm's default data folder on each system.
    if sys.platform == "win32":
        base = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
    elif sys.platform == "darwin":
        base = Path.home() / "Library" / "Application Support"
    else:
        base = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
    found.append(base / "app.zapterm.client" / "scripting" / "connection.json")
    return found


def _connect_notebook() -> None:
    args = _notebook or {}
    port, token = args.get("port"), args.get("token")
    if port is None or token is None:
        tried = _connection_candidates(args.get("path"))
        for candidate in tried:
            if candidate.is_file():
                info = json.loads(candidate.read_text(encoding="utf-8"))
                port, token = info["port"], info["token"]
                break
        else:
            raise Error(
                "ZapTerm isn't accepting notebooks: turn on Tools → Allow notebook "
                "connection in ZapTerm (looked for "
                + ", ".join(str(t) for t in tried)
                + ")"
            )
    _open(port, token)


def _connect() -> None:
    if _notebook is not None:
        _connect_notebook()
        return
    port = os.environ.get("ZAPTERM_PORT")
    token = os.environ.get("ZAPTERM_TOKEN")
    if not port or not token:
        raise Error(
            "not connected to ZapTerm: in a notebook call zapterm.connect() first; "
            "a script must be started from ZapTerm (Tools → Run Python script…)"
        )
    _open(int(port), token)


def connect(path: Optional[str] = None, port: Optional[int] = None, token: Optional[str] = None) -> None:
    """Connect a notebook (or any Python you started yourself) to a running
    ZapTerm. Turn on **Tools → Allow notebook connection** in ZapTerm first.

    With no arguments the connection details are read from the file ZapTerm
    writes (found automatically); ``path`` points at that file when ZapTerm keeps
    its data somewhere else, or pass the ``port`` and ``token`` ZapTerm shows.
    If the connection drops later (ZapTerm restarted, connection turned off and
    on), the next call reconnects on its own.
    """
    global _notebook
    with _lock:
        _notebook = {"path": path, "port": port, "token": token}
        _connect_notebook()


def _call(method: str, **params):
    """Send one request and wait for its reply (calls are sequential)."""
    global _next_id
    with _lock:
        if _sock is None:
            _connect()
        _next_id += 1
        rid = _next_id
        req = {"id": rid, "method": method, "params": params}
        try:
            _sock.sendall((json.dumps(req) + "\n").encode("utf-8"))
            while True:
                line = _reader.readline()
                if not line:
                    _close()
                    raise Error(
                        "ZapTerm closed the connection"
                        + (
                            " (notebook connection turned off, or ZapTerm closed) — "
                            "run the cell again to reconnect"
                            if _notebook is not None
                            else ""
                        )
                    )
                reply = json.loads(line)
                if reply.get("id") == rid:
                    break  # anything else is a late reply to a cancelled call
        except KeyboardInterrupt:
            # Stopped cell / Ctrl+C: ask ZapTerm to stop waiting on our behalf.
            _next_id += 1
            cancel = {"id": _next_id, "method": "cancel", "params": {"id": rid}}
            try:
                _sock.sendall((json.dumps(cancel) + "\n").encode("utf-8"))
            except (OSError, AttributeError):
                pass
            raise
        except OSError as e:
            _close()
            raise Error(f"lost the connection to ZapTerm: {e}") from None
    if reply.get("ok"):
        return reply.get("result")
    err = reply.get("error") or {}
    message = err.get("message", "unknown error")
    if err.get("kind") == "timeout":
        raise Timeout(message, err.get("output", ""))
    raise Error(message)


# ── sessions ─────────────────────────────────────────────────────────────────


class Session:
    """A live terminal tab driven by the script."""

    def __init__(self, info: dict) -> None:
        self.id: str = info["id"]
        self.name: str = info.get("name", "")
        #: Detected platform key (``"fortigate"``, ``"cisco_ios"``…) or None.
        self.platform: Optional[str] = info.get("platform")

    def __repr__(self) -> str:
        return f"<zapterm.Session {self.name!r}>"

    def run(self, command: str, timeout: float = 30) -> str:
        """Type ``command`` + Enter and return its output.

        The output is what the device printed after echoing the command, up
        to (not including) the prompt that ends it. Raises :class:`Timeout`
        when the prompt does not come back within ``timeout`` seconds.
        """
        return _call("run", session=self.id, command=command, timeout=timeout)

    def send(self, text: str, enter: bool = True) -> None:
        """Type ``text`` (and Enter, unless ``enter=False``) without waiting."""
        _call("send", session=self.id, text=text, enter=enter)

    def expect(self, pattern: str, timeout: float = 30, regex: bool = False) -> str:
        """Wait until ``pattern`` appears in the output and return the text up
        to and including it. Output already consumed by an earlier
        ``run()``/``expect()`` is not searched again."""
        return _call(
            "expect", session=self.id, pattern=pattern, timeout=timeout, regex=regex
        )

    def send_secret(self, ref: str, enter: bool = True) -> None:
        """Type a vault credential: ``"Name.Password"`` or ``"Name.Username"``.
        The value goes from ZapTerm's vault to the session; the script never
        sees it."""
        _call("send_secret", session=self.id, ref=ref, enter=enter)

    def poll(
        self,
        command: str,
        until: Callable[[str], bool],
        every: float = 5,
        max: float = 60,
        timeout: float = 30,
    ) -> Tuple[bool, str]:
        """Run ``command`` every ``every`` seconds until ``until(output)`` is
        true or ``max`` seconds have passed. Returns ``(ok, last_output)``."""
        deadline = time.monotonic() + max
        while True:
            out = self.run(command, timeout=timeout)
            if until(out):
                return True, out
            if dry_run or time.monotonic() + every > deadline:
                return False, out
            sleep(every)

    def close(self) -> None:
        """Close this tab."""
        _call("close", session=self.id)


def session() -> Session:
    """The focused tab: in a script, call it first thing to get the tab that
    was focused when you ran it; in a notebook, the one focused now."""
    return Session(_call("session"))


def tabs() -> List[Session]:
    """The terminal tabs open in ZapTerm right now."""
    return [Session(t) for t in _call("tabs")]


def tab(name: str) -> Session:
    """An already open tab, by its name (as shown on the tab). Raises
    :class:`Error` if no tab has that name; with several, the first."""
    return Session(_call("tab", name=name))


def open(name: str, timeout: float = 60) -> Session:  # noqa: A001 - mirrors "open a session"
    """Open the saved session called ``name`` in a new tab and wait until it
    is connected and its output settles (jump host, vault password and login
    script are handled by ZapTerm as for any saved session)."""
    return Session(_call("open", name=name, timeout=timeout))


def sessions(folder: Optional[str] = None) -> List[str]:
    """Names of the saved sessions, or only those in ``folder`` (and its
    sub-folders) when given."""
    return list(_call("sessions", folder=folder))


# ── reporting & interaction ──────────────────────────────────────────────────


def report(target: str, status: str, detail: str = "") -> None:
    """Add a row to the run's summary table, e.g.
    ``report("FGT-60F-01", "OK", "SASE_INET 1/1")``."""
    _call("report", target=target, status=status, detail=detail)


def confirm(message: str) -> bool:
    """Ask the user a yes/no question in ZapTerm. In dry-run mode the answer is
    always True (nothing will be sent anyway)."""
    return bool(_call("confirm", message=message))


def sleep(seconds: float) -> None:
    """Pause the script. Skipped (only logged) in dry-run mode."""
    if dry_run:
        _call("note", text=f"[dry-run] would wait {seconds:g} s")
        return
    time.sleep(seconds)


def log(*args, sep: str = " ") -> None:
    """Write a line to ZapTerm's script panel, in order with the actions around it
    (plain ``print`` also shows up, but may land a little out of order)."""
    _call("note", text=sep.join(str(a) for a in args))
