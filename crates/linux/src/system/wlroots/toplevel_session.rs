//! What window control reads and asks of the compositor's toplevels, so the
//! focus, close and state flows can be driven by a fixed desktop.

use agent_desktop_core::{AdapterError, Deadline};

use super::toplevel_match::{ToplevelMatch, WindowIdentity, match_toplevel};
use super::toplevel_request::ToplevelRequest;
use super::toplevel_state::ToplevelState;

pub(crate) trait ToplevelSession {
    /// The toplevels that are open, as of the events dispatched so far.
    fn toplevels(&self) -> Vec<ToplevelState>;
    /// Sends `request` to the toplevel `key` and flushes it to the
    /// compositor.
    fn send(&mut self, key: u32, request: ToplevelRequest) -> Result<(), AdapterError>;
    /// Dispatches events until the toplevel `key` reports a `done` whose
    /// state `wanted` accepts, closes, or `deadline` passes. Answers the
    /// toplevel as last reported, or `None` once it closed.
    fn settle(
        &mut self,
        key: u32,
        wanted: &dyn Fn(&ToplevelState) -> bool,
        deadline: Deadline,
    ) -> Result<Option<ToplevelState>, AdapterError>;
    /// Dispatches every event the compositor sent before a round trip.
    fn refresh(&mut self, deadline: Deadline) -> Result<(), AdapterError>;
    /// Whether the seat has a keyboard. Without one no client holds keyboard
    /// focus, so no window reports itself active however the compositor
    /// activated it.
    fn seat_has_keyboard(&self) -> bool;
}

impl<T: ToplevelSession> ToplevelSession for &mut T {
    fn toplevels(&self) -> Vec<ToplevelState> {
        (**self).toplevels()
    }

    fn send(&mut self, key: u32, request: ToplevelRequest) -> Result<(), AdapterError> {
        (**self).send(key, request)
    }

    fn settle(
        &mut self,
        key: u32,
        wanted: &dyn Fn(&ToplevelState) -> bool,
        deadline: Deadline,
    ) -> Result<Option<ToplevelState>, AdapterError> {
        (**self).settle(key, wanted, deadline)
    }

    fn refresh(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        (**self).refresh(deadline)
    }

    fn seat_has_keyboard(&self) -> bool {
        (**self).seat_has_keyboard()
    }
}

/// The one toplevel among `toplevels` that is the window `identity`
/// describes, or the match that names none.
pub(crate) fn pick(
    toplevels: &[ToplevelState],
    identity: WindowIdentity<'_>,
) -> Result<ToplevelState, ToplevelMatch> {
    let found = match_toplevel(
        toplevels
            .iter()
            .map(|toplevel| (toplevel.title.as_str(), toplevel.app_id.as_str())),
        identity,
    );
    match found {
        ToplevelMatch::Unique(index) => toplevels.get(index).cloned().ok_or(ToplevelMatch::Missing),
        other => Err(other),
    }
}

#[cfg(test)]
#[path = "toplevel_session_fake.rs"]
pub(crate) mod fake;
