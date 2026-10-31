//! Where an accessibility window is drawn on a wlroots compositor, so a
//! pointer can be aimed into it. No wlroots protocol tells a client where
//! another client's window sits; two proofs reach it. A maximized or
//! fullscreen toplevel the compositor reports active fills its output above
//! every ordinary window, so its place follows from the output alone and
//! costs no capture. Any other toplevel is placed from its own pixels, when
//! the compositor can capture one toplevel. Without either proof the window
//! has no place.

use agent_desktop_core::{AdapterError, Deadline, Rect};

use super::capability;
use super::filled_origin::derive;
use super::foreign_toplevel::ForeignToplevels;
use super::output::Output;
use super::toplevel_match::WindowIdentity;
use super::toplevel_session::{ToplevelSession, pick};
use super::toplevel_state::ToplevelState;
use super::window_origin::{WindowOrigin, prove_recent};
use crate::tree::Extents;

/// A proved place of a window: where its accessibility coordinates start in
/// the output layout, and whether its own pixels are what a rectangle shows.
pub(crate) trait Placed {
    /// The layout position of the window's accessibility coordinate origin.
    fn origin(&self) -> (i32, i32);
    /// Whether the window's own pixels are drawn across `rect`, a rectangle
    /// in layout coordinates: `Some(false)` when something else covers it,
    /// `None` when nothing tells.
    fn shows(&self, rect: Rect) -> Option<bool>;
}

#[derive(Debug, Clone)]
pub(crate) enum WindowPlace {
    /// The active maximized or fullscreen toplevel: it fills its output and
    /// the compositor stacks the window it activated above the others.
    Filled((i32, i32)),
    /// A toplevel whose own pixels were found on an output.
    Pixels(WindowOrigin),
}

impl Placed for WindowPlace {
    fn origin(&self) -> (i32, i32) {
        match self {
            Self::Filled(origin) => *origin,
            Self::Pixels(proof) => proof.origin,
        }
    }

    fn shows(&self, rect: Rect) -> Option<bool> {
        match self {
            Self::Filled(_) => Some(true),
            Self::Pixels(proof) => proof.shows(rect),
        }
    }
}

/// The first proof that holds for the window `identity` names, whose
/// accessibility frame reads `frame` in window coordinates: the output an
/// active maximized or fullscreen toplevel among `toplevels` fills, then the
/// pixels `pixels` proves.
pub(crate) fn choose(
    toplevels: &[ToplevelState],
    outputs: &[Output],
    identity: WindowIdentity<'_>,
    frame: Extents,
    pixels: impl FnOnce() -> Result<Option<WindowOrigin>, AdapterError>,
) -> Result<Option<WindowPlace>, AdapterError> {
    if let Ok(toplevel) = pick(toplevels, identity)
        && toplevel.flags.activated
        && let Some(origin) = derive(&toplevel, outputs, frame)
    {
        return Ok(Some(WindowPlace::Filled(origin)));
    }
    Ok(pixels()?.map(WindowPlace::Pixels))
}

/// [`choose`] over the session's compositor.
pub(crate) fn locate(
    identity: WindowIdentity<'_>,
    frame: Extents,
    deadline: Deadline,
) -> Result<Option<WindowPlace>, AdapterError> {
    let Some(capabilities) = capability::current() else {
        return Ok(None);
    };
    let (toplevels, outputs) = if capabilities.toplevel_control() {
        let session = ForeignToplevels::open(deadline)?;
        (session.toplevels(), session.outputs().to_vec())
    } else {
        (Vec::new(), Vec::new())
    };
    choose(&toplevels, &outputs, identity, frame, || {
        prove_recent(identity, frame, deadline)
    })
}

#[cfg(test)]
#[path = "window_place_tests.rs"]
mod tests;
