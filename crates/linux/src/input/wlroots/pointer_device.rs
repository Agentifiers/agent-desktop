use agent_desktop_core::{AdapterError, Deadline, Rect};

/// A virtual pointer on the compositor's seat, aimed in layout coordinates.
/// Requests are queued; `sync` sends them and waits until the compositor has
/// handled them.
pub(crate) trait PointerDevice {
    /// The output layout the absolute motion is expressed in.
    fn layout(&self) -> Option<Rect>;
    fn motion(&mut self, x: f64, y: f64) -> Result<(), AdapterError>;
    fn button(&mut self, button: u32, down: bool) -> Result<(), AdapterError>;
    /// Turns the wheel `steps` notches; positive is down or right.
    fn wheel(&mut self, vertical: bool, steps: i32) -> Result<(), AdapterError>;
    fn sync(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
}
