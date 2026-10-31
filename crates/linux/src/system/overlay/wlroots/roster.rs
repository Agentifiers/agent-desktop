//! The markers one session's helper draws, changed by the controls the X11
//! overlay answers and drawn the way it draws them: an enable or a present
//! creates the agent's marker, a present moves its tip to the destination
//! and shows it, hide and show without an agent reach every marker of the
//! session, and disable ends them all.

use std::collections::BTreeMap;

use agent_desktop_core::{AdapterError, CursorOverlayControl};

use super::canvas::{AgentKey, Canvas};
use super::marker_state::MarkerState;

#[derive(Debug, Default)]
pub(crate) struct Roster {
    markers: BTreeMap<AgentKey, MarkerState>,
    concealers: usize,
}

impl Roster {
    /// Applies `control`; answers `false` once the session's overlay is
    /// disabled: every marker is hidden and nothing may be drawn again.
    pub(crate) fn apply(&mut self, control: &CursorOverlayControl) -> bool {
        let agent = control.agent_id().map(str::to_owned);
        match control {
            CursorOverlayControl::Disable { .. } => {
                for marker in self.markers.values_mut() {
                    marker.hidden = true;
                }
                return false;
            }
            CursorOverlayControl::Hide { .. } | CursorOverlayControl::Show { .. } => {
                let hidden = control.is_hide();
                for (key, marker) in &mut self.markers {
                    if agent.is_none() || *key == agent {
                        marker.hidden = hidden;
                    }
                }
            }
            CursorOverlayControl::Enable { style, .. } => {
                let marker = self.markers.entry(agent).or_default();
                marker.style = style.clone();
                marker.hidden = false;
            }
            CursorOverlayControl::Present {
                instruction, style, ..
            } => {
                let marker = self.markers.entry(agent).or_default();
                marker.point = Some(instruction.destination().clone());
                marker.style = style.clone();
                marker.hidden = false;
            }
        }
        true
    }

    /// Keeps every marker off the screen until each conceal is revealed.
    pub(crate) fn conceal(&mut self) {
        self.concealers += 1;
    }

    pub(crate) fn reveal(&mut self) {
        self.concealers = self.concealers.saturating_sub(1);
    }

    #[cfg(test)]
    pub(crate) fn marker(&self, agent: &AgentKey) -> Option<&MarkerState> {
        self.markers.get(agent)
    }

    /// Brings `canvas` to the markers' state and waits until the compositor
    /// has applied it.
    pub(crate) fn render(&self, canvas: &mut impl Canvas) -> Result<(), AdapterError> {
        for (agent, marker) in &self.markers {
            match marker.shown_at().filter(|_| self.concealers == 0) {
                Some(point) => canvas.show(agent, point, &marker.style)?,
                None => canvas.hide(agent)?,
            }
        }
        canvas.settle()
    }
}

#[cfg(test)]
#[path = "roster_tests.rs"]
mod tests;
