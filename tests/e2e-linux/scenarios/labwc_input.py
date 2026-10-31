"""Input legs on the private labwc tier: keys, headed typing and pointer delivery, each read
back through raw AT-SPI rather than the product's own report."""
from __future__ import annotations

from oracle import atspi_raw as raw, wlr
from runner import session
from runner.apps import launch, stop
from runner.helper import activate, changed, command, display, require
from runner.legs import implementation
from runner.verdict import LegContext, NotApplicable
from scenarios.headed_actions import find_ref

CALCULATOR = "Calculator"
TEXT_EDITOR = "Text Editor"
CALCULATOR_ID = "org.gnome.Calculator"
TEXT_EDITOR_ID = "org.gnome.TextEditor"
SEED = "hello world"
HEADED_TEXT = "Ben from srv! @#$"
FRAME_ROLES = (23, 69)
PUSH_BUTTON = 43
SNAPSHOT_DEPTH = "50"


def placement(identifier: str) -> tuple[int, int]:
    found = wlr.placements(session.LABWC_RC.read_text()).get(identifier)
    require(found is not None, f"the labwc rc places no {identifier} window")
    return found


def snapshot_ref(ctx: LegContext, app, *, role: str | None = None, name: str | None = None) -> str:
    response = command(ctx, ["snapshot", "-i", "--max-depth", SNAPSHOT_DEPTH], app, headed=False)
    ref = find_ref((response.data or {}).get("tree"), name=name, role=role)
    require(ref is not None, f"{app.name} snapshot has no {role}/{name} ref")
    return ref


def frames(root) -> list:
    return [child for child in root.children() if child.role in FRAME_ROLES]


def frame_active(ctx: LegContext, app) -> bool:
    return any("ACTIVE" in frame.states
               for frame in frames(raw.app_root(ctx.tier, app.owner_names[0])))


def document_node(root):
    nodes = [node for node in root.walk()
             if "org.a11y.atspi.EditableText" in node.interfaces() and "SHOWING" in node.states]
    require(len(nodes) == 1, f"expected one showing document node, found {len(nodes)}")
    return nodes[0]


def activate_by_pointer(ctx: LegContext, app, identifier: str, node) -> None:
    """Make a placed window active with one headed click on `node`. labwc activates no
    window on map until its seat has seen input, and a fresh headless seat has none."""
    if frame_active(ctx, app):
        return
    point = wlr.placed_point(placement(identifier), node.extents(True))
    command(ctx, ["mouse-click", "--xy", f"{point[0]},{point[1]}"], headed=True)
    require(raw.poll(lambda: frame_active(ctx, app), 5.0),
            f"{app.name} did not become the active window after a click inside it")


def document_text(ctx: LegContext, app) -> str:
    return document_node(raw.app_root(ctx.tier, app.owner_names[0])).text() or ""


def button(root, name: str):
    matches = [node for node in root.walk() if node.role == PUSH_BUTTON and node.name == name]
    require(len(matches) == 1, f"expected one raw {name!r} button, found {len(matches)}")
    return matches[0]


def _calculator(ctx: LegContext):
    app = launch(ctx.tier, CALCULATOR)
    root = raw.app_root(ctx.tier, app.owner_names[0])
    return app, root, lambda: display(root)


def _clear_calculator(root) -> None:
    activate(button(root, "C"))


@implementation("labwc.press_modifiers_replace_text")
def press_modifiers_replace_text(ctx: LegContext) -> None:
    app = launch(ctx.tier, TEXT_EDITOR)
    try:
        ref = snapshot_ref(ctx, app, role="textfield")
        read = lambda: document_text(ctx, app)
        command(ctx, ["clear", ref])
        command(ctx, ["type", ref, SEED])
        before = changed(read, SEED)
        ctx.before("atspi.document_text", before)
        activate_by_pointer(ctx, app, TEXT_EDITOR_ID,
                            document_node(raw.app_root(ctx.tier, app.owner_names[0])))
        command(ctx, ["press", "ctrl+a"], app)
        command(ctx, ["press", "shift+b"], app)
        ctx.after("atspi.document_text", changed(read, "B", seconds=4))
        command(ctx, ["clear", ref])
        command(ctx, ["type", ref, SEED])
        ctx.restored("atspi.document_text", changed(read, before))
        command(ctx, ["clear", ref])
    finally:
        stop(app)


@implementation("labwc.headed_type_exact_text")
def headed_type_exact_text(ctx: LegContext) -> None:
    app = launch(ctx.tier, TEXT_EDITOR)
    try:
        ref = snapshot_ref(ctx, app, role="textfield")
        read = lambda: document_text(ctx, app)
        command(ctx, ["clear", ref], headed=False)
        before = changed(read, "")
        ctx.before("atspi.document_text", before)
        command(ctx, ["type", ref, HEADED_TEXT])
        ctx.after("atspi.document_text", changed(read, HEADED_TEXT, seconds=5))
        command(ctx, ["clear", ref], headed=False)
        ctx.restored("atspi.document_text", changed(read, before))
    finally:
        stop(app)


def capture_offered(ctx: LegContext) -> bool:
    """Whether the compositor can capture one toplevel, so a window that is not maximized can
    be placed from its own pixels."""
    return wlr.TOPLEVEL_CAPTURE in wlr.globals_of(ctx.tier)


def maximize_and_focus(ctx: LegContext, app) -> None:
    command(ctx, ["maximize"], window_id=app.windows[0])
    command(ctx, ["focus-window", "--window-id", app.windows[0]])


@implementation("labwc.headed_click_calculator_maximized")
def headed_click_calculator_maximized(ctx: LegContext) -> None:
    app, root, read = _calculator(ctx)
    try:
        before = read()
        ctx.before("atspi.display", before)
        maximize_and_focus(ctx, app)
        ref = snapshot_ref(ctx, app, role="button", name="7")
        command(ctx, ["click", ref])
        ctx.after("atspi.display", changed(read, "7"))
        _clear_calculator(root)
        command(ctx, ["restore"], window_id=app.windows[0])
        ctx.restored("atspi.display", changed(read, before))
    finally:
        stop(app)


@implementation("labwc.headed_click_calculator_pixel_route")
def headed_click_calculator_pixel_route(ctx: LegContext) -> None:
    if not capture_offered(ctx):
        raise NotApplicable(f"the compositor advertises no {wlr.TOPLEVEL_CAPTURE}, so no "
                            "window is placed from its pixels; labwc.headed_click_unplaced_refused "
                            "asserts the refusal instead")
    app, root, read = _calculator(ctx)
    try:
        before = read()
        ctx.before("atspi.display", before)
        ref = snapshot_ref(ctx, app, role="button", name="7")
        command(ctx, ["click", ref])
        ctx.after("atspi.display", changed(read, "7"))
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
    finally:
        stop(app)


@implementation("labwc.headed_click_unplaced_refused")
def headed_click_unplaced_refused(ctx: LegContext) -> None:
    if capture_offered(ctx):
        raise NotApplicable(f"the compositor advertises {wlr.TOPLEVEL_CAPTURE}, so the window is "
                            "placed from its pixels; labwc.headed_click_calculator_pixel_route "
                            "asserts the click instead")
    app, root, read = _calculator(ctx)
    try:
        before = read()
        ctx.before("atspi.display", before)
        ref = snapshot_ref(ctx, app, role="button", name="7")
        error = command(ctx, ["click", ref], code="ACTION_FAILED").error or {}
        details = error.get("details") or {}
        require(details.get("kind") == "wlroots_window_origin_unproven", f"refusal: {error}")
        require(details.get("physical_delivery_started") is False, f"refusal: {error}")
        require((error.get("disposition") or {}).get("delivery") == "not_delivered",
                f"refusal: {error}")
        require(f"maximize --window-id {details.get('window_id')}" in error.get("suggestion", ""),
                f"refusal names no maximize: {error}")
        require(not raw.poll(lambda: read() != before, 1.0),
                f"the display changed after a refused click: {read()!r}")
        ctx.after("atspi.display", read())
        ctx.restored("atspi.display", read())
    finally:
        stop(app)


@implementation("labwc.mouse_click_calculator_digit")
def mouse_click_calculator_digit(ctx: LegContext) -> None:
    app, root, read = _calculator(ctx)
    try:
        before = read()
        ctx.before("atspi.display", before)
        point = wlr.placed_point(placement(CALCULATOR_ID), button(root, "8").extents(True))
        command(ctx, ["mouse-click", "--xy", f"{point[0]},{point[1]}"])
        ctx.after("atspi.display", changed(read, "8"))
        _clear_calculator(root)
        ctx.restored("atspi.display", changed(read, before))
    finally:
        stop(app)
