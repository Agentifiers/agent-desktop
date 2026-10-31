//! Keyboard and pointer input on a wlroots compositor through the
//! `zwp_virtual_keyboard_v1` and `zwlr_virtual_pointer_v1` protocols.

pub(crate) mod key_device;
pub(crate) mod keymap_layout;
pub(crate) mod keysym_route;
pub(crate) mod plan_route;
pub(crate) mod pointer_device;
pub(crate) mod virtual_devices;
pub(crate) mod virtual_key_session;
pub(crate) mod wlroots_sink;

#[cfg(test)]
mod fake_devices;
