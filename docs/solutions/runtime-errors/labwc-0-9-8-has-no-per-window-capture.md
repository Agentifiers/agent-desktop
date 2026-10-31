---
module: crates/linux/src/system/capture/wlroots_window.rs, crates/linux/src/system/wlroots
date: 2026-10-30
problem_type: integration_issue
tags: [linux, wayland, wlroots, labwc, screenshot, pointer, compatibility]
---

# labwc 0.9.8 has no per-window capture

## Symptom

On labwc 0.9.8, `agent-desktop screenshot --app <name>` and `--window-id`
answer `PLATFORM_NOT_SUPPORTED` ("A single window cannot be captured on a
Wayland session"), while the same commands return the window on labwc 0.20.2.
Headed ref pointer actions on an unmaximized window refuse there too.

## Cause

labwc 0.9.8 advertises `ext_foreign_toplevel_list_v1`,
`ext_image_copy_capture_manager_v1`, `zwlr_foreign_toplevel_manager_v1`,
`zwlr_screencopy_manager_v1` and the virtual input protocols, but not
`ext_foreign_toplevel_image_capture_source_manager_v1`. Without that global a
client cannot turn a toplevel into a capture source, so it cannot copy one
window. The same copy is how the CLI proves where a window sits in the output
layout: no wlroots protocol tells a client a window's position, and AT-SPI
gives none on Wayland.

## What works there

- Display screenshots through screencopy, keys, coordinate pointer commands,
  `focus-window`, graceful `close-app`, `minimize`, `maximize` and `restore`.
- Ref pointer aiming on a window the compositor reports active and maximized
  or fullscreen, whose origin follows from the output geometry. Any other
  window refuses before a pointer event with `ACTION_FAILED`, `details.kind:
  "wlroots_window_origin_unproven"`, `not_delivered`, no semantic fallback,
  and the suggestion "Run 'maximize --window-id <id>' and 'focus-window
  --window-id <id>', then retry; or run the action without --headed to act
  through accessibility; or click a point you located yourself with
  'mouse-click --xy'".

Check a compositor with `wayland-info | grep image_capture_source` before you
rely on window capture.

## Known limit

Not specific to 0.9.8: under both labwc 0.9.8 and 0.20.2 with Xwayland 24.1.14,
an Xwayland window takes only the first synthetic click; later clicks are lost
while the command reports `delivered_unverified`, and Xwayland logs
`pointer_handle_leave` BUG errors. Native Wayland windows take every click.
