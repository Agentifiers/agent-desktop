use agent_desktop_core::{CURSOR_HIGHLIGHT_HOLD_MS, CursorMotion, Point, Rect};

use super::look::schedule::highlight_progress;

/// The flourish after an action landed: the click ripple and the highlight
/// around the target, played after the action was answered, as on Windows.
pub(crate) struct Effect {
    pub(crate) started_ms: u64,
    pub(crate) motion: CursorMotion,
    pub(crate) target: Option<Rect>,
}

impl Effect {
    pub(crate) fn at(point: &Point, click: bool, ripple: bool, started_ms: u64) -> Self {
        Self {
            started_ms,
            motion: CursorMotion::new(point.clone(), point.clone())
                .with_impact(click)
                .with_ripple(ripple),
            target: None,
        }
    }

    /// The ripple phase and highlight opacity at `now_ms`, or `None` once the
    /// flourish is over.
    pub(crate) fn sample(&self, now_ms: u64) -> Option<(f64, f64)> {
        let elapsed = now_ms.saturating_sub(self.started_ms);
        if elapsed >= self.motion.total_ms().max(CURSOR_HIGHLIGHT_HOLD_MS) {
            return None;
        }
        Some((
            self.motion.pose(elapsed).ripple,
            highlight_progress(elapsed, CURSOR_HIGHLIGHT_HOLD_MS),
        ))
    }
}
