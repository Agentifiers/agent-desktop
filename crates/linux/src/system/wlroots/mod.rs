//! Clients of the wlroots compositor protocols: the globals probe that picks
//! the route, the connection every wlroots read and request uses, outputs,
//! and output and toplevel capture.

pub(crate) mod capability;
pub(crate) mod captured_image;
pub(crate) mod link;
pub(crate) mod listed_toplevel;
pub(crate) mod output;
pub(crate) mod outputs;
pub(crate) mod pixel_layout;
pub(crate) mod screencopy;
pub(crate) mod shm;
pub(crate) mod toplevel_capture;
pub(crate) mod toplevel_match;
