//! Clients of the wlroots compositor protocols: the globals probe that picks
//! the route, the connection every wlroots read and request uses, outputs,
//! output and toplevel capture, and toplevel control.

pub(crate) mod capability;
pub(crate) mod captured_image;
pub(crate) mod foreign_toplevel;
pub(crate) mod link;
pub(crate) mod listed_toplevel;
pub(crate) mod output;
pub(crate) mod outputs;
pub(crate) mod pixel_layout;
pub(crate) mod screencopy;
pub(crate) mod shm;
pub(crate) mod toplevel_capture;
pub(crate) mod toplevel_flags;
pub(crate) mod toplevel_match;
pub(crate) mod toplevel_request;
pub(crate) mod toplevel_session;
pub(crate) mod toplevel_state;
