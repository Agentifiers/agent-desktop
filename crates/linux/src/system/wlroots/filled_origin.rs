//! Where a maximized or fullscreen toplevel's accessibility origin sits,
//! read from the compositor's own state instead of from pixels. Such a
//! toplevel's surface fills the usable area of its output, so its frame lies
//! flush with the output's left, right and bottom edges, and a title bar the
//! compositor draws is the only thing above it. The frame's window rectangle
//! must be as wide as the output, so nothing else can explain a gap; the gap
//! left above it must be a plausible title bar, or none for a fullscreen
//! toplevel. A panel that reserves the bottom of the output would shift the
//! frame without the compositor saying so; the headless desktops this route
//! drives have none.

use super::output::Output;
use super::toplevel_state::ToplevelState;
use crate::tree::Extents;

/// The tallest title bar a compositor is taken to draw above a maximized
/// toplevel.
const TALLEST_TITLE_BAR: i32 = 64;

/// The layout position of the window-coordinate origin of `frame`, the
/// accessibility frame's rectangle in window coordinates, when `toplevel` is
/// maximized or fullscreen on one output whose layout position is known and
/// the sizes leave one answer. `None` otherwise.
pub(crate) fn derive(
    toplevel: &ToplevelState,
    outputs: &[Output],
    frame: Extents,
) -> Option<(i32, i32)> {
    let flags = toplevel.flags;
    if flags.minimized || !(flags.maximized || flags.fullscreen) {
        return None;
    }
    let output = match (toplevel.outputs.as_slice(), outputs) {
        ([name], _) => outputs.iter().find(|output| output.name == *name)?,
        ([], [only]) => only,
        _ => return None,
    };
    if !output.logical {
        return None;
    }
    let area = output.rect();
    if f64::from(frame.width) != area.width {
        return None;
    }
    let title_bar = (area.height as i64).checked_sub(i64::from(frame.height))?;
    let tallest = if flags.fullscreen {
        0
    } else {
        i64::from(TALLEST_TITLE_BAR)
    };
    if !(0..=tallest).contains(&title_bar) {
        return None;
    }
    let top = output.y.checked_add(i32::try_from(title_bar).ok()?)?;
    Some((output.x.checked_sub(frame.x)?, top.checked_sub(frame.y)?))
}

#[cfg(test)]
#[path = "filled_origin_tests.rs"]
mod tests;
