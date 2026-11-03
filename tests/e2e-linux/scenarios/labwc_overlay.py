"""Cursor overlay legs on the private labwc tier. Where the marker is drawn is read from the
output through grim, its helper from /proc, and every effect beneath it through raw AT-SPI."""
from __future__ import annotations

import tempfile
import threading
from pathlib import Path

from oracle import atspi_raw as raw, pixels, proc, wlr
from oracle.gsettings import tool
from runner.apps import stop
from runner.helper import changed, command, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.headed_actions import find_ref
from scenarios.labwc_input import (CALCULATOR_ID, SNAPSHOT_DEPTH, _calculator, _clear_calculator,
                                   activate_by_pointer, button, capture_offered, placement)
from scenarios.labwc_windows import active_facts, frame_facts

RIM = (17, 19, 24)
FILL = (255, 255, 255)
GLYPH = (32, 40)
BUBBLE = (232, 38)
RIPPLE = 54
INK = 30
FIRST = (200, 150)
SECOND = (900, 450)
PARK = (1200, 650)
HELPER_SESSION_ENV = "AGENT_DESKTOP_WLR_CURSOR_SESSION"
NEAR_PX = 4
SETTLE_S = 5.0


def _count(image: pixels.Image, box: tuple[int, int, int, int], colour) -> int:
    x, y, width, height = box
    left, top = max(x, 0), max(y, 0)
    right, bottom = min(x + width, image.width), min(y + height, image.height)
    return sum(1 for row in range(top, bottom) for column in range(left, right)
               if image.pixel(column, row) == colour)


def glyph_box(point: tuple[int, int], slack: int = 0) -> tuple[int, int, int, int]:
    """The dart's box with its tip on `point`: the dart is drawn in a 32x40 box whose tip
    sits 1 right and 5 down from the box's corner."""
    return (point[0] - 1 - slack, point[1] - 5 - slack, GLYPH[0] + 2 * slack,
            GLYPH[1] + 2 * slack)


def marker_at(image: pixels.Image, point: tuple[int, int], slack: int = 0) -> bool:
    """Whether the default dart - white body inside a dark rim - is drawn with its tip on
    `point`, give or take `slack` pixels."""
    box = glyph_box(point, slack)
    return _count(image, box, FILL) >= INK and _count(image, box, RIM) >= INK


def marker_near(image: pixels.Image, point: tuple[int, int], radius: int = NEAR_PX) -> bool:
    return marker_at(image, point, radius)


def rim_centre(image: pixels.Image) -> tuple[float, float] | None:
    """The centre of every rim-coloured pixel on the output, for a desktop that shows nothing
    else in that colour."""
    found = [(column, row) for row in range(image.height) for column in range(image.width)
             if image.pixel(column, row) == RIM]
    if len(found) < INK:
        return None
    xs, ys = [x for x, _ in found], [y for _, y in found]
    return ((min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2)


def ripple_pixels(image: pixels.Image, point: tuple[int, int]) -> int:
    """Accent-blue pixels in the ripple's 108-pixel box around `point`."""
    x, y = point
    count = 0
    for row in range(max(y - RIPPLE, 0), min(y + RIPPLE, image.height)):
        for column in range(max(x - RIPPLE, 0), min(x + RIPPLE, image.width)):
            red, _, blue = image.pixel(column, row)
            count += blue > 100 and blue > red + 60
    return count


def card(image: pixels.Image, point: tuple[int, int]) -> tuple[int, int]:
    """Opaque white card pixels and dark ink pixels inside the label card that core places
    18 pixels right of and below `point`."""
    box = (point[0] + 18 + 3, point[1] + 18 + 3, BUBBLE[0] - 6, BUBBLE[1] - 6)
    white = _count(image, box, FILL)
    x, y, width, height = box
    ink = sum(1 for row in range(y, y + height) for column in range(x, x + width)
              if 0 <= column < image.width and 0 <= row < image.height
              and max(image.pixel(column, row)) < 160)
    return white, ink


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
        activate_by_pointer(ctx, app, CALCULATOR_ID, button(root, "C"))
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
    region = glyph_box(FIRST)
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
        ctx.before("pixels.marker_place", place()[1])
        session_id = start_session(ctx, cursor=True)
        move(ctx, session_id, PARK)
        require(raw.poll(lambda: place() == (True, False), SETTLE_S),
                f"the marker is not parked at {PARK}")
        ref = session_ref(ctx, app, session_id, "7")
        command(ctx, ["--session", session_id, "click", ref], headed=False)
        ctx.after("atspi.display", changed(read, "7"))
        expected = (False, True) if proven else (None, False)
        matches = lambda: all(seen == want for seen, want in zip(place(), expected)
                              if want is not None)
        settled = raw.poll(matches, SETTLE_S, floor_s=0.0 if proven else 1.0)
        ctx.after("pixels.marker_place", place()[1])
        require(settled and (proven or not place()[1]), f"marker (parked, on target) is {place()}, expected {expected} when "
                         f"the window's place is {'proven' if proven else 'unproven'}")
        disable_and_reap(ctx, session_id)
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
        ctx.restored("pixels.marker_place", place()[1])
    finally:
        if session_id is not None:
            end_session(ctx, session_id)
        stop(app)


class Sampler:
    """Captures the output, or one layout region of it, again and again on a thread until
    stopped, for effects that last only a moment. Captures are decoded only after sampling
    ends, so decoding never slows the sampling down."""

    def __init__(self, ctx: LegContext, region: tuple[int, int, int, int] | None = None):
        self.ctx, self.captured, self.done = ctx, [], threading.Event()
        if region is None:
            self.source = ["-o", output_name(ctx)]
        else:
            x, y, width, height = region
            self.source = ["-g", f"{x},{y} {width}x{height}"]
        self.thread = threading.Thread(target=self._run, daemon=True)

    def _run(self) -> None:
        with tempfile.TemporaryDirectory(prefix="glide-", dir=self.ctx.tier.runtime_dir) as directory:
            index = 0
            while not self.done.is_set():
                path = Path(directory) / f"{index}.png"
                tool(self.ctx.tier, ["grim", "-l", "0", *self.source, str(path)],
                     timeout_s=10.0)
                self.captured.append(path.read_bytes())
                index += 1

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.done.set()
        self.thread.join(timeout=15)

    def decoded(self) -> list[pixels.Image]:
        return [pixels.decode_png(data) for data in self.captured]


@implementation("labwc.cursor_overlay_glides_between_points")
def cursor_overlay_glides_between_points(ctx: LegContext) -> None:
    read = lambda: markers(ctx, (FIRST, SECOND))
    clean = {f"{x},{y}": False for x, y in (FIRST, SECOND)}
    ctx.before("pixels.markers", changed(read, clean))
    session_id = start_session(ctx, cursor=False)
    try:
        command(ctx, ["--session", session_id, "cursor-overlay", "enable", "--travel-ms",
                      "600,700"], headed=False)
        move(ctx, session_id, FIRST)
        changed(read, {**clean, f"{FIRST[0]},{FIRST[1]}": True})
        with Sampler(ctx) as sampler:
            move(ctx, session_id, SECOND)
        landed = read()
        centres = [centre for centre in map(rim_centre, sampler.decoded()) if centre]
        between = [centre for centre in centres
                   if FIRST[0] + GLYPH[0] < centre[0] < SECOND[0] - GLYPH[0]]
        ctx.after("pixels.markers", landed)
        require(between, f"no frame caught the dart between the points: {centres}")
        require(landed == {**clean, f"{SECOND[0]},{SECOND[1]}": True},
                f"the dart did not land on {SECOND}: {landed}")
        disable_and_reap(ctx, session_id)
        ctx.restored("pixels.markers", changed(read, clean))
    finally:
        end_session(ctx, session_id)


@implementation("labwc.cursor_overlay_click_ripple")
def cursor_overlay_click_ripple(ctx: LegContext) -> None:
    """The click lands on a Calculator digit rather than the bare desktop: a desktop click opens
    labwc's root menu at the click point, and labwc draws its menus above every layer-shell
    surface, so the menu would cover the dart that the product did draw over its ripple."""
    app, root, read = _calculator(ctx)
    session_id = None
    try:
        point = wlr.placed_point(placement(CALCULATOR_ID), button(root, "8").extents(True))
        rippling = lambda: ripple_pixels(screen(ctx), point)
        before, bare = read(), rippling()
        ctx.before("atspi.display", before)
        ctx.before("pixels.ripple", bare)
        require(bare == 0, f"accent-blue pixels around {point} before any click: {bare}")
        session_id = start_session(ctx, cursor=True)
        move(ctx, session_id, point)
        require(raw.poll(lambda: marker_at(screen(ctx), point), SETTLE_S), "no dart to click")
        with Sampler(ctx) as sampler:
            move(ctx, session_id, point, action="mouse-click")
            raw.poll(lambda: False, 1.2)
        ctx.after("atspi.display", changed(read, "8"))
        frames = sampler.decoded()
        counts = [ripple_pixels(frame, point) for frame in frames]
        ctx.after("pixels.ripple", max(counts, default=0))
        require(max(counts, default=0) > 200, f"no ripple frame at the click point: {counts}")
        darts = [marker_at(frame, point) for frame in frames]
        rippled = [index for index, count in enumerate(counts) if count > 200]
        require(all(darts[index] for index in rippled),
                f"the dart was not over its ripple: {list(zip(counts, darts))}")
        require(counts[-1] == 0, f"the ripple did not end: {counts}")
        require(darts[-1], f"the dart did not stay at the click point: {list(zip(counts, darts))}")
        disable_and_reap(ctx, session_id)
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
        ctx.restored("pixels.ripple", rippling())
    finally:
        if session_id is not None:
            end_session(ctx, session_id)
        stop(app)


@implementation("labwc.cursor_overlay_label_card")
def cursor_overlay_label_card(ctx: LegContext) -> None:
    read = lambda: card(screen(ctx), FIRST)
    area = (BUBBLE[0] - 6) * (BUBBLE[1] - 6)
    present = lambda: read()[0] > area // 2 and read()[1] > 40
    ctx.before("pixels.card", present())
    session_id = start_session(ctx, cursor=False)
    try:
        command(ctx, ["--session", session_id, "cursor-overlay", "enable", "--label",
                      "Clicking eight"], headed=False)
        move(ctx, session_id, FIRST)
        shown = raw.poll(present, SETTLE_S)
        ctx.after("pixels.card", bool(shown))
        require(shown, f"no card with text beside {FIRST}: white, ink = {read()}")
        disable_and_reap(ctx, session_id)
        ctx.restored("pixels.card", present())
    finally:
        end_session(ctx, session_id)
