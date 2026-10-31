---
module: crates/linux/src/system/focus/focus_wlroots.rs, tests/e2e-linux
date: 2026-10-30
problem_type: runtime_error
tags: [linux, wayland, wlroots, labwc, focus, headless, e2e]
---

# A fresh labwc seat activates no window until it has seen input

## Symptom

In a headless labwc session with `WLR_LIBINPUT_NO_DEVICES=1`, a newly mapped
window is not activated, and no AT-SPI frame carries `ACTIVE`. A scoped
`press` refuses its focus check, `focus-changed` waits time out, and a focus
that the compositor granted still cannot be confirmed from the frame.
`wayland-info` shows the seat with empty capabilities.

## Cause

labwc activates a window on map only after its seat has seen input. A seat
with no keyboard also sends no keyboard focus to any client, so toolkits never
mark a frame `ACTIVE`, even for the toplevel the compositor reports activated.

## Fix

- The e2e harness gives the seat input first: the key and focus legs of the
  `wayland-labwc` tier click inside the placed Text Editor with a headed
  `mouse-click` before they act.
- `focus-window` on the wlroots route asks the compositor to activate the
  toplevel through `zwlr_foreign_toplevel_manager_v1`. When the seat has no
  keyboard, it attaches a virtual keyboard that sends no key while it waits for
  the frame to report `ACTIVE`, so the frame check can pass. labwc 0.20.2 keeps
  that keyboard on the seat afterwards, which also lets a later headed `type`
  pass its own focus check. This was not checked on labwc 0.9.8; a compositor
  that drops the keyboard would make that later check refuse again.

Evidence: `focus_wlroots_tests.rs`, tests
`without_a_keyboard_one_is_attached_and_the_frame_still_verifies_the_focus` and
`a_seat_with_a_keyboard_never_gets_another`.
