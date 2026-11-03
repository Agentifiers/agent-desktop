---
module: crates/linux/src/tree/placement/page_offset.rs, crates/linux/src/tree/placement/reported.rs
date: 2026-11-02
problem_type: integration_issue
tags: [linux, wayland, wlroots, labwc, gtk4, atspi, pointer, cursor-overlay]
---

# GTK 4 stack pages count their offset twice

## Symptom

On labwc, with a maximized GNOME Calculator, a headless `click` on the `7`
ref drew the cursor overlay's target outline and dart down and to the right
of the button. grim showed the button at (302, 520, 132, 44) on a 1280x720
output and the outline framing (314, 532, 132, 44). The offset is the same in
logical pixels at every output scale (1, 1.5 and 1.601562), and a headed
click aimed at the same wrong point; it still entered the digit because the
point stayed inside the button.

## Cause

GTK 4 computes an accessible's window coordinates by adding up the bounds of
the accessible and every accessible parent
(`gtk_at_spi_translate_coordinates_from_accessible`), each bound measured in
its parent widget's coordinates. A `GtkStackPage` is an accessible that is
not a widget: `gtk_stack_page_accessible_get_bounds` answers its child's
bounds in the stack, and the child answers the same bounds again. So every
node inside a stack page reads the page's offset twice. libadwaita's
`AdwViewStackPage` and `AdwTabPage` do the same. In Calculator the keypad
page sits at (12, 12) in its stack, so every keypad button reads 12 px too
far right and down. GTK's own `GetAccessibleAtPoint` uses the same sum, so a
point query stays consistent with the wrong rectangles.

## Fix

On the wlroots route, before a node's window rectangle is moved to its
window's proved origin, its accessible parents are read up to the frame
(`GetExtents` in parent coordinates and the child list). A page is an
ancestor that lists one child and reads that child's parent-coordinate
rectangle as its own; no ordinary container can, because a child at the
container's own offset and full size would overflow it. Each page's offset
is subtracted once. The placed hit test still asks the toolkit in its own
(doubled) coordinates. If the parents cannot be read the window counts as
unproven and the action is refused with `wlroots_window_origin_unproven`.

## Verification

`labwc.cursor_overlay_aligned_calculator` and its fractional variant read the
button from its own fill in grim and require the outline within 1 px and the
dart within 1.5 px of a reference dart drawn at the true centre. Both pass on
labwc 0.9.8 and 0.20.2; before the fix the outline and dart were 12 px off.

## Known limit

X11 and Mutter still place GTK 4 nodes from the toolkit's window extents and
keep the doubled offset.
