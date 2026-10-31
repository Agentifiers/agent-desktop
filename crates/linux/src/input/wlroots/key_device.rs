use agent_desktop_core::{AdapterError, Deadline};

/// A virtual keyboard on the compositor's seat. Requests are queued; `sync`
/// sends them and waits until the compositor has handled them.
pub(crate) trait KeyDevice {
    fn upload_keymap(&mut self, text: &str) -> Result<(), AdapterError>;
    fn key(&mut self, evdev: u32, down: bool) -> Result<(), AdapterError>;
    fn modifiers(&mut self, depressed: u32) -> Result<(), AdapterError>;
    fn sync(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
    fn close(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
}
