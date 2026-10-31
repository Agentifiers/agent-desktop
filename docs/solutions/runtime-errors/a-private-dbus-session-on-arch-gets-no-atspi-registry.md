---
module: scripts/private-labwc-shell.sh, tests/e2e-linux/runner/gate.py
date: 2026-10-30
problem_type: integration_issue
tags: [linux, at-spi, dbus, arch, headless, e2e]
---

# A private D-Bus session on Arch gets no AT-SPI registry

## Symptom

Inside `dbus-run-session`, `org.a11y.Bus` answers `GetAddress`, but apps never
appear: `agent-desktop list-apps` returns no rows and `status` shows an empty
desktop.

## Cause

Arch's at-spi2-core activates `at-spi2-registryd` through the systemd user
manager. A private `dbus-run-session` bus has no user manager behind it, so the
activation request goes to nobody and the registry never starts. Accessibility
is also off until `org.a11y.Status` `IsEnabled` is set, so toolkits do not load
their bridge.

## Fix

Inside the private session, enable accessibility, then start the registry
yourself and wait until it answers:

```bash
gdbus call --session -d org.a11y.Bus -o /org/a11y/bus \
  -m org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'
/usr/lib/at-spi2-registryd &
```

The binary lives in `/usr/libexec`, `/usr/lib`, `/usr/lib/at-spi2-core` or
`/usr/lib/x86_64-linux-gnu` depending on the distribution.
`scripts/private-labwc-shell.sh` searches those paths, and the e2e gate lists
`at-spi2-registryd` as a required tool for the `wayland-labwc` tier.
