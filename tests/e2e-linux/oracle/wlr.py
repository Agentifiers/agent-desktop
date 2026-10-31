"""Compositor-side reads for the private labwc tier: outputs through wlr-randr, output
pixels through grim, and the window placement the tier's labwc rc.xml imposes."""
from __future__ import annotations

import json
import tempfile
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from pathlib import Path

from oracle import pixels
from oracle.gsettings import tool
from runner.session import Tier


@dataclass(frozen=True)
class Output:
    name: str
    x: int
    y: int
    width: int
    height: int
    scale: float
    refresh: float


def parse_outputs(text: str) -> list[Output]:
    result = []
    for entry in json.loads(text):
        if not entry.get("enabled"):
            continue
        current = [mode for mode in entry.get("modes", []) if mode.get("current")]
        if len(current) != 1:
            raise ValueError(f"output {entry.get('name')!r} has no single current mode")
        mode = current[0]
        position = entry.get("position") or {}
        result.append(Output(
            name=entry["name"], x=int(position.get("x", 0)), y=int(position.get("y", 0)),
            width=int(mode["width"]), height=int(mode["height"]),
            scale=float(entry.get("scale", 1.0)), refresh=float(mode.get("refresh", 0.0)),
        ))
    return result


def outputs(tier: Tier) -> list[Output]:
    return parse_outputs(tool(tier, ["wlr-randr", "--json"]))


def capture(tier: Tier, output: str) -> pixels.Image:
    with tempfile.TemporaryDirectory(prefix="grim-", dir=tier.runtime_dir) as directory:
        path = Path(directory) / "output.png"
        tool(tier, ["grim", "-o", output, str(path)], timeout_s=10.0)
        return pixels.decode_png(path.read_bytes())


def placements(rc_xml: str) -> dict[str, tuple[int, int]]:
    root = ElementTree.fromstring(rc_xml)
    result = {}
    for rule in root.iter("windowRule"):
        identifier = rule.get("identifier")
        for action in rule.iter("action"):
            if identifier and action.get("name") == "MoveTo":
                result[identifier] = (int(action.get("x")), int(action.get("y")))
    return result


def placed_point(origin: tuple[int, int], extents: tuple[int, int, int, int]) -> tuple[int, int]:
    """The output point at the centre of window-relative AT-SPI extents of a window whose
    geometry the compositor placed at `origin`."""
    x, y, width, height = extents
    if width <= 0 or height <= 0:
        raise ValueError("extents must have a positive size")
    return origin[0] + x + width // 2, origin[1] + y + height // 2


TOPLEVEL_CAPTURE = "ext_foreign_toplevel_image_capture_source_manager_v1"


def parse_globals(text: str) -> set[str]:
    """The interface names `wayland-info` lists as the compositor's globals."""
    found = set()
    for line in text.splitlines():
        _, sep, rest = line.partition("interface: '")
        if sep:
            found.add(rest.split("'", 1)[0])
    return found


def globals_of(tier: Tier) -> set[str]:
    return parse_globals(tool(tier, ["wayland-info"], timeout_s=10.0))
