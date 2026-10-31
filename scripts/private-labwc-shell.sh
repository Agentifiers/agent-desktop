#!/usr/bin/env bash
# Runs a command against a private headless labwc (wlroots) session shaped like a cloud
# desktop: headless backend, pixman renderer, no input devices, one 1280x720 output at 60 Hz,
# Xwayland kept alive by a persistent X client, and accessibility enabled with its own
# registry daemon.
# Usage: scripts/private-session.sh --no-x -- scripts/private-labwc-shell.sh [--rc FILE] [--] \
#          command [args...]
# --rc names a labwc rc.xml (for example window placement rules); the default is empty.
# LABWC names the compositor binary (default: labwc on PATH) and LABWC_LD_LIBRARY_PATH the
# library path that binary alone runs with, so a separately installed labwc can be driven,
# for example LABWC=/opt/amp-wayland/bin/labwc
# LABWC_LD_LIBRARY_PATH=/opt/amp-wayland/lib/x86_64-linux-gnu:/opt/amp-wayland/lib.
# The compositor's `--version` line is written to $XDG_RUNTIME_DIR/labwc/version.
# The output mode names a refresh rate on purpose: a headless output without one runs at
# 0 Hz, never schedules frames, and clients stop repainting, so every capture reads stale
# pixels. The AT-SPI registry is started here because some distributions activate it only
# through the systemd user manager, which belongs to the owner's real desktop.
# WAYLAND_DISPLAY and DISPLAY name the private compositor and its Xwayland for the command;
# the command is not run unless both answer and the registry is up.
set -euo pipefail

[ -n "${AGENT_DESKTOP_PRIVATE_SESSION:-}" ] || {
  echo "private-labwc-shell: run it through scripts/private-session.sh" >&2
  exit 2
}
rc=
if [ "${1:-}" = "--rc" ]; then
  rc=${2:?--rc needs a file}
  shift 2
fi
[ "${1:-}" = "--" ] && shift
[ $# -gt 0 ] || { echo "usage: private-labwc-shell.sh [--rc FILE] [--] command [args...]" >&2; exit 2; }
[ -z "$rc" ] || [ -f "$rc" ] || { echo "private-labwc-shell: no rc file at $rc" >&2; exit 2; }

rt=$XDG_RUNTIME_DIR
dir=$rt/labwc
labwc_bin=${LABWC:-labwc}
labwc_libs=${LABWC_LD_LIBRARY_PATH:-}
mode=1280x720@60Hz
labwc_pid=
helper_pids=()

fail() {
  echo "private-labwc-shell: $*" >&2
  exit 1
}

# shellcheck disable=SC2317,SC2329
stop_labwc() {
  local pid
  for pid in "${helper_pids[@]+"${helper_pids[@]}"}"; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  if [ -n "$labwc_pid" ]; then
    kill -TERM -- "-$labwc_pid" 2>/dev/null || true
    wait "$labwc_pid" 2>/dev/null || true
    labwc_pid=
  fi
}
trap stop_labwc EXIT

registryd() {
  local candidate
  if command -v at-spi2-registryd >/dev/null 2>&1; then
    command -v at-spi2-registryd
    return 0
  fi
  for candidate in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-registryd \
    /usr/lib/at-spi2-core/at-spi2-registryd /usr/lib/x86_64-linux-gnu/at-spi2-registryd; do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

registry_bin=$(registryd) || fail "at-spi2-registryd was not found"

mkdir -p "$dir/config"
command -v "$labwc_bin" > /dev/null 2>&1 || fail "no labwc binary at $labwc_bin"
env ${labwc_libs:+LD_LIBRARY_PATH="$labwc_libs"} "$labwc_bin" --version > "$dir/version" 2>&1 ||
  fail "$labwc_bin --version failed: $(cat "$dir/version")"
if [ -n "$rc" ]; then
  cp -- "$rc" "$dir/config/rc.xml"
else
  : > "$dir/config/rc.xml"
fi
ready=$dir/ready.env
rm -f "$ready" "$ready.tmp"
cat > "$dir/ready.sh" <<EOF
#!/bin/sh
unset LD_LIBRARY_PATH
{
  printf 'WAYLAND_DISPLAY=%s\n' "\$WAYLAND_DISPLAY"
  printf 'DISPLAY=%s\n' "\${DISPLAY:-}"
} > "$ready.tmp"
mv "$ready.tmp" "$ready"
EOF
chmod 700 "$dir/ready.sh"

setsid env -u DISPLAY -u WAYLAND_DISPLAY \
  WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=1 WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  ${labwc_libs:+LD_LIBRARY_PATH="$labwc_libs"} \
  "$labwc_bin" -C "$dir/config" -s "$dir/ready.sh" > "$rt/labwc.log" 2>&1 < /dev/null &
labwc_pid=$!

for _ in $(seq 1 100); do
  [ -s "$ready" ] && break
  kill -0 "$labwc_pid" 2>/dev/null || break
  sleep 0.1
done
[ -s "$ready" ] || fail "labwc did not start; see $rt/labwc.log"

WAYLAND_DISPLAY=$(sed -n 's/^WAYLAND_DISPLAY=//p' "$ready")
DISPLAY=$(sed -n 's/^DISPLAY=//p' "$ready")
[ -n "$WAYLAND_DISPLAY" ] || fail "labwc named no Wayland socket"
[ -n "$DISPLAY" ] || fail "labwc started without Xwayland"
export WAYLAND_DISPLAY DISPLAY
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=labwc:wlroots

globals=$(wayland-info) || fail "wayland-info could not read the compositor's globals"
for global in wl_seat zwp_virtual_keyboard_manager_v1 zwlr_virtual_pointer_manager_v1 \
  zwlr_screencopy_manager_v1 zwlr_foreign_toplevel_manager_v1; do
  printf '%s\n' "$globals" | grep -q "'$global'" || fail "labwc advertises no $global"
done

output=$(wlr-randr | sed -n '1s/ .*//p')
[ -n "$output" ] || fail "labwc reports no output"
wlr-randr --output "$output" --custom-mode "$mode" || fail "the output refused mode $mode"
wlr-randr | grep -q "1280x720 px, 60.0.* (current)" || fail "the output did not take mode $mode"

xsettingsd -c /dev/null > "$rt/xsettingsd.log" 2>&1 < /dev/null &
helper_pids+=("$!")
xwayland_up=0
for _ in $(seq 1 100); do
  if xprop -root _NET_SUPPORTING_WM_CHECK > /dev/null 2>&1; then
    xwayland_up=1
    break
  fi
  sleep 0.1
done
[ "$xwayland_up" = 1 ] || fail "Xwayland did not answer on $DISPLAY"

dbus-update-activation-environment DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP
gdbus call --session -d org.a11y.Bus -o /org/a11y/bus \
  -m org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>' > /dev/null ||
  fail "the accessibility bus did not accept IsEnabled"
"$registry_bin" > "$rt/registryd.log" 2>&1 < /dev/null &
helper_pids+=("$!")
a11y=$(gdbus call --session -d org.a11y.Bus -o /org/a11y/bus -m org.a11y.Bus.GetAddress |
  sed -n "s/^('\\(.*\\)',)\$/\\1/p")
[ -n "$a11y" ] || fail "the accessibility bus named no address"
registry_up=0
for _ in $(seq 1 100); do
  if gdbus call --address "$a11y" -d org.a11y.atspi.Registry -o /org/a11y/atspi/registry \
    -m org.a11y.atspi.Registry.GetRegisteredEvents > /dev/null 2>&1; then
    registry_up=1
    break
  fi
  sleep 0.1
done
[ "$registry_up" = 1 ] || fail "the AT-SPI registry did not come up; see $rt/registryd.log"

"$@"
