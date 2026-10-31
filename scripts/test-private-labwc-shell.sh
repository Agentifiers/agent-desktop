#!/usr/bin/env bash
# Checks the private labwc launcher: it refuses to run outside a private session, and inside one
# it hands the command a private Wayland socket, a live Xwayland display, one 1280x720 output
# at 60 Hz, the wlroots input globals and an answering AT-SPI registry.
#   bash scripts/test-private-labwc-shell.sh
set -euo pipefail
cd "$(dirname "$0")/.."

status=0
env -u AGENT_DESKTOP_PRIVATE_SESSION bash scripts/private-labwc-shell.sh -- true 2>/dev/null || status=$?
[ "$status" = 2 ] || { echo "the launcher ran outside a private session (exit $status)" >&2; exit 1; }
status=0
bash scripts/private-session.sh --wm --no-x -- true 2>/dev/null || status=$?
[ "$status" = 2 ] || { echo "private-session accepted --wm with --no-x (exit $status)" >&2; exit 1; }

check=$(cat <<'SH'
set -euo pipefail
[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || { echo "no private Wayland socket" >&2; exit 1; }
case "$DISPLAY" in :[0-9]*) ;; *) echo "no Xwayland display: '$DISPLAY'" >&2; exit 1 ;; esac
xprop -root _NET_SUPPORTING_WM_CHECK | grep -q 'window id' || { echo "Xwayland has no WM" >&2; exit 1; }
wlr-randr | grep -q '1280x720 px, 60.0.* (current)' || { echo "the output is not 1280x720@60" >&2; exit 1; }
[ "$(wlr-randr --json | grep -c '"name"')" = 1 ] || { echo "expected one output" >&2; exit 1; }
for global in zwp_virtual_keyboard_manager_v1 zwlr_virtual_pointer_manager_v1; do
  wayland-info | grep -q "'$global'" || { echo "no $global" >&2; exit 1; }
done
a11y=$(gdbus call --session -d org.a11y.Bus -o /org/a11y/bus -m org.a11y.Bus.GetAddress |
  sed -n "s/^('\(.*\)',)\$/\1/p")
gdbus call --address "$a11y" -d org.a11y.atspi.Registry -o /org/a11y/atspi/registry \
  -m org.a11y.atspi.Registry.GetRegisteredEvents > /dev/null
echo "the private labwc session is ready"
SH
)

bash scripts/private-session.sh --no-x -- bash scripts/private-labwc-shell.sh \
  --rc tests/e2e-linux/runner/labwc-rc.xml -- bash -c "$check"
