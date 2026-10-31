use agent_desktop_core::{CursorMotionProfile, CursorOverlayStyle, Point};

use super::effect::Effect;
use super::look::reveal::Reveal;

/// What one agent's cursor shows. The compositor never reveals where the
/// pointer is, so a cursor has no pose until an action names one.
pub(crate) struct Presentation {
    pub(crate) pose: Option<Point>,
    pub(crate) style: CursorOverlayStyle,
    pub(crate) gait: (CursorMotionProfile, u64),
    pub(crate) label: Option<String>,
    pub(crate) reveal: Reveal,
    pub(crate) hidden: bool,
    pub(crate) effect: Option<Effect>,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            pose: None,
            style: CursorOverlayStyle::default(),
            gait: (CursorMotionProfile::default(), 0),
            label: None,
            reveal: Reveal::Settled,
            hidden: false,
            effect: None,
        }
    }
}
