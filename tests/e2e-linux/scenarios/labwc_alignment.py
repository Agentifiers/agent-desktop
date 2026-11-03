"""Cursor overlay alignment on the private labwc tier. Where the element is drawn is read from
its own pixels in a grim capture; where the overlay framed it and where its dart points are read
from the same output, and a headed click's landing from the page that received it. Each leg runs
at the tier's 1280x720 output and again at Amp's 928x1424 output at scale 1.601562, where grim
captures buffer pixels that are divided by the scale before any comparison."""
from __future__ import annotations

import contextlib
import tempfile
from pathlib import Path

from oracle import atspi_raw as raw, overlay_marks as marks, pixels, wlr
from oracle.gsettings import tool
from oracle.x11 import encode_product_id
from runner.apps import stop
from runner.helper import changed, command, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.headed_actions import find_ref
from scenarios.labwc_browser import (FRAME_ROLES, PUSH_BUTTON, Browser, browser_root,
                                     headings)
from scenarios.labwc_input import _calculator, _clear_calculator, button, maximize_and_focus
from scenarios.labwc_overlay import (Sampler, disable_and_reap, end_session, move,
                                     output_name, session_ref, start_session)

DEFAULT_MODE = "1280x720@60Hz"
FRACTIONAL_MODE = "928x1424@30Hz"
FRACTIONAL_SCALE = "1.6015625"
MARGIN = 60
EDGE_PX = 1.0
TIP_PX = 1.5
SIZE_PX = 2.0
SETTLE_S = 5.0
SEED_INSET = 6
DART_BOX = (-20, -20, 60, 64)
MAGENTA = (255, 0, 255)
WAITING = "hit none"
TARGET = "Target page"
FAR = "Far corner"
FAR_SIZE = (120, 40)
PAGE = f"""<!doctype html>
<html><head><title>Alignment Page</title><style>
html, body {{ margin: 0; padding: 0; background: #ffffff; }}
#hit {{ position: absolute; left: 20px; top: 10px; font-size: 16px; margin: 0; }}
#target {{ position: absolute; left: 160px; top: 180px; width: 240px; height: 90px; margin: 0;
           padding: 0; border: none; border-radius: 0; outline: none; background: #ff00ff;
           color: #000000; font-size: 20px; }}
#far {{ position: fixed; right: 48px; bottom: 12px; width: {FAR_SIZE[0]}px;
        height: {FAR_SIZE[1]}px; margin: 0; padding: 0; border: none; border-radius: 0;
        outline: none; background: #00ffff; color: #000000; font-size: 14px; }}
</style><script>
let clicks = 0;
let farClicks = 0;
function hit(event) {{
  clicks += 1;
  document.getElementById('hit').textContent =
    'hit ' + clicks + ' ' + event.offsetX + ',' + event.offsetY;
}}
function farHit(event) {{
  farClicks += 1;
  document.title = 'far ' + farClicks + ' ' + event.offsetX + ',' + event.offsetY;
}}
</script></head><body>
<h1 id="hit">{WAITING}</h1>
<button id="target" onclick="hit(event)">{TARGET}</button>
<button id="far" onclick="farHit(event)">{FAR}</button>
</body></html>
"""


@contextlib.contextmanager
def output_scale(ctx: LegContext, fractional: bool):
    """The tier's one output, switched to Amp's mode and scale for a fractional leg and put back
    to the tier's mode afterwards whatever the leg found."""
    name = output_name(ctx)
    if not fractional:
        yield wlr.outputs(ctx.tier)[0]
        return
    tool(ctx.tier, ["wlr-randr", "--output", name, "--custom-mode", FRACTIONAL_MODE,
                    "--scale", FRACTIONAL_SCALE])
    try:
        output = wlr.outputs(ctx.tier)[0]
        require((output.width, output.height) == (928, 1424) and abs(output.scale - 1.601562) < 1e-4,
                f"the output did not take {FRACTIONAL_MODE} at {FRACTIONAL_SCALE}: {output}")
        yield output
    finally:
        tool(ctx.tier, ["wlr-randr", "--output", name, "--custom-mode", DEFAULT_MODE,
                        "--scale", "1"])
        output = wlr.outputs(ctx.tier)[0]
        require((output.width, output.height, output.scale) == (1280, 720, 1.0),
                f"the output was not put back to {DEFAULT_MODE} at scale 1: {output}")


def logical_size(output: wlr.Output) -> tuple[int, int]:
    return round(output.width / output.scale), round(output.height / output.scale)


def region_around(extents, output: wlr.Output) -> tuple[int, int, int, int]:
    """A layout region around window-relative extents of a window that fills the output from
    its corner, wide enough that an offset outline or dart still falls inside it."""
    x, y, width, height = extents
    right, bottom = logical_size(output)
    left, top = max(x - MARGIN, 0), max(y - MARGIN, 0)
    return left, top, min(x + width + MARGIN, right) - left, min(y + height + MARGIN, bottom) - top


def capture(ctx: LegContext, region) -> pixels.Image:
    x, y, width, height = region
    with tempfile.TemporaryDirectory(prefix="align-", dir=ctx.tier.runtime_dir) as directory:
        path = Path(directory) / "region.png"
        tool(ctx.tier, ["grim", "-l", "0", "-g", f"{x},{y} {width}x{height}", str(path)],
             timeout_s=10.0)
        return pixels.decode_png(path.read_bytes())


def settled(ctx: LegContext, region) -> pixels.Image:
    """The region once two captures in a row agree: no glide, ripple or outline is moving."""
    last = [capture(ctx, region)]

    def still():
        image = capture(ctx, region)
        same = image.rgb == last[0].rgb
        last[0] = image
        return same

    require(raw.poll(still, SETTLE_S), "the region never stopped changing")
    return last[0]


def fills_output(frame, output: wlr.Output) -> bool:
    return tuple(frame.extents(True)) == (0, 0, *logical_size(output))


def dart_box(centre, region, scale: float) -> tuple[int, int, int, int]:
    left, top, right, bottom = DART_BOX
    to_buffer = lambda value, origin: round((value - origin) * scale)
    return (to_buffer(centre[0] + left, region[0]), to_buffer(centre[1] + top, region[1]),
            to_buffer(centre[0] + right, region[0]), to_buffer(centre[1] + bottom, region[1]))


class Alignment:
    """One element's true rectangle on the output and the overlay marks measured against it."""

    def __init__(self, ctx: LegContext, session_id: str, region, scale: float, truth):
        self.ctx, self.session_id, self.region, self.scale = ctx, session_id, region, scale
        self.truth = truth
        self.centre = (round(truth[0] + truth[2] / 2), round(truth[1] + truth[3] / 2))
        self.base = settled(ctx, region)
        self.box = dart_box(self.centre, region, scale)
        self.park = park_point(ctx)

    def outline(self, image: pixels.Image):
        return marks.outline_target(image, self.base, self.scale, self.region[:2])

    def dart(self, image: pixels.Image):
        return marks.dark_marks(image, self.base, self.box)

    def framed(self, ref: str, *, headed: bool):
        """The outline a click on `ref` draws, read from the frames sampled while it plays:
        the rectangle most frames agree on; and the last frame, where the dart rests on the
        click point before the overlay's idle fade can begin."""
        with Sampler(self.ctx, region=self.region) as sampler:
            command(self.ctx, ["--session", self.session_id, "click", ref], headed=headed)
            raw.poll(lambda: False, 1.6)
        frames = sampler.decoded()
        found = [rect for rect in map(self.outline, frames) if rect is not None]
        require(found, f"no frame showed an outline around the target after clicking {ref}")
        rounded = [tuple(round(value, 1) for value in rect) for rect in found]
        return max(set(rounded), key=rounded.count), frames[-1]

    def reference(self):
        """The dart's marks with its tip on the element's true centre, drawn by a headed move."""
        move(self.ctx, self.session_id, self.centre)
        return self.dart(settled(self.ctx, self.region))

    def check_outline(self, outline) -> None:
        """Within a pixel; at a fractional scale also within the buffer pixel the compositor
        rounds the element's edges and the outline's edges to, each its own way."""
        tolerance = EDGE_PX if self.scale == 1 else EDGE_PX + 1 / self.scale
        require(marks.within(outline, self.truth, tolerance),
                f"the outline frames {outline}, the element is drawn at {self.truth}")

    def check_dart(self, found, reference, what: str):
        shift = marks.mark_shift(found, reference, self.scale)
        require(shift is not None, f"no dart near the target after {what}")
        require(max(map(abs, shift)) <= TIP_PX,
                f"after {what} the dart points {shift} px from the element's centre {self.centre}")
        return tuple(round(value, 2) for value in shift)


def park_point(ctx: LegContext) -> tuple[int, int]:
    """The bottom-right corner of the output, away from every target region."""
    output = wlr.outputs(ctx.tier)[0]
    width, height = logical_size(output)
    return width - 30, height - 30


def _start(ctx: LegContext, output: wlr.Output) -> str:
    session_id = start_session(ctx, cursor=True)
    move(ctx, session_id, park_point(ctx))
    return session_id


@contextlib.contextmanager
def _session(ctx: LegContext, output: wlr.Output):
    session_id = _start(ctx, output)
    try:
        yield session_id
    finally:
        end_session(ctx, session_id)


def _calculator_aligned(ctx: LegContext, fractional: bool) -> None:
    with output_scale(ctx, fractional) as output:
        app, root, read = _calculator(ctx)
        try:
            maximize_and_focus(ctx, app)
            frame = next(node for node in root.children() if node.role in FRAME_ROLES)
            require(raw.poll(lambda: fills_output(frame, output), SETTLE_S),
                    f"the Calculator does not fill the output: {frame.extents(True)}")
            extents = tuple(button(root, "7").extents(True))
            region = region_around(extents, output)
            before = read()
            ctx.before("atspi.display", before)
            with _session(ctx, output) as session_id:
                scale = output.scale
                base = settled(ctx, region)
                seed = tuple(round((extents[index] + SEED_INSET - region[index]) * scale)
                             for index in (0, 1))
                truth = marks.to_layout(marks.fill_box(base, seed), scale, region[:2])
                require(abs(truth[2] - extents[2]) <= SIZE_PX
                        and abs(truth[3] - extents[3]) <= SIZE_PX,
                        f"the 7 button's own fill {truth} is not the size AT-SPI reads {extents}")
                check = Alignment(ctx, session_id, region, scale, truth)
                ctx.before("pixels.outline", check.outline(check.base))
                ctx.before("pixels.dart", len(check.dart(check.base)))
                ref = session_ref(ctx, app, session_id, "7")
                outline, last = check.framed(ref, headed=False)
                check.check_outline(outline)
                clicked = check.dart(last)
                reference = check.reference()
                semantic = check.check_dart(clicked, reference, "the semantic click")
                _, last = check.framed(ref, headed=True)
                headed = check.check_dart(check.dart(last), reference, "the headed click")
                ctx.after("atspi.display", changed(read, "77"))
                ctx.after("pixels.outline", outline)
                ctx.after("pixels.dart", (semantic, headed))
                move(ctx, session_id, check.park)
                disable_and_reap(ctx, session_id)
                quiet = settled(ctx, region)
                ctx.restored("pixels.outline", check.outline(quiet))
                ctx.restored("pixels.dart", len(check.dart(quiet)))
            _clear_calculator(root)
            ctx.restored("atspi.display", changed(read, before))
        finally:
            stop(app)


@implementation("labwc.cursor_overlay_aligned_calculator")
def cursor_overlay_aligned_calculator(ctx: LegContext) -> None:
    _calculator_aligned(ctx, fractional=False)


@implementation("labwc.cursor_overlay_aligned_calculator_fractional")
def cursor_overlay_aligned_calculator_fractional(ctx: LegContext) -> None:
    _calculator_aligned(ctx, fractional=True)


def landing(root, count: int) -> tuple[float, float] | None:
    """Where the page saw its `count`th click, relative to the button's corner."""
    prefix = f"hit {count} "
    for text in headings(root):
        if text.startswith(prefix):
            x, _, y = text[len(prefix):].partition(",")
            return float(x), float(y)
    return None


def far_landing(frame) -> tuple[float, float] | None:
    """Where the far button saw its first click, relative to its corner, from the window title
    its page sets."""
    prefix, name = "far 1 ", frame.name
    if not name.startswith(prefix):
        return None
    x, _, y = name[len(prefix):].partition(" ")[0].partition(",")
    return float(x), float(y)


def far_click(ctx: LegContext, session_id: str, tree, frame) -> str:
    """A headed click on the button in the page's bottom-right corner, which at a fractional
    scale Chromium reports in device pixels beyond the browser controls that hold it, lands on
    its centre by the offsets the page writes into its title."""
    ref = find_ref(tree, name=FAR, role="button")
    require(ref is not None, "the far page button has no ref")
    command(ctx, ["--session", session_id, "click", ref], headed=True)
    hit = raw.poll(lambda: far_landing(frame), SETTLE_S)
    require(hit is not None, f"the far page button saw no trusted click: {frame.name!r}")
    centre = (FAR_SIZE[0] / 2, FAR_SIZE[1] / 2)
    require(max(abs(hit[0] - centre[0]), abs(hit[1] - centre[1])) <= TIP_PX,
            f"the headed click landed {hit} from the far button's corner, its centre is {centre}")
    return frame.name


def _chromium_aligned(ctx: LegContext, fractional: bool) -> None:
    with output_scale(ctx, fractional) as output, \
            tempfile.TemporaryDirectory(prefix="chromium-", dir=ctx.tier.runtime_dir) as directory:
        page = Path(directory) / "page.html"
        page.write_text(PAGE)
        browser = Browser(ctx, page.as_uri(), Path(directory) / "profile")
        try:
            root = browser_root(ctx, browser.child.pid)
            read = lambda: headings(root)
            before = changed(read, (WAITING,), seconds=20)
            ctx.before("atspi.heading", before)
            frame = next(node for node in root.children() if node.role in FRAME_ROLES)
            title = frame.name
            ctx.before("atspi.title", title)
            window = encode_product_id(frame.bus, frame.path)
            command(ctx, ["maximize"], window_id=window)
            command(ctx, ["focus-window", "--window-id", window])
            require(raw.poll(lambda: fills_output(frame, output), SETTLE_S),
                    f"the browser does not fill the output: {frame.extents(True)}")
            whole = (0, 0, *logical_size(output))
            with _session(ctx, output) as session_id:
                scale = output.scale
                found = marks.colour_box(settled(ctx, whole), MAGENTA)
                require(found is not None, "the page button's colour is not on the output")
                truth = marks.to_layout(found, scale)
                region = region_around(tuple(round(value) for value in truth), output)
                check = Alignment(ctx, session_id, region, scale, truth)
                ctx.before("pixels.outline", check.outline(check.base))
                ctx.before("pixels.dart", len(check.dart(check.base)))
                snapshot = command(ctx, ["--session", session_id, "snapshot", "-i",
                                         "--max-depth", "50"], headed=False, window_id=window)
                tree = (snapshot.data or {}).get("tree")
                ref = find_ref(tree, name=TARGET, role="button")
                require(ref is not None, "the page button has no ref")
                outline, last = check.framed(ref, headed=False)
                check.check_outline(outline)
                clicked = check.dart(last)
                reference = check.reference()
                semantic = check.check_dart(clicked, reference, "the semantic click")
                _, last = check.framed(ref, headed=True)
                headed = check.check_dart(check.dart(last), reference, "the headed click")
                hit = raw.poll(lambda: landing(root, 2), SETTLE_S)
                require(hit is not None, f"the page saw no trusted click: {read()}")
                point = (truth[0] + hit[0], truth[1] + hit[1])
                centre = (truth[0] + truth[2] / 2, truth[1] + truth[3] / 2)
                require(max(abs(point[0] - centre[0]), abs(point[1] - centre[1])) <= TIP_PX,
                        f"the headed click landed at {point}, the button's centre is {centre}")
                ctx.after("atspi.heading", read())
                ctx.after("atspi.title", far_click(ctx, session_id, tree, frame))
                ctx.after("pixels.outline", outline)
                ctx.after("pixels.dart", (semantic, headed))
                move(ctx, session_id, check.park)
                disable_and_reap(ctx, session_id)
                quiet = settled(ctx, region)
                ctx.restored("pixels.outline", check.outline(quiet))
                ctx.restored("pixels.dart", len(check.dart(quiet)))
            reload = [node for node in root.walk()
                      if node.role == PUSH_BUTTON and node.name == "Reload"]
            require(len(reload) == 1, f"expected one raw Reload button, found {len(reload)}")
            reload[0].do_action("press") or reload[0].do_action("click")
            ctx.restored("atspi.heading", changed(read, before, seconds=10))
            ctx.restored("atspi.title", changed(lambda: frame.name, title, seconds=10))
        finally:
            browser.stop()


@implementation("labwc.cursor_overlay_aligned_chromium")
def cursor_overlay_aligned_chromium(ctx: LegContext) -> None:
    _chromium_aligned(ctx, fractional=False)


@implementation("labwc.cursor_overlay_aligned_chromium_fractional")
def cursor_overlay_aligned_chromium_fractional(ctx: LegContext) -> None:
    _chromium_aligned(ctx, fractional=True)
