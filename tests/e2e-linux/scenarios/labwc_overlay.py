"""Cursor overlay legs on the private labwc tier. Where the marker is drawn is read from the
output through grim, its helper from /proc, and every effect beneath it through raw AT-SPI."""
from __future__ import annotations

import tempfile
from pathlib import Path

from oracle import atspi_raw as raw, pixels, proc, wlr
from runner.apps import stop
from runner.helper import changed, command, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.headed_actions import find_ref
from scenarios.labwc_input import (CALCULATOR_ID, SNAPSHOT_DEPTH, _calculator, _clear_calculator,
                                   button, capture_offered, placement)
from scenarios.labwc_windows import active_facts, frame_facts

RIM = (17, 19, 24)
FILL = (255, 255, 255)
MARKER = (24, 32)
FIRST = (200, 150)
SECOND = (900, 450)
PARK = (1200, 650)
HELPER_SESSION_ENV = "AGENT_DESKTOP_WLR_CURSOR_SESSION"
NEAR_PX = 3
SETTLE_S = 5.0


def marker_at(image: pixels.Image, point: tuple[int, int]) -> bool:
    """Whether the default arrow's tip sits on `point`: rim at the tip and down its left edge,
    fill inside the body."""
    x, y = point
    if x < 0 or y < 0 or x + MARKER[0] > image.width or y + MARKER[1] > image.height:
        return False
    return (image.pixel(x, y) == RIM and image.pixel(x, y + 12) == RIM
            and image.pixel(x + 3, y + 12) == FILL and image.pixel(x + 12, y + 22) == FILL)


def marker_near(image: pixels.Image, point: tuple[int, int], radius: int = NEAR_PX) -> bool:
    return any(marker_at(image, (point[0] + dx, point[1] + dy))
               for dx in range(-radius, radius + 1) for dy in range(-radius, radius + 1))


def helpers(ctx: LegContext, session_id: str) -> list[int]:
    """Processes of this tier that serve `session_id`'s overlay, found by their environment."""
    found = []
    for directory in Path("/proc").iterdir():
        if not directory.name.isdecimal():
            continue
        try:
            _, _, _, env = proc.facts(int(directory.name))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        if (env.get(HELPER_SESSION_ENV) == session_id
                and env.get("XDG_RUNTIME_DIR") == str(ctx.tier.runtime_dir)):
            found.append(int(directory.name))
    return sorted(found)


def output_name(ctx: LegContext) -> str:
    outputs = wlr.outputs(ctx.tier)
    require(len(outputs) == 1, f"expected one labwc output, found {outputs}")
    return outputs[0].name


def screen(ctx: LegContext) -> pixels.Image:
    return wlr.capture(ctx.tier, output_name(ctx))


def markers(ctx: LegContext, points) -> dict[str, bool]:
    image = screen(ctx)
    return {f"{x},{y}": marker_at(image, (x, y)) for x, y in points}


def start_session(ctx: LegContext, *, cursor: bool) -> str:
    argv = ["session", "start", "--name", "overlay"] + (["--cursor"] if cursor else [])
    data = command(ctx, argv, headed=False).data or {}
    require(not cursor or data.get("rendered") is True,
            f"session start --cursor did not render on wlroots: {data}")
    return data["session_id"]


def end_session(ctx: LegContext, session_id: str) -> None:
    ctx.product(["session", "end", session_id], headed=False)


def move(ctx: LegContext, session_id: str, point: tuple[int, int], action="mouse-move") -> None:
    command(ctx, ["--session", session_id, action, "--xy", f"{point[0]},{point[1]}"],
            headed=True)


def disable_and_reap(ctx: LegContext, session_id: str) -> None:
    command(ctx, ["--session", session_id, "cursor-overlay", "disable"], headed=False)
    require(raw.poll(lambda: not helpers(ctx, session_id), SETTLE_S),
            f"the overlay helper survived disable: {helpers(ctx, session_id)}")


@implementation("labwc.cursor_overlay_follows_moves")
def cursor_overlay_follows_moves(ctx: LegContext) -> None:
    read = lambda: markers(ctx, (FIRST, SECOND))
    clean = {f"{x},{y}": False for x, y in (FIRST, SECOND)}
    baseline = screen(ctx)
    ctx.before("pixels.markers", changed(read, clean))
    session_id = start_session(ctx, cursor=False)
    try:
        ctx.before("proc.helpers", len(helpers(ctx, session_id)))
        enabled = command(ctx, ["--session", session_id, "cursor-overlay", "enable"],
                          headed=False)
        require((enabled.data or {}).get("rendered") is True, f"enable: {enabled.raw}")
        require(len(helpers(ctx, session_id)) == 1, "enable started no single overlay helper")
        move(ctx, session_id, FIRST)
        changed(read, {**clean, f"{FIRST[0]},{FIRST[1]}": True})
        move(ctx, session_id, SECOND)
        ctx.after("pixels.markers", changed(read, {**clean, f"{SECOND[0]},{SECOND[1]}": True}))
        ctx.after("proc.helpers", len(helpers(ctx, session_id)))
        disable_and_reap(ctx, session_id)
        ctx.restored("pixels.markers", changed(read, clean))
        require(pixels.similar(baseline, screen(ctx), tolerance=0.0),
                "the output still differs from before enable after disable")
        ctx.restored("proc.helpers", len(helpers(ctx, session_id)))
    finally:
        end_session(ctx, session_id)


def listed_windows(ctx: LegContext) -> list[tuple]:
    """Every window list-windows reports, the overlay's own included if it ever appeared."""
    data = command(ctx, ["list-windows"]).data or []
    return sorted((item.get("id"), item.get("title"), item.get("pid")) for item in data)


@implementation("labwc.cursor_overlay_click_passes_through")
def cursor_overlay_click_passes_through(ctx: LegContext) -> None:
    app, root, read = _calculator(ctx)
    session_id = None
    try:
        point = wlr.placed_point(placement(CALCULATOR_ID), button(root, "8").extents(True))
        frames = lambda: frame_facts(ctx, (app,))
        before = read()
        ctx.before("atspi.display", before)
        ctx.before("atspi.frames", frames())
        active = active_facts(ctx, (app,))
        windows = listed_windows(ctx)
        session_id = start_session(ctx, cursor=True)
        move(ctx, session_id, point)
        require(raw.poll(lambda: marker_at(screen(ctx), point), SETTLE_S),
                f"no marker over the 8 button at {point}")
        require(active_facts(ctx, (app,)) == active,
                "showing the marker changed which window is active")
        require(listed_windows(ctx) == windows, "the marker changed what list-windows lists")
        require(read() == before, f"hovering through the marker changed the display: {read()!r}")
        move(ctx, session_id, point, action="mouse-click")
        ctx.after("atspi.display", changed(read, "8"))
        require(marker_at(screen(ctx), point), "the marker left the clicked point")
        require(listed_windows(ctx) == windows, "the marker changed what list-windows lists")
        ctx.after("atspi.frames", frames())
        disable_and_reap(ctx, session_id)
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
        ctx.restored("atspi.frames", frames())
    finally:
        if session_id is not None:
            end_session(ctx, session_id)
        stop(app)


@implementation("labwc.cursor_overlay_hidden_from_screenshots")
def cursor_overlay_hidden_from_screenshots(ctx: LegContext) -> None:
    baseline = screen(ctx)
    region = (*FIRST, *MARKER)
    on_output = lambda: marker_at(screen(ctx), FIRST)
    ctx.before("pixels.marker_on_output", on_output())
    session_id = start_session(ctx, cursor=True)
    try:
        move(ctx, session_id, FIRST)
        require(raw.poll(on_output, SETTLE_S), f"no marker at {FIRST} after the move")
        ctx.before("pixels.screenshot_marker", False)
        with tempfile.TemporaryDirectory(prefix="shot-", dir=ctx.tier.runtime_dir) as directory:
            path = Path(directory) / "screen.png"
            command(ctx, ["--session", session_id, "screenshot", "--screen", "0", str(path)])
            shot = pixels.decode_png(path.read_bytes())
        captured = marker_near(shot, FIRST)
        ctx.after("pixels.screenshot_marker", captured)
        require(not captured, "the screenshot shows the cursor overlay")
        require(pixels.similar(baseline, shot, region, tolerance=0.0),
                "the screenshot differs from the bare output where the marker is drawn")
        ctx.after("pixels.marker_on_output", raw.poll(on_output, SETTLE_S))
        require(on_output(), "the marker did not come back after the screenshot")
        disable_and_reap(ctx, session_id)
        ctx.restored("pixels.marker_on_output", on_output())
        ctx.restored("pixels.screenshot_marker", False)
    finally:
        end_session(ctx, session_id)


def session_ref(ctx: LegContext, app, session_id: str, name: str) -> str:
    response = command(ctx, ["--session", session_id, "snapshot", "-i", "--max-depth",
                             SNAPSHOT_DEPTH], app, headed=False)
    ref = find_ref((response.data or {}).get("tree"), name=name, role="button")
    require(ref is not None, f"{app.name} snapshot has no {name} button ref")
    return ref


@implementation("labwc.cursor_overlay_semantic_marks_proven_only")
def cursor_overlay_semantic_marks_proven_only(ctx: LegContext) -> None:
    proven = capture_offered(ctx)
    app, root, read = _calculator(ctx)
    session_id = None
    try:
        target = wlr.placed_point(placement(CALCULATOR_ID), button(root, "7").extents(True))
        place = lambda: (marker_at(screen(ctx), PARK), marker_near(screen(ctx), target))
        before = read()
        ctx.before("atspi.display", before)
        ctx.before("pixels.marker_place", place())
        session_id = start_session(ctx, cursor=True)
        move(ctx, session_id, PARK)
        require(raw.poll(lambda: place() == (True, False), SETTLE_S),
                f"the marker is not parked at {PARK}")
        ref = session_ref(ctx, app, session_id, "7")
        command(ctx, ["--session", session_id, "click", ref], headed=False)
        ctx.after("atspi.display", changed(read, "7"))
        expected = (False, True) if proven else (True, False)
        settled = raw.poll(lambda: place() == expected, SETTLE_S)
        ctx.after("pixels.marker_place", place())
        require(settled, f"marker (parked, on target) is {place()}, expected {expected} when "
                         f"the window's place is {'proven' if proven else 'unproven'}")
        disable_and_reap(ctx, session_id)
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
        ctx.restored("pixels.marker_place", place())
    finally:
        if session_id is not None:
            end_session(ctx, session_id)
        stop(app)
