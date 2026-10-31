//! What shows the session's frames. The helper's compositor stage is the
//! real one; tests record the calls.

use agent_desktop_core::{AdapterError, Point, Rect};

use super::look::raster::Surface;

/// One agent's cursor; `None` is the session's own.
pub(crate) type AgentKey = Option<String>;

pub(crate) trait Canvas {
    /// The output rectangle, in layout coordinates, that holds `point`.
    fn screen_at(&self, point: &Point) -> Option<Rect>;

    /// Shows `surface` with its top-left corner at `origin` for `agent`,
    /// on the output that holds the cursor's `tip`.
    fn present(
        &mut self,
        agent: &AgentKey,
        tip: &Point,
        frame: (&Point, &Surface),
    ) -> Result<(), AdapterError>;

    /// Takes `agent`'s frame off the screen.
    fn hide(&mut self, agent: &AgentKey) -> Result<(), AdapterError>;

    /// Returns once the compositor has applied every change sent before.
    fn settle(&mut self) -> Result<(), AdapterError>;
}
