"""Display, capture and window legs on the private labwc tier. Outputs and pixels are read
through wlr-randr and grim; windows through raw AT-SPI frames and process state."""
from __future__ import annotations

import tempfile
import time
from pathlib import Path

from oracle import atspi_raw as raw, pixels, wlr
from oracle.proc import stat_fields
from oracle.x11 import encode_product_id
from runner import apps
from runner.apps import launch, stop
from runner.helper import changed, command, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.labwc_input import (CALCULATOR, CALCULATOR_ID, TEXT_EDITOR, TEXT_EDITOR_ID,
                                   activate_by_pointer, document_node, frames, placement)

PIXEL_TOLERANCE = 0.02
SETTLE_S = 1.0
MOVE_TARGET = ("--x", "300", "--y", "120")


def output_facts(outputs: list[wlr.Output]) -> list[tuple]:
    return [(item.name, item.x, item.y, item.width, item.height, item.scale) for item in outputs]


def display_facts(displays: list[dict]) -> list[tuple]:
    result = []
    for item in displays:
        bounds = item.get("bounds") or {}
        result.append((item.get("id"), int(bounds.get("x", -1)), int(bounds.get("y", -1)),
                       int(bounds.get("width", -1)), int(bounds.get("height", -1)),
                       float(item.get("scale", 0.0))))
    return result


def frame_facts(ctx: LegContext, owned) -> list[tuple[str, str]]:
    result = []
    for app in owned:
        for frame in frames(raw.app_root(ctx.tier, app.owner_names[0])):
            result.append((encode_product_id(frame.bus, frame.path), frame.name))
    return sorted(result)


def listed_facts(windows: list[dict], pids: set[int]) -> list[tuple[str, str]]:
    return sorted((item["id"], item["title"]) for item in windows if item.get("pid") in pids)


def active_facts(ctx: LegContext, owned) -> tuple[tuple[str, bool], ...]:
    return tuple((app.name, any("ACTIVE" in frame.states
                                for frame in frames(raw.app_root(ctx.tier, app.owner_names[0]))))
                 for app in owned)


def running(pid: int, start: int, proc_root: Path = Path("/proc")) -> bool:
    try:
        text = (proc_root / str(pid) / "stat").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return False
    state = text[text.rindex(")") + 2:].split()[0]
    return state != "Z" and stat_fields(text)[0] == start


def window_region(origin: tuple[int, int], extents: tuple[int, int, int, int],
                  output: wlr.Output) -> tuple[int, int, int, int]:
    """The output rectangle a window placed at `origin` covers, clipped to the output."""
    left, top = max(origin[0], 0), max(origin[1], 0)
    right = min(origin[0] + extents[2], output.width)
    bottom = min(origin[1] + extents[3], output.height)
    if right <= left or bottom <= top:
        raise ValueError("the window lies outside the output")
    return left, top, right - left, bottom - top


def _single_output(ctx: LegContext) -> wlr.Output:
    outputs = wlr.outputs(ctx.tier)
    require(len(outputs) == 1, f"expected one labwc output, found {output_facts(outputs)}")
    return outputs[0]


@implementation("labwc.list_displays_matches_output")
def list_displays_matches_output(ctx: LegContext) -> None:
    read = lambda: output_facts(wlr.outputs(ctx.tier))
    before = read()
    ctx.before("wlr.outputs", before)
    require(len(before) == 1, f"expected one labwc output, found {before}")
    response = command(ctx, ["list-displays"])
    require(display_facts(response.data or []) == before,
            f"list-displays {response.data} disagrees with wlr-randr {before}")
    ctx.after("wlr.outputs", read())
    ctx.restored("wlr.outputs", read())


@implementation("labwc.screenshot_matches_grim")
def screenshot_matches_grim(ctx: LegContext) -> None:
    app = launch(ctx.tier, CALCULATOR)
    try:
        output = _single_output(ctx)
        read = lambda: output_facts(wlr.outputs(ctx.tier))
        ctx.before("wlr.outputs", read())
        ctx.before("pixels.screenshot", None)
        with tempfile.TemporaryDirectory(prefix="shot-", dir=ctx.tier.runtime_dir) as directory:
            path = Path(directory) / "screen.png"
            command(ctx, ["screenshot", "--screen", "0", str(path)])
            shot = pixels.decode_png(path.read_bytes())
        reference = wlr.capture(ctx.tier, output.name)
        mismatch = (pixels.mismatch_fraction(shot, reference)
                    if (shot.width, shot.height) == (reference.width, reference.height) else 1.0)
        ctx.after("pixels.screenshot", {"size": (shot.width, shot.height),
                                        "mismatch": round(mismatch, 4)})
        ctx.after("wlr.outputs", read())
        require((shot.width, shot.height) == (output.width, output.height),
                f"screenshot is {shot.width}x{shot.height}, the output {output.width}x{output.height}")
        require(mismatch <= PIXEL_TOLERANCE, f"screenshot differs from grim in {mismatch:.2%} of pixels")
        ctx.restored("pixels.screenshot", None)
        ctx.restored("wlr.outputs", read())
    finally:
        stop(app)


@implementation("labwc.list_windows_matches_atspi_frames")
def list_windows_matches_atspi_frames(ctx: LegContext) -> None:
    calculator = launch(ctx.tier, CALCULATOR)
    editor = None
    try:
        editor = launch(ctx.tier, TEXT_EDITOR)
        owned = (calculator, editor)
        read = lambda: frame_facts(ctx, owned)
        before = read()
        ctx.before("atspi.frames", before)
        require(len(before) == 2, f"expected one frame per app, found {before}")
        response = command(ctx, ["list-windows"])
        listed = listed_facts(response.data or [], {calculator.pid, editor.pid})
        require(listed == before, f"list-windows {listed} disagrees with AT-SPI frames {before}")
        ctx.after("atspi.frames", read())
        ctx.restored("atspi.frames", read())
    finally:
        if editor is not None:
            stop(editor)
        stop(calculator)


@implementation("labwc.focus_window_moves_active")
def focus_window_moves_active(ctx: LegContext) -> None:
    calculator = launch(ctx.tier, CALCULATOR)
    editor = None
    try:
        editor = launch(ctx.tier, TEXT_EDITOR)
        owned = (calculator, editor)
        read = lambda: active_facts(ctx, owned)
        activate_by_pointer(ctx, editor, TEXT_EDITOR_ID,
                            document_node(raw.app_root(ctx.tier, editor.owner_names[0])))
        before = changed(read, ((calculator.name, False), (editor.name, True)), seconds=5)
        ctx.before("atspi.active", before)
        ids = dict(frame_facts(ctx, (calculator,))), dict(frame_facts(ctx, (editor,)))
        require(all(len(item) == 1 for item in ids), f"expected one frame per app: {ids}")
        command(ctx, ["focus-window", "--window-id", next(iter(ids[0]))])
        ctx.after("atspi.active", changed(read, ((calculator.name, True), (editor.name, False)),
                                          seconds=5))
        command(ctx, ["focus-window", "--window-id", next(iter(ids[1]))])
        ctx.restored("atspi.active", changed(read, before, seconds=5))
    finally:
        if editor is not None:
            stop(editor)
        stop(calculator)


@implementation("labwc.close_app_graceful")
def close_app_graceful(ctx: LegContext) -> None:
    app = launch(ctx.tier, CALCULATOR)
    replacement = None
    try:
        child = apps._SPAWNED.get(app.pid)
        require(child is not None, "the Calculator process was not started by this leg")
        alive = lambda: running(app.pid, app.start_time)
        ctx.before("proc.calculator", {"running": alive(), "exit": None})
        ctx.before("atspi.calculator_frames", len(frame_facts(ctx, (app,))))
        command(ctx, ["close-app", app.name])
        require(raw.poll(lambda: not alive(), 10.0), "Calculator is still running after close-app")
        status = child.wait(timeout=5)
        ctx.after("proc.calculator", {"running": alive(), "exit": status})
        require(status == 0, f"Calculator exited with {status}, not a graceful quit")
        ctx.after("atspi.calculator_frames", sum(
            1 for node in raw.registry(ctx.tier) if node.bus == app.owner_names[0]))
        replacement = launch(ctx.tier, CALCULATOR)
        ctx.restored("proc.calculator", {
            "running": running(replacement.pid, replacement.start_time), "exit": None})
        ctx.restored("atspi.calculator_frames", len(frame_facts(ctx, (replacement,))))
    finally:
        if replacement is not None:
            stop(replacement)
        stop(app)


@implementation("labwc.move_window_refused")
def move_window_refused(ctx: LegContext) -> None:
    app = launch(ctx.tier, CALCULATOR)
    try:
        output = _single_output(ctx)
        frame = frames(raw.app_root(ctx.tier, app.owner_names[0]))
        require(len(frame) == 1, f"expected one Calculator frame, found {len(frame)}")
        region = window_region(placement(CALCULATOR_ID), frame[0].extents(True), output)
        baseline = wlr.capture(ctx.tier, output.name)
        unchanged = lambda: pixels.similar(baseline, wlr.capture(ctx.tier, output.name), region,
                                           tolerance=PIXEL_TOLERANCE)
        read = lambda: output_facts(wlr.outputs(ctx.tier))
        ctx.before("wlr.outputs", read())
        ctx.before("pixels.window_unchanged", unchanged())
        response = command(ctx, ["move-window", *MOVE_TARGET], app,
                           code="ACTION_NOT_SUPPORTED", window_id=app.windows[0])
        delivery = ((response.error or {}).get("disposition") or {}).get("delivery")
        require(delivery == "not_delivered", f"refusal reports delivery {delivery!r}")
        time.sleep(SETTLE_S)
        after = unchanged()
        ctx.after("pixels.window_unchanged", after)
        ctx.after("wlr.outputs", read())
        require(after, "the pixels at the placed Calculator window changed after the refused move")
        ctx.restored("pixels.window_unchanged", unchanged())
        ctx.restored("wlr.outputs", read())
    finally:
        stop(app)
