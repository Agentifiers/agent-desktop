---
module: scripts/private-labwc-shell.sh, tests/e2e-linux
date: 2026-10-30
problem_type: runtime_error
tags: [linux, wayland, wlroots, labwc, headless, screenshot, e2e]
---

# A headless wlroots output without a refresh rate never repaints

## Symptom

In a headless labwc session (`WLR_BACKENDS=headless`), apps stop drawing after
the first frame. `agent-desktop screenshot` and `grim` both return pixels that
do not show the change an action just made, so a pixel oracle disagrees with
the accessibility tree and a correct action looks like a failure.

## Cause

The output was configured with `wlr-randr --output <name> --custom-mode
1280x720`. A custom mode without a refresh rate runs at 0 Hz. The headless
backend then schedules no frames: clients get no frame callbacks and stop
drawing, and screencopy copies whatever the output last composited.

## Fix

Name the refresh rate in the mode and check that the output took it:

```bash
wlr-randr --output "$output" --custom-mode 1280x720@60Hz
wlr-randr | grep -q "1280x720 px, 60.0.* (current)"
```

`scripts/private-labwc-shell.sh` sets `mode=1280x720@60Hz` and fails the
session start when the output does not report 60 Hz. The rule is recorded in
`tests/e2e-linux/README.md` next to the `wayland-labwc` tier.
