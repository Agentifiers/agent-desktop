//! The seam between a window capture on a wlroots compositor and the
//! compositor's toplevel list, so the capture rules run against a fake.

use agent_desktop_core::{AdapterError, Deadline};

use crate::system::wlroots::captured_image::CapturedImage;
use crate::system::wlroots::listed_toplevel::ListedToplevel;
use crate::system::wlroots::toplevel_capture::ToplevelLister;

/// One open view of the compositor's toplevels that can capture each.
pub(crate) trait ToplevelSource {
    /// The toplevels still open, in the order the compositor announced them.
    fn listed(&self) -> Vec<ListedToplevel>;

    /// The pixels of the toplevel named `identifier`.
    fn copy(&mut self, identifier: &str, deadline: Deadline)
    -> Result<CapturedImage, AdapterError>;
}

impl ToplevelSource for ToplevelLister {
    fn listed(&self) -> Vec<ListedToplevel> {
        self.toplevels()
    }

    fn copy(
        &mut self,
        identifier: &str,
        deadline: Deadline,
    ) -> Result<CapturedImage, AdapterError> {
        self.capture(identifier, deadline)
    }
}

/// Opens the compositor's toplevel list.
pub(crate) fn open(deadline: Deadline) -> Result<Box<dyn ToplevelSource>, AdapterError> {
    Ok(Box::new(ToplevelLister::open(deadline)?))
}
