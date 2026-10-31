"""Portable checks for the private labwc tier: its gate, session argv, compositor oracle
parsing, placement math, browser ownership, and the key and click-ripple legs' honest/lying
verdicts."""
from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import scenarios  # noqa: F401  (registers every implementation)
from oracle import wlr
from runner import gate, legs, main, session, verdict
from runner.product import Envelope
from runner.verdict import LegContext
from scenarios import labwc_browser, labwc_input, labwc_overlay, labwc_windows

TIER = "wayland-labwc"
OUTPUT_JSON = json.dumps([
    {"name": "HEADLESS-1", "enabled": True, "position": {"x": 0, "y": 0}, "scale": 1.0,
     "modes": [{"width": 1920, "height": 1080, "refresh": 60.0, "current": False},
               {"width": 1280, "height": 720, "refresh": 60.0, "current": True}]},
    {"name": "HEADLESS-2", "enabled": False, "modes": []},
])


def _gate(root: Path, which, *, libexec=()):
    binary = root / "agent-desktop"
    binary.write_text("x")
    env = {"AGENT_DESKTOP_SESSION_BASE": str(root / "sessions"), "AGENT_DESKTOP_E2E_BIN": str(binary)}
    return gate.check_prerequisites((TIER,), which=which, env=env, extra_paths=(),
                                    gi_probe=lambda: None, platform="linux",
                                    libexec_dirs=tuple(libexec))


class GateTests(unittest.TestCase):
    def test_every_tool_found_passes_without_xvfb(self):
        with tempfile.TemporaryDirectory() as directory:
            which = lambda tool: None if tool in ("Xvfb", "xwd") else f"/usr/bin/{tool}"
            self.assertEqual(_gate(Path(directory), which), [])

    def test_each_missing_tier_tool_is_named(self):
        for tool in gate.TIER_TOOLS[TIER]:
            with self.subTest(tool=tool), tempfile.TemporaryDirectory() as directory:
                which = lambda name, tool=tool: None if name == tool else f"/usr/bin/{name}"
                self.assertEqual(_gate(Path(directory), which),
                                 [f"missing tool for tier {TIER}: {tool}"])

    def test_any_browser_satisfies_the_tier_and_none_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            for present in session.BROWSERS:
                with self.subTest(present=present):
                    which = lambda name, present=present: (
                        None if name in session.BROWSERS and name != present else f"/usr/bin/{name}")
                    self.assertEqual(_gate(Path(directory), which), [])
            which = lambda name: None if name in session.BROWSERS else f"/usr/bin/{name}"
            missing = _gate(Path(directory), which)
        self.assertEqual(missing, [f"missing tool for tier {TIER}: one of chromium, google-chrome, "
                                   "google-chrome-stable"])

    def test_the_registry_daemon_is_found_in_a_libexec_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            libexec = Path(directory) / "libexec"
            libexec.mkdir()
            daemon = libexec / "at-spi2-registryd"
            daemon.write_text("#!/bin/sh\n")
            daemon.chmod(0o700)
            which = lambda name: None if name == "at-spi2-registryd" else f"/usr/bin/{name}"
            self.assertEqual(_gate(Path(directory), which, libexec=(libexec,)), [])
            self.assertEqual(_gate(Path(directory), which),
                             [f"missing tool for tier {TIER}: at-spi2-registryd"])

    def test_either_fusermount_satisfies_the_base(self):
        with tempfile.TemporaryDirectory() as directory:
            which = lambda name: None if name == "fusermount" else f"/usr/bin/{name}"
            self.assertEqual(_gate(Path(directory), which), [])
            which = lambda name: None if name.startswith("fusermount") else f"/usr/bin/{name}"
            self.assertEqual(_gate(Path(directory), which),
                             ["missing tool: one of fusermount, fusermount3"])


class SessionTests(unittest.TestCase):
    def test_tier_argv_runs_labwc_without_xvfb_and_with_the_placement_rules(self):
        argv = session._tier_argv(TIER)
        shell = argv.index(str(session.SCRIPT_LABWC_SHELL))
        self.assertEqual(argv[:4], ["bash", str(session.SCRIPT_PRIVATE_SESSION), "--no-x", "--"])
        self.assertEqual(argv[shell + 1:shell + 4], ["--rc", str(session.LABWC_RC), "--"])
        self.assertTrue(session.LABWC_RC.is_file())
        self.assertTrue(session.SCRIPT_LABWC_SHELL.is_file())

    def test_tier_keeps_xwayland_for_apps_but_no_x11_oracles(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state"
            state.mkdir()
            tier = session._build_tier(TIER, {
                "AGENT_DESKTOP_PRIVATE_SESSION": directory, "AGENT_DESKTOP_HOME": str(state),
                "DBUS_SESSION_BUS_ADDRESS": "unix:path=/private/bus", "DISPLAY": ":4",
                "WAYLAND_DISPLAY": "wayland-0"})
        self.assertIsNone(tier.display)
        self.assertEqual(tier.env["DISPLAY"], ":4")
        self.assertEqual(tier.wayland_display, "wayland-0")
        self.assertEqual(tier.env["XDG_SESSION_TYPE"], "wayland")
        self.assertNotIn("GDK_BACKEND", tier.env)

    def test_tier_is_selectable_and_owns_its_legs(self):
        self.assertIn(TIER, main.TIER_CHOICES)
        self.assertIn(TIER, session._INNER_WRAPPERS)
        labwc = [leg for leg in legs.REGISTRY if leg.id.startswith("labwc.")]
        self.assertEqual(len(labwc), 20)
        self.assertTrue(all(leg.tiers == (TIER,) for leg in labwc))
        self.assertTrue(all(leg.id in legs.implementations() for leg in labwc))
        self.assertFalse([leg.id for leg in legs.REGISTRY
                          if TIER in leg.tiers and not leg.id.startswith("labwc.")])


class OracleTests(unittest.TestCase):
    def test_outputs_keep_enabled_outputs_with_their_current_mode(self):
        self.assertEqual(wlr.parse_outputs(OUTPUT_JSON),
                         [wlr.Output("HEADLESS-1", 0, 0, 1280, 720, 1.0, 60.0)])

    def test_an_output_without_one_current_mode_is_rejected(self):
        text = json.dumps([{"name": "HEADLESS-1", "enabled": True, "modes": [{"width": 1, "height": 1}]}])
        with self.assertRaises(ValueError):
            wlr.parse_outputs(text)

    def test_the_committed_rules_place_both_apps_inside_the_output(self):
        placed = wlr.placements(session.LABWC_RC.read_text())
        self.assertEqual(set(placed), {labwc_input.CALCULATOR_ID, labwc_input.TEXT_EDITOR_ID})
        for x, y in placed.values():
            self.assertTrue(0 <= x < 1280 and 0 <= y < 720)

    def test_placed_point_is_the_centre_of_window_relative_extents(self):
        self.assertEqual(wlr.placed_point((40, 40), (24, 428, 64, 44)), (96, 490))
        with self.assertRaises(ValueError):
            wlr.placed_point((40, 40), (24, 428, 0, 44))

    def test_list_displays_and_wlr_randr_facts_compare_equal(self):
        displays = [{"id": "HEADLESS-1", "bounds": {"x": 0.0, "y": 0.0, "width": 1280.0, "height": 720.0},
                     "scale": 1.0, "is_primary": True}]
        self.assertEqual(labwc_windows.display_facts(displays),
                         labwc_windows.output_facts(wlr.parse_outputs(OUTPUT_JSON)))
        displays[0]["bounds"]["width"] = 1920.0
        self.assertNotEqual(labwc_windows.display_facts(displays),
                            labwc_windows.output_facts(wlr.parse_outputs(OUTPUT_JSON)))

    def test_window_region_is_clipped_to_the_output(self):
        output = wlr.Output("HEADLESS-1", 0, 0, 1280, 720, 1.0, 60.0)
        self.assertEqual(labwc_windows.window_region((40, 40), (0, 0, 360, 616), output),
                         (40, 40, 360, 616))
        self.assertEqual(labwc_windows.window_region((1000, 600), (0, 0, 360, 616), output),
                         (1000, 600, 280, 120))
        with self.assertRaises(ValueError):
            labwc_windows.window_region((1300, 0), (0, 0, 10, 10), output)

    def test_listed_windows_are_filtered_to_owned_processes(self):
        windows = [{"id": "w-1", "title": "Calculator", "pid": 10},
                   {"id": "w-2", "title": "Other", "pid": 99}]
        self.assertEqual(labwc_windows.listed_facts(windows, {10}), [("w-1", "Calculator")])

    def test_running_rejects_zombies_and_reused_pids(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "7").mkdir()
            fields = " ".join(["0"] * 16)
            for state, start, expected in (("S", 55, True), ("Z", 55, False), ("S", 56, False)):
                with self.subTest(state=state, start=start):
                    (root / "7" / "stat").write_text(f"7 (calc) {state} 1 1 {fields} {start} 0\n")
                    self.assertIs(labwc_windows.running(7, 55, root), expected)
            self.assertFalse(labwc_windows.running(8, 55, root))


class BrowserTests(unittest.TestCase):
    def _process(self, root: Path, pid: int, session_id: int, args: list[str]) -> None:
        directory = root / str(pid)
        directory.mkdir()
        (directory / "stat").write_text(f"{pid} (chromium) S 1 {pid} {session_id} 0 0\n")
        (directory / "cmdline").write_bytes(b"\0".join(arg.encode() for arg in args) + b"\0")

    def test_owned_processes_follow_the_session_and_private_paths_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._process(root, 100, 100, ["chromium", "--user-data-dir=/rt/p"])
            self._process(root, 101, 100, ["chromium", "--type=renderer"])
            self._process(root, 102, 102, ["crashpad", "--database=/rt/cfg/chromium/Crash Reports"])
            self._process(root, 103, 103, ["chromium", "--user-data-dir=/home/owner/.config"])
            self._process(root, 104, 104, ["labwc", "-C", "/rt/labwc/config"])
            found = labwc_browser.owned_processes(
                100, ("--user-data-dir=/rt/p", "--database=/rt/"), root)
        self.assertEqual(found, [100, 101, 102])

    def test_browser_uses_a_private_profile_on_wayland_with_accessibility(self):
        argv = labwc_browser.browser_argv("chromium", Path("/rt/p"), "file:///rt/page.html")
        for flag in ("--user-data-dir=/rt/p", "--ozone-platform=wayland",
                     "--force-renderer-accessibility", "--no-first-run"):
            self.assertIn(flag, argv)
        self.assertEqual(argv[-1], "file:///rt/page.html")

    def test_browser_binary_falls_back_in_order(self):
        self.assertEqual(labwc_browser.browser_binary(lambda name: name == "google-chrome"),
                         "google-chrome")
        with self.assertRaises(AssertionError):
            labwc_browser.browser_binary(lambda name: False)

    def test_hidden_hit_names_only_text_the_button_claims_but_does_not_list(self):
        class Node(SimpleNamespace):
            def invoke(self, interface, method, signature, args):
                self.asked = args
                return [(self.bus, self.lands)]

            def extents(self, window_coords=False):
                return (8, 95, 62, 21)

            def children(self):
                return self.listed

            def get_property(self, interface, name):
                return self.parent

        frame = Node(bus=":1.2", path="/frame", connection=object(), lands="/text")
        button = Node(bus=":1.2", path="/button", listed=[])
        text = Node(bus=":1.2", path="/text", parent=(":1.2", "/button"))
        with mock.patch.object(labwc_browser.raw, "RawNode", lambda conn, bus, path: text):
            self.assertIs(labwc_browser.hidden_hit(frame, button), text)
            self.assertEqual(frame.asked, (39, 105, 1))
            button.listed = [text]
            self.assertIsNone(labwc_browser.hidden_hit(frame, button))
            button.listed = []
            text.parent = (":1.2", "/section")
            self.assertIsNone(labwc_browser.hidden_hit(frame, button))
            frame.lands = "/button"
            self.assertIsNone(labwc_browser.hidden_hit(frame, button))


class PressLegTests(unittest.TestCase):
    def test_press_leg_passes_on_real_keys_and_fails_on_a_lying_product(self):
        for lying in (False, True):
            with self.subTest(lying=lying):
                leg = legs.leg_by_id("labwc.press_modifiers_replace_text")
                tier = SimpleNamespace(runtime_dir=Path("/private/test"), display=None)
                ctx = LegContext(tier, "headless", leg, member_reader=lambda _: frozenset())
                document, selected = [""], [False]

                def product(_tier, argv, **_kw):
                    if argv[0] == "clear":
                        document[0], selected[0] = "", False
                    elif argv[0] == "type":
                        document[0] += argv[2]
                    elif lying:
                        pass
                    elif argv[:2] == ["press", "ctrl+a"]:
                        selected[0] = True
                    elif argv[:2] == ["press", "shift+b"]:
                        document[0] = "B" if selected[0] else document[0] + "B"
                        selected[0] = False
                    return Envelope(True, {}, None, "{}", 0, argv)

                app = SimpleNamespace(name="gnome-text-editor", owner_names=[":1.5"],
                                      windows=["w-1.5-editor"], pid=5, start_time=5)
                with mock.patch.object(labwc_input, "launch", return_value=app), \
                        mock.patch.object(labwc_input, "stop"), \
                        mock.patch.object(labwc_input, "snapshot_ref", return_value="@s:e1"), \
                        mock.patch.object(labwc_input, "activate_by_pointer"), \
                        mock.patch.object(labwc_input, "document_node"), \
                        mock.patch.object(labwc_input.raw, "app_root"), \
                        mock.patch.object(labwc_input, "document_text",
                                          side_effect=lambda *_: document[0]), \
                        mock.patch("oracle.atspi_raw.poll", side_effect=lambda read, *a, **kw: read()), \
                        mock.patch("runner.verdict.run_product", side_effect=product):
                    if lying:
                        with self.assertRaisesRegex(AssertionError, "'B'"):
                            labwc_input.press_modifiers_replace_text(ctx)
                    else:
                        labwc_input.press_modifiers_replace_text(ctx)
                        self.assertEqual(verdict.evaluate(ctx), [])
                        self.assertEqual(ctx.reads["atspi.document_text"]["after"], ["B"])


if __name__ == "__main__":
    unittest.main()


class CompositorChoiceTests(unittest.TestCase):
    def _gate_with(self, root: Path, labwc: str):
        binary = root / "agent-desktop"
        binary.write_text("x")
        env = {"AGENT_DESKTOP_SESSION_BASE": str(root / "sessions"),
               "AGENT_DESKTOP_E2E_BIN": str(binary), gate.LABWC_VAR: labwc}
        which = lambda tool: None if tool in ("Xvfb", "xwd", "labwc") else f"/usr/bin/{tool}"
        return gate.check_prerequisites((TIER,), which=which, env=env, extra_paths=(),
                                        gi_probe=lambda: None, platform="linux")

    def test_a_chosen_labwc_binary_replaces_the_path_lookup(self):
        with tempfile.TemporaryDirectory() as directory:
            chosen = Path(directory) / "labwc"
            chosen.write_text("#!/bin/sh\n")
            chosen.chmod(0o755)
            self.assertEqual(self._gate_with(Path(directory), str(chosen)), [])

    def test_a_chosen_labwc_that_cannot_run_is_named(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = str(Path(directory) / "nope")
            self.assertEqual(self._gate_with(Path(directory), missing),
                             [f"missing tool for tier {TIER}: LABWC={missing} is not executable"])

    def test_the_recorded_version_heads_the_report(self):
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory) / "labwc").mkdir()
            (Path(directory) / "labwc" / "version").write_text("labwc 0.9.8 (+xwayland)\n")
            tier = SimpleNamespace(name=TIER, env={"XDG_RUNTIME_DIR": directory})
            with mock.patch.dict(os.environ, {"LABWC": "/opt/amp-wayland/bin/labwc"}):
                note = main._compositor_note(tier)
        self.assertEqual(note, f"compositor {TIER}: labwc 0.9.8 (+xwayland) "
                               "(/opt/amp-wayland/bin/labwc)")
        from runner import report
        text = report.render([], (note,))
        self.assertTrue(text.startswith(note))

    def test_a_not_applicable_leg_does_not_fail_the_run(self):
        from runner import report
        outcome = report.LegOutcome("leg", TIER, "headed", report.NOT_APPLICABLE, "absent")
        self.assertEqual(report.count_failures([outcome]), 0)

    def test_globals_are_read_from_wayland_info(self):
        text = ("interface: 'wl_seat',                                    version:  9, name: 3\n"
                "interface: 'ext_foreign_toplevel_image_capture_source_manager_v1', version:  1\n")
        self.assertEqual(wlr.parse_globals(text),
                         {"wl_seat", "ext_foreign_toplevel_image_capture_source_manager_v1"})


def _painted(points: dict[tuple[int, int], tuple[int, int, int]]):
    from oracle import pixels
    rgb = bytearray(bytes(3) * (64 * 64))
    for (x, y), colour in points.items():
        offset = (y * 64 + x) * 3
        rgb[offset:offset + 3] = bytes(colour)
    return pixels.Image(width=64, height=64, rgb=bytes(rgb))


class OverlayOracleTests(unittest.TestCase):
    def _dart(self, origin, fill=40, rim=40):
        x, y = origin[0] - 1, origin[1] - 5
        points = {(x + 2 + i % 20, y + 10 + i // 20): labwc_overlay.FILL for i in range(fill)}
        points.update({(x + 2 + i % 20, y + 25 + i // 20): labwc_overlay.RIM for i in range(rim)})
        return points

    def test_the_dart_is_found_only_around_its_tip(self):
        image = _painted(self._dart((10, 8)))
        self.assertTrue(labwc_overlay.marker_at(image, (10, 8)))
        self.assertTrue(labwc_overlay.marker_near(image, (14, 11)))
        self.assertFalse(labwc_overlay.marker_at(image, (40, 40)))

    def test_too_little_body_or_rim_is_not_the_dart(self):
        self.assertFalse(labwc_overlay.marker_at(_painted({}), (10, 8)))
        self.assertFalse(labwc_overlay.marker_at(_painted(self._dart((10, 8), fill=10)), (10, 8)))
        self.assertFalse(labwc_overlay.marker_at(_painted(self._dart((10, 8), rim=10)), (10, 8)))

    def test_ripple_and_rim_centre_are_read_from_their_colours(self):
        blue = {(30 + i % 8, 30 + i // 8): (66, 153, 255) for i in range(64)}
        self.assertEqual(labwc_overlay.ripple_pixels(_painted(blue), (32, 32)), 64)
        self.assertEqual(labwc_overlay.ripple_pixels(_painted({}), (32, 32)), 0)
        self.assertEqual(labwc_overlay.rim_centre(_painted(self._dart((10, 8)))), (20.5, 28.5))
        self.assertIsNone(labwc_overlay.rim_centre(_painted({})))

    def test_helpers_are_found_by_session_and_runtime_directory(self):
        tier = SimpleNamespace(runtime_dir=Path("/rt/a"))
        ctx = SimpleNamespace(tier=tier)
        table = {
            11: {"AGENT_DESKTOP_WLR_CURSOR_SESSION": "run-1", "XDG_RUNTIME_DIR": "/rt/a"},
            12: {"AGENT_DESKTOP_WLR_CURSOR_SESSION": "run-2", "XDG_RUNTIME_DIR": "/rt/a"},
            13: {"AGENT_DESKTOP_WLR_CURSOR_SESSION": "run-1", "XDG_RUNTIME_DIR": "/rt/b"},
            14: {"XDG_RUNTIME_DIR": "/rt/a"},
        }
        entries = [SimpleNamespace(name=str(pid)) for pid in table] + [SimpleNamespace(name="self")]
        with mock.patch.object(labwc_overlay, "Path") as path, \
                mock.patch.object(labwc_overlay.proc, "facts",
                                  side_effect=lambda pid: (0, 0, "x", table[pid])):
            path.return_value.iterdir.return_value = entries
            self.assertEqual(labwc_overlay.helpers(ctx, "run-1"), [11])


class _LabwcDesk:
    """The 1280x720 output as labwc stacks it: the placed Calculator, the overlay's dart and
    ripple above it, and above both the root menu that a click on the bare desktop opens with
    its corner on the click point. `honest` is False for a product that loses the dart once it
    has clicked."""

    WINDOW = (40, 40, 360, 620)
    EIGHT = (100, 300, 64, 44)

    def __init__(self, honest: bool):
        self.honest, self.dart, self.menu, self.display = honest, None, None, "0"

    def move(self, _ctx, _session, point, action="mouse-move"):
        self.dart = point
        if action != "mouse-click":
            return
        x, y, width, height = self.WINDOW
        if x <= point[0] < x + width and y <= point[1] < y + height:
            self.display = "8"
        else:
            self.menu = point

    def clear(self, _root):
        self.display = "0"

    def frame(self, ripple: bool, clicked: bool = False):
        from oracle import pixels
        rgb = bytearray(1280 * 720 * 3)

        def paint(x, y, colour):
            if 0 <= x < 1280 and 0 <= y < 720:
                rgb[(y * 1280 + x) * 3:(y * 1280 + x) * 3 + 3] = bytes(colour)

        if self.dart is not None:
            tip_x, tip_y = self.dart
            if ripple:
                for row in range(tip_y - 20, tip_y + 20):
                    for column in range(tip_x - 20, tip_x + 20):
                        paint(column, row, (66, 153, 255))
            if self.honest or not clicked:
                for index in range(40):
                    paint(tip_x + 2 + index % 20, tip_y + 5 + index // 20, labwc_overlay.FILL)
                    paint(tip_x + 2 + index % 20, tip_y + 20 + index // 20, labwc_overlay.RIM)
        if self.menu is not None:
            for row in range(self.menu[1], self.menu[1] + 120):
                for column in range(self.menu[0], self.menu[0] + 180):
                    paint(column, row, (250, 250, 248))
        return pixels.Image(width=1280, height=720, rgb=bytes(rgb))


class RippleLegTests(unittest.TestCase):
    def _run(self, desk: _LabwcDesk) -> LegContext:
        leg = legs.leg_by_id("labwc.cursor_overlay_click_ripple")
        tier = SimpleNamespace(runtime_dir=Path("/private/test"), display=None)
        ctx = LegContext(tier, "headed", leg, member_reader=lambda _: frozenset())

        class Sampler:
            def __init__(self, _ctx):
                pass

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def decoded(self):
                return [desk.frame(True, True), desk.frame(True, True), desk.frame(False, True)]

        eight = SimpleNamespace(extents=lambda _window: desk.EIGHT)
        with mock.patch.object(labwc_overlay, "_calculator",
                               return_value=(None, None, lambda: desk.display)), \
                mock.patch.object(labwc_overlay, "placement", return_value=desk.WINDOW[:2]), \
                mock.patch.object(labwc_overlay, "button", return_value=eight), \
                mock.patch.object(labwc_overlay, "screen", side_effect=lambda _: desk.frame(False)), \
                mock.patch.object(labwc_overlay, "Sampler", Sampler), \
                mock.patch.object(labwc_overlay, "move", side_effect=desk.move), \
                mock.patch.object(labwc_overlay, "start_session", return_value="run-1"), \
                mock.patch.object(labwc_overlay, "end_session"), \
                mock.patch.object(labwc_overlay, "disable_and_reap",
                                  side_effect=lambda *_: setattr(desk, "dart", None)), \
                mock.patch.object(labwc_overlay, "_clear_calculator", side_effect=desk.clear), \
                mock.patch.object(labwc_overlay, "stop"), \
                mock.patch("oracle.atspi_raw.poll", side_effect=lambda read, *a, **kw: read()):
            labwc_overlay.cursor_overlay_click_ripple(ctx)
        return ctx

    def test_the_ripple_leg_clicks_a_window_so_no_labwc_menu_covers_the_dart(self):
        desk = _LabwcDesk(honest=True)
        ctx = self._run(desk)
        self.assertIsNone(desk.menu, "the leg clicked the bare desktop and opened the root menu")
        self.assertEqual(verdict.evaluate(ctx), [])
        self.assertEqual(ctx.reads["atspi.display"]["after"], ["8"])

    def test_the_ripple_leg_fails_a_product_that_loses_the_dart_after_its_click(self):
        with self.assertRaisesRegex(AssertionError, "the dart was not over its ripple"):
            self._run(_LabwcDesk(honest=False))

