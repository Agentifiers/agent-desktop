# Display Servers and Input on Linux

Observation and semantic ref actions use AT-SPI and behave the same on X11 and
Wayland. Input, window control and capture differ.

## How the display server is chosen

The CLI reads `WAYLAND_DISPLAY`, `DISPLAY` and `XDG_RUNTIME_DIR` (an empty value
counts as unset). A Wayland socket that accepts a connection wins, because an
Xwayland client lives on a Wayland desktop whose windows X11 calls cannot list.
A stale `WAYLAND_DISPLAY` from another session falls through to a working
`DISPLAY`. With neither reachable, display-dependent commands fail with cause
`not_configured`, `missing` or `refused` and list the variables they tried.

On Wayland the CLI then reads the compositor's registry globals. A compositor
that offers `wl_seat`, `zwp_virtual_keyboard_manager_v1` and
`zwlr_virtual_pointer_manager_v1` takes the wlroots route; any other compositor
takes the GNOME (Mutter) route. The compositor's name is never consulted, so
labwc, sway and other wlroots compositors are chosen by what they offer. The
wlroots route also uses `zwlr_screencopy_manager_v1`,
`zwlr_foreign_toplevel_manager_v1` and the ext toplevel capture protocols when
they are offered; see "wlroots compositors" below.

## X11 versus Wayland

| Capability | X11 | Wayland (GNOME) | Wayland (wlroots, labwc) |
|------------|-----|-----------------|--------------------------|
| Observation, semantic ref actions | AT-SPI, no tool | AT-SPI, no tool | AT-SPI, no tool |
| Keys, `press`, text keys | XTEST in process, live keymap | Mutter RemoteDesktop, keysyms | Virtual keyboard with a generated keymap; any Unicode character |
| App pointer: `hover`, `drag`, `mouse-*`, physical click, wheel | XTEST, `--headed` | None: `ACTION_NOT_SUPPORTED` with `physical_delivery_started: false` | Virtual pointer, `--headed`; coordinate commands work, ref aiming is limited (see below) |
| Window ops | EWMH on the X11 display | Xwayland clients only; native windows and un-minimize refused | `minimize`, `maximize`, `restore` through `zwlr_foreign_toplevel_manager_v1`, Xwayland included; `move-window` and `resize-window` refused |
| `focus-window`, headed scoped `press` | EWMH activation, then keys for `press` | Refused with `ACTION_NOT_SUPPORTED`, cause `wayland_session`, Xwayland clients included | Toplevel activation, verified by the compositor and the AT-SPI frame |
| Headless scoped `press` | Live display and frame focus guards, no activation | Live frame focus guard, no activation | Live frame focus guard, no activation |
| `screenshot` | Window or display; primary display by default | Portal consent; primary display by default, `--screen` picks a monitor | Screencopy, no consent; a window needs the toplevel capture protocol |
| `list-displays` | RandR | Mutter `DisplayConfig` | `wl_output` and `xdg_output` |
| Clipboard | Native selection owner | `wl-clipboard` | `wl-clipboard` |
| GNOME Shell ref click, popup opener | `--headed`, receiver proof from the shell's own tree | Ref click refused with `wayland_session`, popup opener refused as unproven; see shell reference | Not applicable |

X11 needs no tool beyond the binary and its clipboard helper. `xdotool`,
`ydotool`, `xclip` and `wmctrl` are not used. `wl-clipboard` is the one outside
tool, on Wayland only. AT-SPI actions need no input device on either server.

## Headed and headless

Ref actions are headless and semantic by default: the pointer and the active
window never change, and a headless action reports `semantic_api`. Pointer
commands (`hover`, `drag`, `double-click`, `triple-click`, `mouse-move`,
`mouse-click`, `mouse-wheel`) need global `--headed`, because every X11
synthetic pointer path moves the user's real cursor.

- Under `--headed`, ref `click`, `right-click` and `type` try physical input
  first. Physical success is `physical_synthetic` with `verified: false` (XTEST
  has no read-back).
- The adapter falls back to the semantic action only when the physical attempt
  returns `ACTION_NOT_SUPPORTED` or a geometry refusal (`check: geometry`) with
  `not_delivered`; `details.physical_fallback` records the skipped
  `physical_input` step and refusal. Focus, policy, timeout, uncertain and
  partial-delivery refusals never fall back.
- On GNOME Wayland the CLI's pointer gate runs before that fallback:
  ordinary-app headed pointer commands are refused at once with
  `ACTION_NOT_SUPPORTED`, cause `wayland_session` and
  `physical_delivery_started: false` (`click`, `hover`, `double-click`,
  `mouse-wheel`, `mouse-down` and `select` included). A headed `type` sends keys
  through `RemoteDesktop.type_text` once the Mutter capability probe succeeds.
  The wlroots route has a virtual pointer and does not refuse up front; see
  "wlroots compositors".
- Windows works the other way round (semantic first); do not carry that
  assumption across platforms.

A physical ref-addressed key needs the target frame `active` within a short
wait, else `check: window_active`. Headed `type` and ref-addressed key presses
also need the target, or its proven internal editor, `focused`: before each X11
text plan and, on Wayland, before the bootstrap Shift and each subsequent
key-down. The internal-editor proof needs an editable target, a same-owner hit,
a parent chain back to the target and a focused editor that advertises
`EditableText`.

Closing a GTK4 menu can leave the document unfocused even while its window is
active. GNOME 46's GTK4 bridge rejects AT-SPI `Component.GrabFocus` as
unsupported. If neither the target nor its proven internal editor is focused,
the tool cannot restore that document's focus automatically: focus the document
yourself, or use headless `type` through `EditableText`. A `target_focus`
refusal with `physical_delivery_started: false` sent no keys, and waiting does
not supply the missing focus action.

Headless semantic actions need no active frame: a headless `type` into a
background window works. A headless `press --app` to a background app on X11
fails with `display_active_window`. When headless `press --window-id` cannot
confirm the named window's exact focus, it returns `STALE_REF`,
`check: exact_window_focus`, `cause: exact_window_focus_unconfirmed`. Bring that
window forward, refresh `list-windows`, and retry. The active-frame gate is
necessary, not sufficient: while a popup menu holds the grab, keys reach the
menu even if the frame reads active, so key steps stay unverified.

Pointer forms aim at the centre of the ref's bounds, which the adapter withholds
where the position is not proved. Read the result with a fresh `get`.

Ref-addressed double and triple clicks prove the pointer receiver before every
press, so a window raised by the first click is seen before the second. The
checks share a 300 ms press-to-press budget, inside the 400 ms in which GTK, Qt
and GNOME read two presses as one gesture. A changed receiver, or a check that
outruns the budget (`details.kind: multi_click_window`), stops the gesture with
`ACTION_FAILED`; `details.delivered_clicks` counts the clicks already sent and
the disposition is `delivered_unverified`. Inspect the app before retrying.

`hover @ref` proves the receiver the same way before the move; a covered target
or an unproven receiver is refused (`ACTION_FAILED`, `not_delivered`, cursor
unmoved). `hover --xy` has no target element to prove.

## Keys and `press`

```bash
agent-desktop press ctrl+a
agent-desktop press return --app gnome-text-editor
```

- Key names: the macOS and Windows names (`return`, `escape`, `tab`, `space`,
  `home`, `pageup`, `f1`-`f24`), punctuation names (`comma`, `period`,
  `greater`, `leftbracket`) and X keysym names such as `Page_Up`. `delete` is
  Backspace; `forwarddelete` is the forward Delete key. A single character is
  its own key. An unknown name is `INVALID_ARGS`, `not_delivered`, and lists the
  accepted forms.
- A bare upper-case letter (`H`) is `shift+h` on every platform; with other
  modifiers the case is ignored (`ctrl+A` is `ctrl+a`).
- `meta`, `cmd`, `command`, `super` and `win` name the Super modifier in combos;
  for a bare key use `super` or `meta`, not `cmd`, `command` or `win`.
- `press` without `--app` or `--window-id` sends to the focused window. Headless
  `press --app` requires the target process to own the active window;
  `press --window-id` also requires the exact window's focus. A background or
  minimized target is refused. See `references/windows-and-lifecycle.md`.
- Keys carry the Shift level the live keymap needs. On X11 `press` takes only
  keys the active layout maps and the named keysyms above; on GNOME Wayland it
  takes printable ASCII, function and modifier keysyms without reading the
  layout. A character the layout cannot produce (`é`, `你`) is refused and not
  delivered (`INVALID_ARGS` on X11, `ACTION_NOT_SUPPORTED` with
  `character_outside_layout` on GNOME Wayland). On X11, `--headed type` sends
  such a character through spare keycodes; on GNOME Wayland it refuses it, so
  use headless `type` or `set-value` for accents and non-Latin text. On a
  wlroots compositor every character is one key of a generated keymap, so
  `press` and `--headed type` send `é` and `你` too.

### Blocked combos

`press` refuses these shortcuts with `POLICY_DENIED`, `not_delivered`, before
any key is sent, because they take the foreground, close a window or switch the
session. `--force` sends them: `alt+f4`, `alt+tab`, `alt+f2`, `meta+l`,
`meta+d`, `meta+tab`, a bare `super`, `ctrl+alt+t`, `ctrl+alt+delete`,
`ctrl+alt+forwarddelete`, `ctrl+alt+backspace`, `ctrl+alt+l`, `ctrl+alt+f1` to
`ctrl+alt+f12`. App shortcuts such as `ctrl+q` are sent. The list is in the
adapter, not in the CLI help; `agent-desktop press <combo>` shows the refusal
per combo.

## `type`

For literal newlines in Bash, set `TEXT_REF` to the qualified ref of a
multiline text field from the latest snapshot, then use headless typing:

```bash
agent-desktop type "$TEXT_REF" $'First line\nSecond line'
```

Empty text sends no keys, requests no focus on the target and reports
`not_delivered`; `--headed` can still activate the source window before the
empty insertion is dispatched.

- Headless `type @ref text` writes through `EditableText.InsertText`. Without
  `--timeout-ms`, `type` waits 5000 ms plus 30 ms per character; an explicit
  `--timeout-ms` replaces that.
- Under `--headed` it sends physical keys first (step `XTEST.type_text` on X11,
  `RemoteDesktop.type_text` on GNOME Wayland, `VirtualKeyboard.type_text` on
  wlroots, `physical_synthetic`; the result still
  reads the field back when it can) and falls back to `EditableText` only where
  keys cannot be sent. It proves focus on the field or its proven internal
  editor; focus elsewhere in the window does not authorize ref typing. Verify
  the value or application effect afterwards.
- A terminal's text node (`textfield` named `Terminal`, only `RightClick`) takes
  keys only under `--headed`; headless `type` there is `POLICY_DENIED`, check
  `supported_action`. Its value is null, so a ref cannot read terminal output:
  verify what a terminal command did through files or another app.
- X11 physical `type` refuses control characters, Tab and newline included,
  before sending anything, because those keys can move focus or submit a form
  inside a text burst. Use headless `type` or `set-value` for literal text with
  those characters, and an explicit `press tab` or `press return` for
  navigation or submission. On Wayland, `--headed type` sends a newline as
  `Return` and a tab as `Tab`, which can submit a form or move focus. Other
  control characters are refused before any key: with
  `character_outside_layout` on GNOME, with `control_character` on wlroots.
- X11 physical text checks the live keyboard state before each chunk. If that
  state is unsupported, the physical route refuses before that chunk's keys; the
  semantic fallback applies only if no key was sent, since earlier chunks report
  `delivered_unverified`.
- The first `type` into a fresh document changes the window title, but the ref
  from the snapshot still resolves: `get` on it reads the typed value. Take a
  fresh snapshot only when the UI you address has changed for another reason.

## GNOME Wayland keys

GNOME Wayland keys use one detected backend, `org.gnome.Mutter.RemoteDesktop` on the
session bus. Before creating a session or emitting an event, the adapter probes
the interface's typed `Version` property. A missing service or capability is
`PLATFORM_NOT_SUPPORTED`, `not_delivered`, `physical_delivery_started: false`;
another compositor never falls back to X11. A probe timeout is `TIMEOUT`. Calls
send keysyms and wait 150 ms for the compositor to drain them. A long-lived host
can retain the keyboard session for later deliveries; a new CLI process opens
its own. Ending a session releases its held keys.

- **Lead Shift.** A new session sends a lone Shift tap and waits 100 ms before
  the real keys so the compositor can attach its keyboard. It inserts no text,
  but a handler that watches bare Shift can observe it. Reusing a still-live
  session skips the lead tap.
- **Characters.** Only printable ASCII, function, modifier and vendor keysyms;
  anything else is `ACTION_NOT_SUPPORTED`, cause `character_outside_layout`.
  Set the value directly with `set-value` instead. On a non-US layout, a
  character your keymap holds can still be refused. A refusal is safe: nothing
  was sent.
- **App pointer.** AT-SPI gives no global window origin on Wayland, so ordinary
  app pointer commands cannot be aimed (see Headed and headless above). GNOME
  Shell ref clicks and popup openers refuse too; see
  `references/shell-tray-and-notifications.md`.
- **No active window.** On a Wayland session with no seat keyboard no window
  carries `ACTIVE`, so `focus-changed` waits time out.

## wlroots compositors

A compositor on the wlroots route (labwc, sway and others; see "How the display
server is chosen") gets keys and pointer input through virtual devices that the
CLI creates on the compositor's seat for each delivery. No D-Bus service, portal
or outside tool is involved. Destroying a device, or the CLI process dying,
makes the compositor release every key and button the device still holds.

- **Keys.** Each delivery uploads an XKB keymap generated for exactly the
  keysyms it sends. Every character, Latin-1 or any other Unicode, is one key,
  so nothing depends on the user's layout and `character_outside_layout` never
  occurs. Shift, Control, Alt and Super hold fixed keycodes and the modifier
  map, so a chord such as `ctrl+a` or `shift+b` carries exactly the modifiers
  you name. Steps are `VirtualKeyboard.press_key` and
  `VirtualKeyboard.type_text`, `physical_synthetic`.
- **Control characters.** `\n` is `Return` and `\t` is `Tab`. Any other control
  character refuses the whole text before the first key with
  `ACTION_NOT_SUPPORTED`, cause `control_character`,
  `physical_delivery_started: false`, `not_delivered`. Remove it, or use
  `set-value`.
- **Pointer at coordinates.** `mouse-move`, `mouse-click`, `mouse-wheel` and
  `drag` with explicit coordinates run under `--headed` through the virtual
  pointer, in the output layout that `list-displays` reports. Wheel clicks are
  discrete axis notches. Steps are `VirtualPointer.click` and
  `VirtualPointer.scroll`. The result is `delivered_unverified`: check the
  effect.
- **Pointer at a ref.** `--headed` ref `click`, `right-click`, `double-click`,
  `triple-click`, `hover`, `drag` and `scroll --headed` need the window's
  position in the output layout, which AT-SPI does not give on Wayland and no
  wlroots protocol tells a client. The CLI proves it one of two ways:
  - The compositor reports the window's toplevel active and maximized or
    fullscreen: its origin follows from the output geometry and costs no
    capture. `maximize --window-id <id>` then `focus-window --window-id <id>`
    puts a window in that state.
  - The compositor offers `ext_foreign_toplevel_image_capture_source_manager_v1`:
    the window's own pixels are copied and found in a copy of the output. If
    something else is drawn over the target point in three proofs taken 300 ms
    apart, the click is refused with the usual interceptor error ("Another
    element intercepts the pointer target", `check: receives_events`,
    interceptor role `window`); a single repaint, such as a hover highlight,
    does not count.

  Otherwise the action is refused in about 0.2 s before any pointer event,
  with no semantic fallback: `ACTION_FAILED`, `details.kind:
  "wlroots_window_origin_unproven"`, `window_id`, `retryable: false`,
  `physical_delivery_started: false`, disposition `not_delivered` (retry
  `safe`). The message is "The compositor does not reveal where this window is
  drawn, so no pointer event was sent" and the suggestion is "Run 'maximize
  --window-id <id>' and 'focus-window --window-id <id>', then retry; or run the
  action without --headed to act through accessibility; or click a point you
  located yourself with 'mouse-click --xy'". Xwayland windows take the same
  compositor path: their X11 position matches the screen, but X11 stacking
  cannot see native Wayland windows above them. The hit test ignores children
  that are neither `VISIBLE` nor `SHOWING`, such as the invisible full-window
  panel Chromium keeps beside its content. It also counts a point as reaching
  a target that lists no children when the node the point query names gives
  that target as its `Parent` and lies inside it: Chromium lists a page
  button as a leaf, but once it has hit-tested a point on the button it
  answers with the button's text node.
- **Focus.** `focus-window` and the focus step of headed ref actions activate
  the window through `zwlr_foreign_toplevel_manager_v1`; see
  `references/windows-and-lifecycle.md`. A fresh labwc seat that has never had
  a keyboard gives no window `ACTIVE`; `focus-window` attaches a virtual
  keyboard that sends no key so the frame can report it.
- **Cursor overlay.** Drawn when the compositor offers `zwlr_layer_shell_v1`
  (both labwc versions): a click-through arrow on the overlay layer, held by a
  per-session `agent-desktop` helper process. Its tip moves to each pointer
  point and to each ref target whose window place is proven; an unproven
  target is not marked. See the Cursor overlay section of `SKILL.md`.
- **Unavailable.** `key-down`, `key-up`, `mouse-down` and `mouse-up` fail closed
  as on every platform.

labwc 0.20.2 offers every protocol above. labwc 0.9.8 has no
`ext_foreign_toplevel_image_capture_source_manager_v1`, so window screenshots
refuse there and ref pointer aiming works only on an active maximized or
fullscreen window.

Each pointer delivery creates a virtual pointer and destroys it afterwards.
Xwayland (24.1.14, under both labwc versions) takes the first click sent to one
of its windows; every later click to an Xwayland window is lost, the command
still reports `delivered_unverified`, and Xwayland logs `pointer_handle_leave`
BUG errors. Native Wayland windows take every click. Read the effect after
each pointer command on an Xwayland window, and prefer semantic actions there.

## Wheel and scroll

```bash
agent-desktop --headed mouse-wheel --x 640 --y 400 --dy 5
```

- Positive `--dy` scrolls up, negative down; positive `--dx` right, negative
  left, on all three platforms. Write a negative value as `--dy -5` or
  `--dy=-5`.
- With no `--dy`, the default is three lines down when `--dx` is zero (explicit
  `--dx 0` included) and no vertical scroll when `--dx` is nonzero. Zero on both
  axes, or more than 1000 clicks on an axis, is `INVALID_ARGS`, `not_delivered`.
- The result carries `dy`, `dx`, `target`, `steps` and `disposition`, with no
  `scrolled` success flag. Wheel delivery is `delivered_unverified`: read the
  content afterwards.
- `scroll @ref --headed` sends wheel clicks at the centre of the target's
  bounds, only for a node with the `Scroll` capability. A wheel click is
  page-size dependent (about two rows on GTK, exactly three items on Qt), so
  `--amount` counts clicks, not rows. A `scrollarea` without a ref cannot take
  `scroll @ref`; use `scroll-to` on a child or `mouse-wheel` at coordinates.
  Each wheel burst uses click's pointer gate at that point: a hit-test result
  naming an interceptor refuses the burst; an unknown receiver refuses with
  `pointer_hit_unproven`. If earlier bursts were sent, the error reports their
  `delivered_clicks`, `delivered_unverified` and unsafe retry. Inspect the
  content before the next step.
- `scroll-to` scrolls through the bar on GTK3 and GTK4 and through `ScrollTo` on
  Chromium. It does not work on Qt: use `scroll --headed`. A container with no
  scrollable axis has no ref. If target visibility is unknown, `scroll-to`
  refuses before scrolling; equally ranked scroll bars refuse with
  `AMBIGUOUS_TARGET` before a value write. A scroll that moved the viewport
  without revealing the target is unsafe to retry: inspect the target and
  disposition first.

## Drag

`drag` picks up, moves along an interpolated path, holds still for a dwell, then
releases. The dwell is deliberate: a hovered folder or drop target can open
under the pointer, and any later motion lands inside it and copies instead of
moving. The default duration is 300 ms; the minimum is 100 ms, and a shorter
`--duration` is `INVALID_ARGS`, `not_delivered`, because a toolkit such as GTK
needs several motion events to start a drag. The default drop
delay on Linux is 150 ms; set `--drop-delay` only to give a slow drop target
time to arm. The result reports the gesture, not the outcome: check the effect
(a file moved, a list changed) with a fresh `snapshot`.

## Interrupted input

The X server keeps XTEST-pressed buttons when the client dies. The CLI journals
what it holds under `$XDG_RUNTIME_DIR/agent-desktop-input`, and the next call on
that display releases what a killed process left before it sends its own events.
If a button seems stuck after you killed a command, run any input command
with valid arguments, such as `agent-desktop --headed mouse-move --xy 640,400`,
to release it before the new input.
