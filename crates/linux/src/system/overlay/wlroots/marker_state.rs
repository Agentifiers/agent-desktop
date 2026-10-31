use agent_desktop_core::{CursorOverlayStyle, Point};

/// What one agent's marker should show. The compositor never reveals where
/// the pointer is, so a marker has no point until an action names one.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MarkerState {
    pub(crate) point: Option<Point>,
    pub(crate) style: CursorOverlayStyle,
    pub(crate) hidden: bool,
}

impl MarkerState {
    pub(crate) fn shown_at(&self) -> Option<&Point> {
        self.point.as_ref().filter(|_| !self.hidden)
    }
}
