---
name: agent-desktop-linux
tags: linux-automation, accessibility, at-spi, x11, wayland, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Linux platform guide for the agent-desktop CLI. Lists per command group
  what works on X11, on GNOME Wayland and on wlroots compositors such as
  labwc, how to check the AT-SPI accessibility bus,
  the first-contact gotchas (app names, Chromium and Electron flags, headed
  focus, blocked key combos), and where to find lifecycle, screenshot,
  clipboard, tray, notification and troubleshooting details.
  Triggers on: "linux desktop automation", "AT-SPI tree", "automate GNOME app",
  "agent-desktop on Linux", "accessibility bus not available", "empty
  supported_surfaces", "wayland screenshot consent", or any Linux GUI
  interaction task.
---

# agent-desktop-linux

The observe-act loop, refs, JSON envelope, sessions and tracing match every
platform. This file covers only what differs on Linux.

Needs Ubuntu 24.04+ (x86_64 or aarch64), a graphical session of the signed-in
user and a running AT-SPI bus. Other distributions need the at-spi2-core bridge
fix dc0dc33; Ubuntu 22.04's bridge aborts on `Action.GetActions`. The archive
loader floor is glibc 2.35. Run the CLI inside the target apps' desktop session:
the session bus, display and runtime directory come from its environment. A TTY
login, an SSH shell without session variables, a container without a bus or
another user's session sees no apps. GNOME is the measured desktop; X11 and
Wayland sessions both work and differ in input, capture and window control. On
Wayland, a compositor that offers `wl_seat`, `zwp_virtual_keyboard_manager_v1`
and `zwlr_virtual_pointer_manager_v1` (labwc, sway) takes the wlroots route,
chosen from its registry globals, never its name; any other compositor takes
the GNOME route. Observation and semantic actions need no helper tool.

## Capability Table

"wlroots" means a Wayland compositor on the wlroots route; "GNOME Wayland" means
every other Wayland compositor.

| Group | Commands | Status on Linux |
|-------|----------|-----------------|
| Observation | `snapshot`, `find`, `get`, `is` | Works on X11 and Wayland through AT-SPI. Supports `window`, `sheet`, `alert` and `menu`; GNOME adds shell surfaces. Virtual tray getters have limits; see the shell reference and check `status` |
| Surfaces | `list-surfaces` | Lists an app's windows, sheets and open menus; shell reads use `snapshot --surface` |
| Ref interaction | `click`, `right-click`, `type`, `set-value`, `clear`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll-to` | Need a readable live identity and the required AT-SPI capability. See the shell reference for menu-ref resolution limits; X11 headed fallback also needs usable geometry and does not bypass identity checks. wlroots headed ref clicks need a provable window origin; see `references/display-servers-and-input.md` |
| Wheel scrolling | `scroll`, `mouse-wheel` | Physical wheel input; X11 and wlroots, `--headed` only |
| Pointer gestures | `double-click`, `triple-click`, `hover`, `drag` | `--headed`, X11 and wlroots. On GNOME Wayland the pointer cannot be aimed and these refuse with `ACTION_NOT_SUPPORTED`. wlroots aims at coordinates; aiming at a ref has limits |
| Focus | `focus` | Needs `--headed` for the focus permission. Calls AT-SPI `Component.GrabFocus` on a focusable node that is not yet `focused`, not the pointer, and verifies the state; a GTK4 app rejects the call, so `focus` ends `ACTION_FAILED` `grab_focus_rejected` there and `focus-window` is the way to focus its window |
| Keyboard and mouse | `press`, `mouse-click`, `mouse-move`, `mouse-wheel` | `press` and text keys work on X11, capability-detected Mutter Wayland sessions and wlroots (any Unicode character); pointer commands are X11 and wlroots, `--headed`. See `references/display-servers-and-input.md` |
| Held input | `key-down`, `key-up`, `mouse-down`, `mouse-up` | Fail closed on every platform: on Linux, `key-down` and `key-up` with `ACTION_NOT_SUPPORTED`; `mouse-down` and `mouse-up` with `POLICY_DENIED` headless and `ACTION_NOT_SUPPORTED` with `--headed` |
| App and window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `move-window`, `resize-window`, `minimize`, `maximize`, `restore` | Launch and listing work everywhere. X11: all window control through EWMH. GNOME Wayland: `focus-window` refused; other window control and graceful close need an X11 client. wlroots: `focus-window`, graceful close, `minimize`, `maximize` and `restore` through `zwlr_foreign_toplevel_manager_v1`; `move-window` and `resize-window` refuse with `wlroots_no_geometry_control`. See `references/windows-and-lifecycle.md` |
| Displays | `list-displays` | Works; wlroots reads `wl_output` and `xdg_output` |
| Capture | `screenshot` | X11: a window without permission; a display only under a non-compositing manager. GNOME Wayland: portal consent, with display cropping. wlroots: screencopy with no consent; a window needs `ext_foreign_toplevel_image_capture_source_manager_v1` (labwc 0.20, not 0.9.8). No selector returns the primary display. See `references/capture-and-clipboard.md` |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | Works; text, images and file lists. Wayland needs `wl-clipboard` |
| Wait | `wait` | Works for ms, `--element`, `--window`, `--text`, `--menu`, `--menu-closed`, `--event`. Menu waits share the menu-surface detector; `--notification` waits for a new notification on GNOME |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | GNOME Shell only; list headless, mutations `--headed` on X11, and `dismiss-notification` and `notification-action` also need `--expected-app` or `--expected-title`. Fields are read from the shell's widget layout, verified on GNOME Shell 46.0 only, and each mutation is proven by its effect. Wayland refuses the shell pointer before input. A notification owner that is not the shell or its own helper returns `PLATFORM_NOT_SUPPORTED`. See the shell reference |
| Shell surfaces | `open-system-surface` | Only `system-tray` and `action-center` can work. `system-tray` needs a StatusNotifierWatcher (else it refuses with `watcher: absent`); `action-center` needs proven role ancestry. `quick-settings` refuses with `shell_container_unproven`, `taskbar` always refuses, and `start-menu` and `system-tray-overflow` are not supported, though `--help` lists them. See the shell reference |
| Cursor overlay | `cursor-overlay` | Requires an active session; see Cursor overlay below. X11 and wlroots with `zwlr_layer_shell_v1` (labwc) enable returns `data.rendered: true`; GNOME Wayland saves the setting with `data.rendered: false` |
| System | `status`, `permissions`, `version`, `batch`, `skills`, `session`, `trace` | Works. `permissions` reports `screen_recording: not_required` on X11 and on wlroots with screencopy |

On labwc the two tested versions differ only where a protocol is missing:
labwc 0.9.8 lacks `ext_foreign_toplevel_image_capture_source_manager_v1`.

| Command | labwc 0.20.2 | labwc 0.9.8 |
|---|---|---|
| `list-displays`, `list-windows`, display `screenshot` | Works | Works |
| `press`, `type`, headed `type @ref` | Works | Works |
| `mouse-click`, `mouse-move`, `mouse-wheel`, `drag` at coordinates | Works on native windows; an Xwayland window takes only the first click | Works on native windows; an Xwayland window takes only the first click |
| `focus-window`, graceful `close-app`, `minimize`, `maximize`, `restore` | Works | Works |
| `move-window`, `resize-window` | Refused, `wlroots_no_geometry_control` | Refused, `wlroots_no_geometry_control` |
| `screenshot --app`/`--window-id` | Works | Refused, `PLATFORM_NOT_SUPPORTED` |
| Headed ref pointer actions | Active maximized or fullscreen window, or any window whose pixels are found | Active maximized or fullscreen window only; otherwise refused, `wlroots_window_origin_unproven` |
| `cursor-overlay` | Drawn; ref actions mark their target wherever headed ref pointer actions can aim | Drawn; ref actions mark their target only on an active maximized or fullscreen window |

## First Contact

- **Run `agent-desktop status` first.** An empty `supported_surfaces` means the
  accessibility bus did not answer, not an empty desktop. Read
  `references/bus-and-permissions.md`; do not restart a bus you did not start.
- **`--app` takes the name `list-apps` prints** (`gnome-calculator`,
  `nautilus`) or a desktop-entry name or id backed by verified executable and
  argument identity, never a window title or an arbitrary registry label. A
  desktop-entry name such as `Files` is accepted once the entry exists.
  `list-windows --app` is exact (aliases included); `list-apps --app` combines a
  substring of the listed name with exact adapter aliases, ignoring case.
- **Window ids are opaque addresses** of 34 to 70 characters, valid for the
  life of the window's process; run `list-windows` again after a restart.
  `list-apps` rows carry `bundle_id` (desktop id when known) and
  `process_instance`.
- **GTK4 apps nest deeply.** `--max-depth` and `--skeleton` count semantic
  levels, so empty layout wrappers add output levels. `-i --compact` still
  shows nested nameless `group` nodes: read past them.
- **An empty file view does not prove an empty folder.** If an icon view
  exposes no file items, try an available list or detailed view, then take a
  fresh snapshot and verify which items the application exposes.
- **A window appears before its content.** Run
  `wait --window "Text Editor" --app gnome-text-editor` first, then
  `wait --text`, which matches a node's own name, value or description. For a
  web-engine app (detected from its AT-SPI toolkit metadata), a window
  observation waits for three equal full reads and a web-document role, or a
  requested depth cut. A shell with no document yet stays incomplete when the
  budget ends. A `--timeout` shorter than the first read returns `TIMEOUT`,
  never a partial tree.
- **Geometry-only refs require unchanged geometry.** Position and size must
  both match, in the ref's recorded coordinate frame; a one-pixel change breaks
  identity. After a layout change or a window move, take a fresh snapshot
  instead of retrying the old ref. Indistinguishable siblings stay
  `AMBIGUOUS_TARGET`: in a Text Editor window with three tabs, the text views
  share one rectangle and one state, so a ref to any of them answers
  `AMBIGUOUS_TARGET` with `candidate_count` 3 until the extra tabs close. A ref
  belongs to the window it was taken in, so two windows with the same title each
  resolve their own refs.
- **Chromium and Qt apps need a launch flag or variable** to appear on the bus;
  Electron apps need none, only `--no-sandbox` where `chrome-sandbox` is not
  setuid. See `references/troubleshooting.md`.
- **Headed ref actions need a window that can take focus.** A window the
  window manager never activated ends `activation_not_granted`. See
  `references/windows-and-lifecycle.md`.
- **A GTK4 secret field can show its text.** Only a password role is withheld.
  Treat a GTK4 entry with hidden characters as readable by every observation
  command.
- **Blocked key combos refuse without `--force`.** See
  `references/display-servers-and-input.md`.
- **After opening a menu-role popup, inspect `data.surfaces`.** A `menu` entry
  leads to `snapshot --surface menu --app <name> -i`. If absent, use
  `wait --menu` and `list-surfaces` before assuming the opener failed. A
  list-backed dropdown is part of the window, not a menu surface; see the shell
  reference.
- **Shell pointer input requires receiver proof.** On X11 a headed shell click
  is proven from the shell's own tree: the receiver must be the target and stand
  still, or the click refuses before any pointer event. Wayland shell clicks
  refuse. Notifications need the GNOME notification service; tray snapshots also
  need a StatusNotifierWatcher.
- **The state root must be private.** A group-writable or world-accessible
  `AGENT_DESKTOP_HOME` or `~/.agent-desktop` makes `snapshot` fail with
  `INTERNAL`; run `chmod 700` on it or let the tool create it.

## Wait for a window

Wait for an app window to appear (`--window` requires a title or title substring):

```bash
agent-desktop wait --window "Text Editor" --app gnome-text-editor --timeout 10000
```

To wait for a close that you cause, batch it so the baseline is taken before the
close (Calculator has no unsaved-document prompt to interrupt it):

```bash
agent-desktop batch '[{"command":"close-app","args":{"app":"gnome-calculator"}},{"command":"wait","args":{"event":"window-closed","app":"gnome-calculator","timeout":10000}}]' --stop-on-error
```

To watch someone else close a window, start
`agent-desktop wait --event window-closed --app gnome-text-editor --timeout 10000`
before they close the window. There is no `--window-closed` flag.

## Cursor overlay

Start a session and pass its returned ID to every overlay command:

```bash
session_id=$(agent-desktop session start --name docs-example | jq -r '.data.session_id')
agent-desktop cursor-overlay enable --session "$session_id"
agent-desktop cursor-overlay disable --session "$session_id"
agent-desktop session end "$session_id"
```

Without a session, `cursor-overlay enable` and `disable` return `INVALID_ARGS`.
On X11 and on a wlroots compositor with `zwlr_layer_shell_v1` (labwc), enable
returns `data.rendered: true`. On GNOME Wayland, or a compositor without a
layer shell, it saves the setting and returns `data.rendered: false` without
drawing an overlay.

Both draw one white arrow with a dark rim. Its tip jumps, without animation,
to each point a pointer command acts on and to the target of a ref action.
Labels, ripples and highlights are not drawn; `--image` or `--pointer-image`
gives `data.rendered: false`. A `--headed` command hides the arrow while it
dispatches and shows it again afterwards. `screenshot` hides it during the
capture, so it is never in the image; other screen readers such as `grim` see
it.

On wlroots:

- The compositor does not say where the pointer is, so nothing is drawn after
  `enable` until the first pointer command or ref action places the arrow.
- A ref action marks its target only where the window's place is proven (see
  the labwc table above). Otherwise the arrow stays where it was.
- The arrow is a surface on the overlay layer with no keyboard focus and an
  empty input region: clicks and hover reach the window beneath, activation
  does not change, and `list-windows` does not list it.
- A second `agent-desktop` process per session holds it. That process exits
  on `cursor-overlay disable` and `session end`, when the session ends or its
  overlay is turned off, and when the compositor exits.

`session start --cursor --multi-agent` returns `data.rendered: false`, because
nothing is drawn until an agent acts. On X11 and wlroots each agent id then
gets its own cursor; on GNOME Wayland none is drawn.

## What differs on Linux

- Firefox/Gecko `select` can change the selected value without firing `change`
  or `input` events, so dependent page handlers do not run. The result stays
  `delivered_unverified` even when value readback matches. Check the dependent
  application effect before continuing: value readback alone is no commit proof.
- Two `click` calls on one GTK4 button less than about 250 ms apart act once.
  Both answer `ok: true` with `delivered_unverified`. To click one button
  several times, wait for the effect between the clicks: use
  `wait --element <ref> --predicate value --value <expected>` on the element
  that shows the result, or `wait 300`.
- Ref actions are semantic AT-SPI calls that never move the pointer or change
  the active window. Under `--headed`, ref `click`, `right-click` and `type` try
  physical input first and fall back to the semantic action only after a
  `not_delivered` refusal (Windows works the other way round). On GNOME Wayland,
  headed pointer commands and a headed `select` refuse at once with
  `ACTION_NOT_SUPPORTED`, cause `wayland_session`; a headed `type` needs the
  Mutter RemoteDesktop capability. On wlroots, headed keys go through a virtual
  keyboard and pointer commands through a virtual pointer.
  `references/display-servers-and-input.md` has the full rules.
- `select` tries the semantic route first and the pointer second; the pointer
  route needs an explicit `--headed` request on X11. A Qt combo's semantic
  selection only highlights its popup row, so headless selection refuses with
  `POLICY_DENIED`, kind `pointer_required`, before changing that highlight.
  Headed, on a list whose toolkit index cannot be mapped to the
  row (a GTK4 list) it clicks the row and checks its `selected` state; on a
  combo with no semantic opener (a libadwaita combo row) it clicks the combo
  open, selects the choice through the popup's own list and checks the value the
  combo shows. Headless, a list refuses with `selection_index_unproven`, and a
  combo with no semantic opener with `POLICY_DENIED`, kind `pointer_required`,
  unless the combo already shows the choice. A GTK4 drop-down that opens through
  its own toggle takes `select` without `--headed` and checks the value it
  shows.
- A ref can expose no safe semantic mutation route. Headless requests then
  return an unsupported-action error without inventing `hidden` or `disabled`
  state and without switching to physical input. GTK4 entry and password
  `Activate` actions are one such case: use `type` to edit the field, or request
  `--headed` explicitly when a pointer click is intended.
- Opening a native Wayland Qt combo returns `ACTION_NOT_SUPPORTED`, kind
  `native_popup_requires_input`, before invoking its action. Qt needs a user
  input serial to show that popup. An already open combo may be collapsed;
  Qt windows proven to use Xwayland retain their semantic opener.
- Headless `type` writes through `EditableText`. A terminal's text node is a
  `textfield` named `Terminal` whose only action is `RightClick`; headless
  `type` on it is `POLICY_DENIED` with check `supported_action`, and its value is
  null. It receives keys only under `--headed`.
- In key combos `cmd` and `meta` are the Super key. Key names are the macOS and
  Windows names plus X keysym names. `press` takes mapped keys only; `type`
  handles characters the layout cannot produce.
- On X11 and wlroots, window ops act on the window; on GNOME Wayland only on
  Xwayland clients, and `focus-window` and headed scoped `press` refuse there.
  wlroots matches a window to its compositor toplevel by title, then app id, so
  two windows of one app with one title are `AMBIGUOUS_TARGET`. Headless scoped
  `press` requires live target focus on every display server. Focusing a window
  on another workspace can switch the user's current desktop.
- Linux exact FFI refs carrying bounds or a bounds hash require window-relative
  geometry, even when the ref also carries a name or identifier. Geometry-free
  identity refs are supported.

## References

- `references/bus-and-permissions.md`: bus bootstrap, the three bus states,
  `status` and `permissions` output, state-root rules.
- `references/display-servers-and-input.md`: route choice, X11, GNOME Wayland
  and wlroots table, `press`, `type`, pointer, wheel, drag, blocked combos, key
  names.
- `references/windows-and-lifecycle.md`: `launch`, `close-app`, window ops,
  focus, workspaces, stopped apps, background `press`.
- `references/capture-and-clipboard.md`: `screenshot` on X11, GNOME Wayland and
  wlroots, the Wayland consent dialog, clipboard owner and helper.
- `references/shell-tray-and-notifications.md`: menus and GTK4 popovers, GNOME
  Overview/top bar, shell reads and input, notification identity guards, SNI
  and AppIndicator requirements and refusals, sandbox guidance.
- `references/troubleshooting.md`: symptom-to-cause map, Chromium, Electron
  and Qt flags, toolkit limits, error codes with `platform_detail`.

Read any file from the binary:
`agent-desktop skills get agent-desktop-linux references/troubleshooting.md`.
