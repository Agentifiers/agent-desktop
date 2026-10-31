//! Where an accessibility window's coordinate origin sits in the output
//! layout of a wlroots compositor, proved from pixels: the toplevel's own
//! surface is captured, found in the output's frame, and the accessibility
//! frame rectangle is tied to either the whole surface or its opaque part.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, Deadline, Rect};

use super::capability;
use super::captured_image::CapturedImage;
use super::output::Output;
use super::screencopy::capture_outputs;
use super::surface_locate::{SurfaceFix, key_black, locate, opaque_bounds, shown_share};
use super::toplevel_capture::ToplevelLister;
use super::toplevel_match::{ToplevelMatch, WindowIdentity, match_toplevel};
use crate::tree::Extents;

const SHOWN: f64 = 0.9;
const REUSE: Duration = Duration::from_millis(250);

/// A proved window origin with the pixels that proved it.
#[derive(Debug, Clone)]
pub(crate) struct WindowOrigin {
    /// The layout position of the window's accessibility coordinate origin.
    pub(crate) origin: (i32, i32),
    surface: CapturedImage,
    surface_at: (i32, i32),
    screen: (Output, CapturedImage),
}

/// Where the accessibility frame's coordinate origin sits inside the
/// captured surface. The frame's window rectangle must be the whole surface
/// (a toolkit that counts its client-side border, such as Chromium) or the
/// surface's opaque part (a toolkit that counts from inside its shadow, such
/// as GTK 4); any other size proves nothing.
pub(crate) fn window_offset(surface: &CapturedImage, frame: Extents) -> Option<(i32, i32)> {
    let size = |width: u32, height: u32| {
        i32::try_from(width).ok() == Some(frame.width)
            && i32::try_from(height).ok() == Some(frame.height)
    };
    if size(surface.width, surface.height) {
        return Some((frame.x.checked_neg()?, frame.y.checked_neg()?));
    }
    let (x, y, width, height) = opaque_bounds(surface)?;
    if !size(width, height) {
        return None;
    }
    Some((
        i32::try_from(x).ok()?.checked_sub(frame.x)?,
        i32::try_from(y).ok()?.checked_sub(frame.y)?,
    ))
}

impl WindowOrigin {
    /// Whether the output shows the window's own pixels across `rect`, a
    /// rectangle in layout coordinates: `Some(false)` when something else is
    /// drawn over it, `None` when the rectangle holds no opaque window pixel
    /// on this output.
    pub(crate) fn shows(&self, rect: Rect) -> Option<bool> {
        let (output, image) = &self.screen;
        let at = (
            i64::from(self.surface_at.0) - i64::from(output.x),
            i64::from(self.surface_at.1) - i64::from(output.y),
        );
        let local = (
            rect.x.floor() as i64 - i64::from(self.surface_at.0),
            rect.y.floor() as i64 - i64::from(self.surface_at.1),
            rect.width.ceil().max(1.0) as i64,
            rect.height.ceil().max(1.0) as i64,
        );
        shown_share(&self.surface, image, at, local).map(|share| share >= SHOWN)
    }
}

fn found_on(
    surface: &CapturedImage,
    screens: Vec<(Output, CapturedImage)>,
) -> Option<((i32, i32), (Output, CapturedImage))> {
    let mut found = None;
    for (output, image) in screens {
        match locate(surface, &image) {
            SurfaceFix::Found(at) => {
                if found.is_some() {
                    return None;
                }
                let layout = (output.x.checked_add(at.x)?, output.y.checked_add(at.y)?);
                found = Some((layout, (output, image)));
            }
            SurfaceFix::Ambiguous => return None,
            SurfaceFix::NotFound => {}
        }
    }
    found
}

/// Proves the origin of the window `identity` names, whose accessibility
/// frame reads `frame` in window coordinates. `None` when the compositor
/// cannot capture toplevels or outputs, the toplevel is not one unique match,
/// or the pixels do not place it.
pub(crate) fn prove(
    identity: WindowIdentity<'_>,
    frame: Extents,
    deadline: Deadline,
) -> Result<Option<WindowOrigin>, AdapterError> {
    let Some(capabilities) = capability::current() else {
        return Ok(None);
    };
    if !capabilities.toplevel_capture() || !capabilities.screencopy() {
        return Ok(None);
    }
    let mut lister = ToplevelLister::open(deadline)?;
    let toplevels = lister.toplevels();
    let pairs = toplevels
        .iter()
        .map(|toplevel| (toplevel.title.as_str(), toplevel.app_id.as_str()));
    let ToplevelMatch::Unique(index) = match_toplevel(pairs, identity) else {
        return Ok(None);
    };
    let surface = key_black(lister.capture(&toplevels[index].identifier, deadline)?);
    let Some(offset) = window_offset(&surface, frame) else {
        return Ok(None);
    };
    let screens = capture_outputs(|output| output.scale <= 1.0, deadline)?;
    let Some((surface_at, screen)) = found_on(&surface, screens) else {
        return Ok(None);
    };
    let origin = (
        surface_at.0.saturating_add(offset.0),
        surface_at.1.saturating_add(offset.1),
    );
    Ok(Some(WindowOrigin {
        origin,
        surface,
        surface_at,
        screen,
    }))
}

type Recent = Option<(String, Extents, Instant, WindowOrigin)>;

static RECENT: Mutex<Recent> = Mutex::new(None);

/// [`prove`], reusing a proof of the same window made within the last
/// quarter second, so the reads of one pointer check share one capture.
pub(crate) fn prove_recent(
    identity: WindowIdentity<'_>,
    frame: Extents,
    deadline: Deadline,
) -> Result<Option<WindowOrigin>, AdapterError> {
    if let Ok(recent) = RECENT.lock()
        && let Some((title, extents, when, origin)) = recent.as_ref()
        && title == identity.title
        && *extents == frame
        && when.elapsed() < REUSE
    {
        return Ok(Some(origin.clone()));
    }
    let proved = prove(identity, frame, deadline)?;
    if let (Some(origin), Ok(mut recent)) = (&proved, RECENT.lock()) {
        *recent = Some((
            identity.title.to_owned(),
            frame,
            Instant::now(),
            origin.clone(),
        ));
    }
    Ok(proved)
}

#[cfg(test)]
#[path = "window_origin_tests.rs"]
mod tests;
