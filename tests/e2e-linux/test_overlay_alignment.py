"""Portable checks for the overlay alignment oracle: an element's own fill and colour, the
outline's framed rectangle at scale 1 and at a fractional scale, and the dart's offset from a
reference dart, each catching the twelve-pixel offset GTK 4 stack pages caused."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import scenarios  # noqa: F401  (registers every implementation)
from oracle import overlay_marks as marks, pixels
from runner import legs
from scenarios import labwc_alignment

GREY = (250, 250, 250)
BUTTON = (211, 211, 211)
INK = (40, 40, 40)
ACCENT = (66, 153, 255)
MAGENTA = (255, 0, 255)


class Canvas:
    def __init__(self, width: int, height: int, colour=GREY):
        self.width, self.height = width, height
        self.rgb = bytearray(bytes(colour) * (width * height))

    def paint(self, box, colour) -> "Canvas":
        left, top, right, bottom = box
        for y in range(max(top, 0), min(bottom, self.height)):
            for x in range(max(left, 0), min(right, self.width)):
                offset = (y * self.width + x) * 3
                self.rgb[offset:offset + 3] = bytes(colour)
        return self

    def image(self) -> pixels.Image:
        return pixels.Image(width=self.width, height=self.height, rgb=bytes(self.rgb))


def blend(colour, other, share: float):
    return tuple(round(a * share + b * (1 - share)) for a, b in zip(colour, other))


def outlined(canvas: Canvas, target, scale: float) -> Canvas:
    """An outline as the overlay draws it, rasterised at `scale`: the stroke runs from the
    padded edge inward, full where it covers a pixel and blended where it covers part."""
    x, y, width, height = target
    outer = (x - 5, y - 5, x + width + 5, y + height + 5)
    stroke = 2.5
    for row in range(canvas.height):
        for column in range(canvas.width):
            cover = 0.0
            for sub_y in (0.25, 0.75):
                for sub_x in (0.25, 0.75):
                    lx, ly = (column + sub_x) / scale, (row + sub_y) / scale
                    inside = outer[0] <= lx < outer[2] and outer[1] <= ly < outer[3]
                    hollow = (outer[0] + stroke <= lx < outer[2] - stroke
                              and outer[1] + stroke <= ly < outer[3] - stroke)
                    cover += 0.25 if inside and not hollow else 0.0
            if cover:
                canvas.paint((column, row, column + 1, row + 1),
                             blend(ACCENT, GREY, cover))
    return canvas


def dart(canvas: Canvas, tip, scale: float) -> Canvas:
    """A stand-in dart: a dark rim box and a white body below and right of the tip."""
    x, y = round(tip[0] * scale), round(tip[1] * scale)
    canvas.paint((x - round(3 * scale), y - round(3 * scale), x + round(20 * scale),
                  y + round(30 * scale)), (17, 19, 24))
    canvas.paint((x, y, x + round(14 * scale), y + round(24 * scale)), (255, 255, 255))
    return canvas


class FillTests(unittest.TestCase):
    def test_the_fill_around_a_seed_is_the_button_with_its_label_a_hole(self):
        canvas = Canvas(200, 120).paint((40, 30, 172, 74), BUTTON).paint((100, 40, 110, 64), INK)
        self.assertEqual(marks.fill_box(canvas.image(), (46, 36)), (40, 30, 172, 74))

    def test_a_unique_colour_is_found_wherever_it_is_painted(self):
        canvas = Canvas(300, 200).paint((60, 50, 300, 140), MAGENTA)
        self.assertEqual(marks.colour_box(canvas.image(), MAGENTA), (60, 50, 300, 140))
        self.assertIsNone(marks.colour_box(Canvas(10, 10).image(), MAGENTA))

    def test_buffer_pixels_become_layout_units_at_the_capture_origin(self):
        self.assertEqual(marks.to_layout((16, 32, 176, 96), 1.6, (100, 200)),
                         (110.0, 220.0, 100.0, 40.0))


class OutlineTests(unittest.TestCase):
    def _framed(self, target, drawn, scale: float, size=(260, 160)):
        base = Canvas(*size).paint(tuple(round(v * scale) for v in (60, 60, 192, 104)), BUTTON)
        canvas = Canvas(*size).paint(tuple(round(v * scale) for v in (60, 60, 192, 104)),
                                     BUTTON)
        outlined(canvas, drawn, scale)
        return marks.outline_target(canvas.image(), base.image(), scale)

    def test_an_outline_at_scale_one_frames_the_target_within_a_quarter_pixel(self):
        target = (60, 60, 132, 44)
        found = self._framed(target, target, 1.0)
        self.assertTrue(marks.within(found, target, 0.3), found)

    def test_an_outline_at_amps_fractional_scale_frames_the_target_within_a_pixel(self):
        target = (60, 60, 132, 44)
        scale = 1.601562
        found = self._framed(target, target, scale, size=(420, 260))
        self.assertTrue(marks.within(found, target, 1.0), found)

    def test_an_outline_twelve_pixels_off_is_caught(self):
        target = (60, 60, 132, 44)
        for scale, size in ((1.0, (260, 160)), (1.601562, (420, 260))):
            found = self._framed(target, (72, 72, 132, 44), scale, size)
            self.assertIsNotNone(found)
            self.assertFalse(marks.within(found, target, labwc_alignment.EDGE_PX), found)

    def test_a_solid_ripple_core_inside_the_outline_is_not_read_as_an_edge(self):
        base = Canvas(260, 160)
        canvas = outlined(Canvas(260, 160), (60, 60, 132, 44), 1.0)
        canvas.paint((107, 63, 145, 101), ACCENT)
        self.assertIsNone(marks.outline_target(canvas.image(), base.image(), 1.0))

    def test_nothing_drawn_is_no_outline(self):
        image = Canvas(100, 100).image()
        self.assertIsNone(marks.outline_target(image, image, 1.0))


class DartTests(unittest.TestCase):
    def test_a_dart_on_the_reference_point_has_no_offset(self):
        base = Canvas(200, 200).image()
        box = (0, 0, 200, 200)
        found = marks.dark_marks(dart(Canvas(200, 200), (60, 60), 1.0).image(), base, box)
        reference = marks.dark_marks(dart(Canvas(200, 200), (60, 60), 1.0).image(), base, box)
        self.assertEqual(marks.mark_shift(found, reference, 1.0), (0.0, 0.0))

    def test_a_dart_twelve_pixels_off_reads_twelve_layout_pixels_at_any_scale(self):
        for scale in (1.0, 1.601562):
            size = round(200 * scale)
            base = Canvas(size, size).image()
            box = (0, 0, size, size)
            found = marks.dark_marks(dart(Canvas(size, size), (72, 72), scale).image(), base, box)
            reference = marks.dark_marks(dart(Canvas(size, size), (60, 60), scale).image(), base,
                                         box)
            shift = marks.mark_shift(found, reference, scale)
            self.assertTrue(all(abs(value - 12) <= 0.7 for value in shift), (scale, shift))
            self.assertGreater(max(map(abs, shift)), labwc_alignment.TIP_PX)

    def test_dark_content_already_on_the_output_is_not_the_dart(self):
        base = Canvas(100, 100).paint((10, 10, 30, 30), INK)
        image = Canvas(100, 100).paint((10, 10, 30, 30), INK)
        self.assertEqual(marks.dark_marks(image.image(), base.image(), (0, 0, 100, 100)), [])
        self.assertIsNone(marks.mark_shift([], [], 1.0))


class AlignmentLegTests(unittest.TestCase):
    def test_every_alignment_leg_is_registered_headed_on_labwc_with_an_implementation(self):
        implemented = legs.implementations()
        for name in ("calculator", "calculator_fractional", "chromium", "chromium_fractional"):
            leg = legs.leg_by_id(f"labwc.cursor_overlay_aligned_{name}")
            self.assertEqual(leg.tiers, ("wayland-labwc",))
            self.assertEqual(leg.modes, ("headed",))
            self.assertIn(leg.id, implemented)

    def test_the_fractional_mode_is_amps_desktop(self):
        self.assertEqual(labwc_alignment.FRACTIONAL_MODE, "928x1424@30Hz")
        self.assertEqual(round(float(labwc_alignment.FRACTIONAL_SCALE) * 256), 410)

    def test_the_capture_region_is_clipped_to_the_logical_output(self):
        output = SimpleOutput(928, 1424, 1.601562)
        self.assertEqual(labwc_alignment.region_around((24, 701, 108, 44), output),
                         (0, 641, 192, 164))
        self.assertEqual(labwc_alignment.region_around((500, 850, 70, 30), output),
                         (440, 790, 139, 99))

    def test_the_dart_box_is_in_the_region_capture_buffer_pixels(self):
        self.assertEqual(labwc_alignment.dart_box((66, 712), (0, 641, 192, 164), 1.6),
                         (74, 82, 202, 216))


class SimpleOutput:
    def __init__(self, width: int, height: int, scale: float):
        self.width, self.height, self.scale = width, height, scale


if __name__ == "__main__":
    unittest.main()
