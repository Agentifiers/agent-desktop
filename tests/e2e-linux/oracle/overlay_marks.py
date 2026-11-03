"""Where an element is drawn and where the cursor overlay drew its target outline and its dart,
read from output pixels alone. Captures are compositor buffer pixels: a rectangle is
(left, top, right, bottom) in buffer pixels until `to_layout` divides it by the output scale and
moves it to the layout position the capture started at."""
from __future__ import annotations

from collections import deque

from oracle import pixels

FILL_TOLERANCE = 6
DARK = 80
OUTLINE_PADDING = 5.0
OUTLINE_STROKE = 2.5
LONG_LINE = 0.6

Box = tuple[int, int, int, int]
Rect = tuple[float, float, float, float]


def close(colour: tuple[int, int, int], other: tuple[int, int, int], tolerance: int) -> bool:
    return all(abs(a - b) <= tolerance for a, b in zip(colour, other))


def fill_box(image: pixels.Image, seed: tuple[int, int],
             tolerance: int = FILL_TOLERANCE) -> Box:
    """The bounding box of the 4-connected region of the seed pixel's colour: an element's own
    fill, with its label a hole inside it."""
    colour = image.pixel(*seed)
    seen, queue = {seed}, deque([seed])
    left, top, right, bottom = seed[0], seed[1], seed[0], seed[1]
    while queue:
        x, y = queue.popleft()
        left, top, right, bottom = min(left, x), min(top, y), max(right, x), max(bottom, y)
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if ((nx, ny) not in seen and 0 <= nx < image.width and 0 <= ny < image.height
                    and close(image.pixel(nx, ny), colour, tolerance)):
                seen.add((nx, ny))
                queue.append((nx, ny))
    return left, top, right + 1, bottom + 1


def colour_box(image: pixels.Image, colour: tuple[int, int, int],
               tolerance: int = FILL_TOLERANCE) -> Box | None:
    """The bounding box of every pixel of `colour`, for an element painted in a colour nothing
    else on the output shows."""
    found = [(x, y) for y in range(image.height) for x in range(image.width)
             if close(image.pixel(x, y), colour, tolerance)]
    if not found:
        return None
    xs, ys = [x for x, _ in found], [y for _, y in found]
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def to_layout(box: Box | Rect, scale: float, origin: tuple[float, float] = (0, 0)) -> Rect:
    """A buffer-pixel box as a layout rectangle (x, y, width, height)."""
    left, top, right, bottom = box
    return (origin[0] + left / scale, origin[1] + top / scale, (right - left) / scale,
            (bottom - top) / scale)


def accent(colour: tuple[int, int, int]) -> bool:
    """A strongly accent-blue pixel: the outline's stroke, not a light ripple wash."""
    red, green, blue = colour
    return blue > 200 and blue - red > 120 and green < 190


def _bands(counts: dict[int, int]) -> list[tuple[int, int]]:
    if not counts:
        return []
    floor = LONG_LINE * max(counts.values())
    lines = sorted(index for index, count in counts.items() if count >= floor)
    bands = [[lines[0], lines[0]]]
    for index in lines[1:]:
        if index == bands[-1][1] + 1:
            bands[-1][1] = index
        else:
            bands.append([index, index])
    return [(first, last) for first, last in bands]


def outline_target(image: pixels.Image, base: pixels.Image, scale: float,
                   origin: tuple[float, float] = (0, 0)) -> Rect | None:
    """The target rectangle an outline frames, as a layout rectangle for a capture that starts
    at layout position `origin`: the stroke's two
    horizontal and two vertical bands of accent pixels the overlay added over `base`, each
    read at its middle so blur at a fractional scale moves neither edge. The outline's outer
    edge lies `OUTLINE_PADDING` outside the target and its stroke is `OUTLINE_STROKE` wide.
    `None` unless exactly two bands run each way."""
    rows: dict[int, int] = {}
    columns: dict[int, int] = {}
    for y in range(image.height):
        for x in range(image.width):
            colour = image.pixel(x, y)
            if colour != base.pixel(x, y) and accent(colour):
                rows[y] = rows.get(y, 0) + 1
                columns[x] = columns.get(x, 0) + 1
    horizontal, vertical = _bands(rows), _bands(columns)
    if len(horizontal) != 2 or len(vertical) != 2:
        return None
    middle = lambda band: (band[0] + band[1] + 1) / 2 / scale
    inset = OUTLINE_PADDING - OUTLINE_STROKE / 2
    left = origin[0] + middle(vertical[0]) + inset
    right = origin[0] + middle(vertical[1]) - inset
    top = origin[1] + middle(horizontal[0]) + inset
    bottom = origin[1] + middle(horizontal[1]) - inset
    return left, top, right - left, bottom - top


def dark_marks(image: pixels.Image, base: pixels.Image, box: Box) -> list[tuple[int, int]]:
    """Dark pixels inside `box` that `base` does not show dark: the dart's rim over whatever
    lies beneath it."""
    left, top, right, bottom = box
    return [(x, y) for y in range(max(top, 0), min(bottom, image.height))
            for x in range(max(left, 0), min(right, image.width))
            if max(image.pixel(x, y)) < DARK and max(base.pixel(x, y)) >= DARK]


def centroid(points: list[tuple[int, int]]) -> tuple[float, float] | None:
    if not points:
        return None
    return (sum(x for x, _ in points) / len(points), sum(y for _, y in points) / len(points))


def mark_shift(marks: list[tuple[int, int]], reference: list[tuple[int, int]],
               scale: float) -> tuple[float, float] | None:
    """How far, in layout units, one dart is drawn from a reference dart drawn at a known
    point: the same shape, so the offset of their centroids."""
    found, known = centroid(marks), centroid(reference)
    if found is None or known is None:
        return None
    return (found[0] - known[0]) / scale, (found[1] - known[1]) / scale


def within(found: Rect, truth: Rect, tolerance: float) -> bool:
    """Whether every edge of `found` lies within `tolerance` layout units of `truth`'s."""
    edges = lambda rect: (rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3])
    return all(abs(a - b) <= tolerance for a, b in zip(edges(found), edges(truth)))
