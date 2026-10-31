/// Where a wlroots compositor draws a frame's window coordinates. There no
/// toolkit's screen extents are a position, so a node is placed only by its
/// window rectangle moved to a proved origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutPlace {
    /// The layout position of the frame's window-coordinate origin.
    Proven((i32, i32)),
    /// Nothing the compositor offers proves where the frame is drawn.
    Unproven,
}
