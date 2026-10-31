use agent_desktop_core::{AdapterError, Deadline};

/// One compositor keyboard session. The calls are the steps of a delivery in
/// the order a caller makes them; `stop` ends whatever `create` made.
pub(crate) trait KeySession {
    fn create(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
    fn start(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
    fn keysym(&mut self, keysym: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError>;
    fn stop(&mut self, deadline: Deadline) -> Result<(), AdapterError>;

    /// Whether the session was created and started by an earlier delivery and
    /// still holds its keyboard, so a delivery skips create, start and lead.
    fn resumed(&self) -> bool {
        false
    }

    /// Whether a delivery that succeeded leaves the session open for the next.
    fn retain(&self) -> bool {
        false
    }

    /// Whether a new session taps a lone Shift before the real keys so the
    /// compositor attaches its keyboard. Mutter needs the tap; a virtual
    /// keyboard whose keymap is uploaded with the device does not.
    fn leads_with_shift(&self) -> bool {
        true
    }
}
