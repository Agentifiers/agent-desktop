use agent_desktop_core::{AdapterError, Deadline, ImageBuffer, ScreenshotTarget};

use super::toplevel_route::ToplevelRoute;
use crate::system::wlroots::captured_image::CapturedImage;
use crate::system::wlroots::output::Output;

pub(crate) type PortalCapture<'a> =
    &'a dyn Fn(&ScreenshotTarget, Deadline) -> Result<ImageBuffer, AdapterError>;

/// Copies the output named by its name, with the output as the copy saw it.
pub(crate) type CopyOutput<'a> =
    &'a dyn Fn(&str, Deadline) -> Result<(Output, CapturedImage), AdapterError>;

/// How a Wayland session captures: the desktop portal, and on a wlroots
/// compositor the output copy and the single-toplevel capture where the
/// compositor offers them. Mutter's route only ever uses the portal.
#[derive(Clone, Copy)]
pub(crate) struct WaylandCapture<'a> {
    pub(crate) portal: PortalCapture<'a>,
    pub(crate) outputs: Option<CopyOutput<'a>>,
    pub(crate) toplevels: Option<ToplevelRoute<'a>>,
}
