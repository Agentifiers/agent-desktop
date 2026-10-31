use agent_desktop_core::{AdapterError, Deadline};

use super::toplevel_source::ToplevelSource;

pub(crate) type OpenToplevels<'a> =
    &'a dyn Fn(Deadline) -> Result<Box<dyn ToplevelSource>, AdapterError>;

/// How a window is captured on a wlroots compositor that captures single
/// toplevels: the toplevel list to open, and every name the application of
/// a process is known by, which separates toplevels that share a title.
#[derive(Clone, Copy)]
pub(crate) struct ToplevelRoute<'a> {
    pub(crate) open: OpenToplevels<'a>,
    pub(crate) app_names: fn(u32) -> Vec<String>,
}
