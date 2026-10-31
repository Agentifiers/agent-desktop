use super::pointer_read::PointerRead;
use super::sink_error::SinkError;
use crate::input::keyboard::keymap::Keymap;
use crate::input::keyboard::keymap_edit::KeymapEdit;
use agent_desktop_core::AdapterError;

/// Where synthesized input steps are delivered: XTEST on an X11 display, or the wlroots virtual
/// keyboard and pointer on a wlroots Wayland compositor.
pub(crate) trait InputSink {
    fn key(&mut self, code: u8, down: bool) -> Result<(), SinkError>;
    fn button(&mut self, code: u8, down: bool) -> Result<(), SinkError>;
    fn move_to(&mut self, x: i16, y: i16) -> Result<(), SinkError>;
    fn round_trip(&mut self) -> Result<(), SinkError>;
    fn pointer(&mut self) -> Result<PointerRead, SinkError>;
    fn keys_down(&mut self) -> Result<[u8; 32], SinkError>;
    fn keymap(&mut self) -> Result<Keymap, SinkError>;
    fn ensure_text_state(&mut self) -> Result<(), AdapterError>;
    fn edit_keymap(&mut self, edit: &KeymapEdit) -> Result<(), SinkError>;
    fn root_size(&mut self) -> Result<(u16, u16), SinkError>;

    /// Whether the server can be asked which keys and buttons are still
    /// down after a call. A sink without that read skips the leftover check
    /// and relies on its release guard alone.
    fn reads_held_input(&self) -> bool {
        true
    }
}
