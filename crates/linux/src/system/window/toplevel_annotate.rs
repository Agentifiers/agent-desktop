//! What a wlroots compositor adds to the window listing. A Wayland toolkit
//! is never told its window was minimized, and without a keyboard on the
//! seat no frame reports itself active, so the toplevel states
//! `zwlr_foreign_toplevel_manager_v1` reports fill those two facts. No
//! wlroots protocol gives a toplevel's position, so bounds stay as the
//! toolkit reported them.

use std::time::Duration;

use agent_desktop_core::{Deadline, WindowInfo};

use crate::system::display::route;
use crate::system::registry::listed_window::ListedWindow;
use crate::system::wlroots::foreign_toplevel::ForeignToplevels;
use crate::system::wlroots::toplevel_match::{WindowIdentity, app_names};
use crate::system::wlroots::toplevel_session::{ToplevelSession, pick};
use crate::system::wlroots::toplevel_state::ToplevelState;

const COMPOSITOR_READ: Duration = Duration::from_millis(500);

/// Marks each window whose toplevel the compositor reports minimized as
/// minimized and not visible, and, when no window is focused yet, the one
/// whose toplevel the compositor reports activated as focused. A window
/// that matches no toplevel, or several, keeps what the toolkit said.
pub(super) fn annotate(
    windows: &mut [&mut WindowInfo],
    toplevels: &[ToplevelState],
    names: impl Fn(u32) -> Vec<String>,
) {
    let any_focused = windows.iter().any(|window| window.state.is_focused);
    for window in windows.iter_mut() {
        let names = names(window.pid.get());
        let identity = WindowIdentity {
            title: &window.title,
            app_names: &names,
        };
        let Ok(toplevel) = pick(toplevels, identity) else {
            continue;
        };
        window.state.minimized = Some(toplevel.flags.minimized);
        if toplevel.flags.minimized {
            window.state.visible = Some(false);
        } else if !any_focused && toplevel.flags.activated {
            window.state.is_focused = true;
        }
    }
}

/// Annotates `windows` from the compositor on the wlroots route; any other
/// route, or a compositor that does not answer in time, leaves them as
/// listed.
pub(super) fn from_compositor(windows: &mut [ListedWindow], deadline: Deadline) {
    if !route::toplevel_control() {
        return;
    }
    let Ok(session) = ForeignToplevels::open(deadline.capped(COMPOSITOR_READ)) else {
        return;
    };
    let mut infos: Vec<&mut WindowInfo> =
        windows.iter_mut().map(|window| &mut window.info).collect();
    annotate(&mut infos, &session.toplevels(), app_names);
}

#[cfg(test)]
#[path = "toplevel_annotate_tests.rs"]
mod tests;
