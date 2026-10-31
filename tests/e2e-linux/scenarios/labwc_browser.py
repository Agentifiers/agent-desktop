"""Chromium on the private labwc tier. Chromium rewrites its process environment, so the
runtime-directory attribution the other apps use cannot see it: the leg owns the browser by
the PID it started, its session and its private profile, and stops every such process."""
from __future__ import annotations

import os
import shutil
import signal
import subprocess
import tempfile
from pathlib import Path

from oracle import atspi_raw as raw
from oracle.x11 import encode_product_id
from runner import session
from runner.helper import activate, changed, command, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.headed_actions import find_ref

HEADING = 83
FRAME_ROLES = (23, 69)
WAITING = "Waiting"
CLICKED = "Clicked"
PAGE = f"""<!doctype html>
<html><head><title>Harness Page</title></head><body>
<h1 id="status">{WAITING}</h1>
<button style="font-size:32px;padding:24px"
  onclick="document.getElementById('status').textContent='{CLICKED}';document.title='Harness {CLICKED}'">Press me</button>
</body></html>
"""


def browser_binary(which=shutil.which) -> str:
    for name in session.BROWSERS:
        if which(name):
            return name
    raise AssertionError(f"no browser among {', '.join(session.BROWSERS)}")


def browser_argv(binary: str, profile: Path, url: str) -> list[str]:
    return [binary, f"--user-data-dir={profile}", "--ozone-platform=wayland",
            "--force-renderer-accessibility", "--no-first-run", "--no-default-browser-check",
            "--password-store=basic", "--disable-crash-reporter", url]


def _session_id(stat: str) -> int:
    return int(stat[stat.rindex(")") + 2:].split()[3])


def owned_processes(leader: int, markers: tuple[str, ...],
                    proc_root: Path = Path("/proc")) -> list[int]:
    """PIDs in the browser's session, or with an argument that starts with one of `markers`
    (crash handlers leave the session but keep the private profile or database path)."""
    found = []
    for directory in proc_root.glob("[0-9]*"):
        try:
            stat = (directory / "stat").read_text()
            args = (directory / "cmdline").read_bytes().split(b"\0")
        except (OSError, ValueError):
            continue
        try:
            in_session = _session_id(stat) == leader
        except (ValueError, IndexError):
            continue
        if in_session or any(arg.startswith(marker.encode()) for arg in args for marker in markers):
            found.append(int(directory.name))
    return sorted(found)


class Browser:
    def __init__(self, ctx: LegContext, url: str, profile: Path):
        env = dict(ctx.tier.env, ACCESSIBILITY_ENABLED="1")
        env.pop("AT_SPI_BUS_ADDRESS", None)
        self.markers = (f"--user-data-dir={profile}",
                        f"--database={ctx.tier.runtime_dir}/")
        self.log = ctx.tier.runtime_dir / "chromium.log"
        with self.log.open("ab") as stream:
            self.child = subprocess.Popen(browser_argv(browser_binary(), profile, url), env=env,
                                          stdin=subprocess.DEVNULL, stdout=stream, stderr=stream,
                                          start_new_session=True)

    def stop(self) -> None:
        for signum in (signal.SIGTERM, signal.SIGKILL):
            for pid in owned_processes(self.child.pid, self.markers):
                try:
                    os.kill(pid, signum)
                except ProcessLookupError:
                    pass
            try:
                self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            if raw.poll(lambda: not owned_processes(self.child.pid, self.markers), 5.0):
                return
        require(not owned_processes(self.child.pid, self.markers),
                f"browser processes survived: {owned_processes(self.child.pid, self.markers)}")


def browser_root(ctx: LegContext, pid: int):
    def locate():
        for node in raw.registry(ctx.tier):
            try:
                if raw.owner_pid(node) == pid and [frame for frame in node.children()
                                                   if frame.role in FRAME_ROLES]:
                    return node
            except Exception as error:
                if "NameHasNoOwner" in str(error) or raw.unresponsive(error):
                    continue
                raise
        return None

    found = raw.poll(locate, 30.0)
    require(found is not None, "the browser never registered an AT-SPI frame")
    return found


def headings(root) -> tuple[str, ...]:
    return tuple(node.name for node in root.walk() if node.role == HEADING)


@implementation("labwc.chromium_headed_click")
def chromium_headed_click(ctx: LegContext) -> None:
    with tempfile.TemporaryDirectory(prefix="chromium-", dir=ctx.tier.runtime_dir) as directory:
        page = Path(directory) / "page.html"
        page.write_text(PAGE)
        browser = Browser(ctx, page.as_uri(), Path(directory) / "profile")
        try:
            root = browser_root(ctx, browser.child.pid)
            read = lambda: headings(root)
            before = changed(read, (WAITING,), seconds=20)
            ctx.before("atspi.heading", before)
            frame = next(node for node in root.children() if node.role in FRAME_ROLES)
            window = encode_product_id(frame.bus, frame.path)
            command(ctx, ["maximize"], window_id=window)
            command(ctx, ["focus-window", "--window-id", window])
            snapshot = command(ctx, ["snapshot", "-i", "--max-depth", "50"], headed=False,
                               window_id=window)
            ref = find_ref((snapshot.data or {}).get("tree"), name="Press me", role="button")
            require(ref is not None, "the page button has no ref")
            command(ctx, ["click", ref])
            ctx.after("atspi.heading", changed(read, (CLICKED,), seconds=5))
            reload = [node for node in root.walk() if node.role == 43 and node.name == "Reload"]
            require(len(reload) == 1, f"expected one raw Reload button, found {len(reload)}")
            activate(reload[0])
            command(ctx, ["restore"], window_id=window)
            ctx.restored("atspi.heading", changed(read, before, seconds=10))
        finally:
            browser.stop()
