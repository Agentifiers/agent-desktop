//! What draws the session's markers. The helper's compositor stage is the
//! real one; tests record the calls.

use agent_desktop_core::{AdapterError, CursorOverlayStyle, Point};

/// One agent's marker; `None` is the session's own cursor.
pub(crate) type AgentKey = Option<String>;

pub(crate) trait Canvas {
    /// Draws `agent`'s marker with its tip on `point`, in layout coordinates.
    fn show(
        &mut self,
        agent: &AgentKey,
        point: &Point,
        style: &CursorOverlayStyle,
    ) -> Result<(), AdapterError>;

    /// Takes `agent`'s marker off the screen.
    fn hide(&mut self, agent: &AgentKey) -> Result<(), AdapterError>;

    /// Returns once the compositor has applied every change sent before.
    fn settle(&mut self) -> Result<(), AdapterError>;
}
