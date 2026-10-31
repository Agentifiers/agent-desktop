//! The close flow on a wlroots compositor. The process's windows are the
//! toplevels `zwlr_foreign_toplevel_manager_v1` lists that match the titles
//! of its accessibility frames and its application names, and a close
//! request is that protocol's `close`. Everything else, the process and its
//! signals, is the inner world's.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::world::World;
use crate::system::process::liveness::Liveness;
use crate::system::process::process_lookup::Lookup;
use crate::system::signal::Signal;
use crate::system::signal::signal_sent::Sent;
use crate::system::wlroots::toplevel_match::{ToplevelMatch, WindowIdentity};
use crate::system::wlroots::toplevel_request::ToplevelRequest;
use crate::system::wlroots::toplevel_session::{ToplevelSession, pick};
use crate::system::wlroots::toplevel_state::ToplevelState;
use crate::system::x11::control::ControlRead;
use crate::system::x11::control::control_error::ControlError;
use crate::system::x11::window::toplevel::Toplevel;
use crate::system::x11::window::window_kind::WindowKind;

const LATE_READ_MS: u64 = 300;

/// The toplevels among `toplevels` that are the windows titled `frames` of
/// an application known by `names`, or the number of toplevels one title
/// matches when they cannot be told apart. A frame without a toplevel is
/// not shown and has nothing to close.
pub(super) fn owned_toplevels(
    toplevels: &[ToplevelState],
    frames: &[String],
    names: &[String],
) -> Result<Vec<Toplevel>, usize> {
    let mut owned: Vec<Toplevel> = Vec::new();
    for title in frames {
        let identity = WindowIdentity {
            title,
            app_names: names,
        };
        match pick(toplevels, identity) {
            Ok(found) if owned.iter().all(|known| known.id != found.key) => {
                owned.push(Toplevel {
                    id: found.key,
                    title: found.title,
                    kind: if found.has_parent {
                        WindowKind::Dialog
                    } else {
                        WindowKind::Normal
                    },
                    transient: found.has_parent,
                });
            }
            Ok(_) | Err(ToplevelMatch::Missing | ToplevelMatch::Unique(_)) => {}
            Err(ToplevelMatch::Ambiguous(count)) => return Err(count),
        }
    }
    Ok(owned)
}

/// The close world of a wlroots session: `inner` for the process, `session`
/// for its toplevels, and `frames` for the titles of its accessibility
/// frames, read again on every look.
pub(super) struct ToplevelWorld<W, S, F> {
    pub(super) inner: W,
    pub(super) session: RefCell<S>,
    pub(super) names: Vec<String>,
    pub(super) frames: F,
    pub(super) deadline: Deadline,
    pub(super) ambiguous: Cell<Option<usize>>,
}

impl<W, S, F> ToplevelWorld<W, S, F>
where
    W: World,
    S: ToplevelSession,
    F: Fn() -> Option<Vec<String>>,
{
    fn read(&self, deadline: Deadline) -> ControlRead<Vec<Toplevel>> {
        let mut session = self.session.borrow_mut();
        if session.refresh(deadline).is_err() {
            return ControlRead::Unavailable;
        }
        let Some(frames) = (self.frames)() else {
            return ControlRead::Unavailable;
        };
        match owned_toplevels(&session.toplevels(), &frames, &self.names) {
            Ok(owned) => {
                self.ambiguous.set(None);
                ControlRead::Ready(owned)
            }
            Err(count) => {
                self.ambiguous.set(Some(count));
                ControlRead::Ready(Vec::new())
            }
        }
    }
}

impl<W, S, F> World for ToplevelWorld<W, S, F>
where
    W: World,
    S: ToplevelSession,
    F: Fn() -> Option<Vec<String>>,
{
    fn is_protected(&self) -> Result<bool, AdapterError> {
        self.inner.is_protected()
    }

    fn facts(&self) -> Lookup {
        self.inner.facts()
    }

    fn liveness(&self) -> Liveness {
        self.inner.liveness()
    }

    fn elapsed(&self) -> Duration {
        self.inner.elapsed()
    }

    fn remaining(&self) -> Duration {
        self.inner.remaining()
    }

    fn pause(&self) {
        self.inner.pause();
    }

    fn toplevels(&self) -> ControlRead<Vec<Toplevel>> {
        self.read(self.deadline)
    }

    fn toplevels_late(&self) -> ControlRead<Vec<Toplevel>> {
        match Deadline::detached_after(LATE_READ_MS) {
            Ok(deadline) => self.read(deadline),
            Err(_) => ControlRead::Unavailable,
        }
    }

    fn nested_dialogs(&self) -> Vec<String> {
        self.inner.nested_dialogs()
    }

    fn request_close(&self, window: u32) -> Result<(), ControlError> {
        self.session
            .borrow_mut()
            .send(window, ToplevelRequest::Close)
            .map_err(|error| {
                if error.code == ErrorCode::WindowNotFound {
                    ControlError::BadWindow
                } else {
                    ControlError::SendFailed
                }
            })
    }

    fn signal(&self, signal: Signal) -> Result<Sent, AdapterError> {
        self.inner.signal(signal)
    }

    fn timeout_error(&self) -> AdapterError {
        self.inner.timeout_error()
    }

    fn on_wayland(&self) -> bool {
        false
    }

    fn ambiguous(&self) -> Option<usize> {
        self.ambiguous.get()
    }
}

#[cfg(test)]
#[path = "toplevel_world_tests.rs"]
mod tests;
