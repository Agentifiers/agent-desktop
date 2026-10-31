use agent_desktop_core::AdapterError;

use super::display_server::{DisplayServer, session_server};
use crate::system::display;

/// How input, capture and window control reach the desktop. A Wayland
/// compositor that advertises a seat, a virtual keyboard and a virtual pointer
/// takes the wlroots route; any other Wayland compositor takes the Mutter
/// route, which probes `org.gnome.Mutter.RemoteDesktop` itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    X11(String),
    Wayland,
    Wlroots,
}

/// The route for `server`, asking the compositor's globals only for a
/// Wayland session.
pub(crate) fn resolve(server: DisplayServer) -> Result<Route, AdapterError> {
    resolve_with(server, wlroots_input)
}

/// The route for `server`, with `wlroots` answering whether the Wayland
/// compositor advertises the wlroots input protocols.
pub(crate) fn resolve_with(
    server: DisplayServer,
    wlroots: impl FnOnce() -> bool,
) -> Result<Route, AdapterError> {
    match server {
        DisplayServer::X11(display) => Ok(Route::X11(display)),
        DisplayServer::Wayland if wlroots() => Ok(Route::Wlroots),
        DisplayServer::Wayland => Ok(Route::Wayland),
        DisplayServer::Unavailable(why) => Err(display::unreachable(why)),
    }
}

fn wlroots_input() -> bool {
    crate::system::wlroots::capability::current().is_some_and(|found| found.input())
}

pub(crate) fn current() -> Result<Route, AdapterError> {
    resolve(session_server())
}

/// Whether the session takes the wlroots route, where no toolkit's screen
/// extents are a position and a window is placed only by what the
/// compositor proves.
pub(crate) fn is_wlroots() -> bool {
    current().is_ok_and(|route| route == Route::Wlroots)
}

/// Whether window control goes through the wlroots foreign-toplevel
/// protocol: the session takes the wlroots route and its compositor lets a
/// client list, activate and close toplevels. Every other session keeps its
/// own window control.
pub(crate) fn toplevel_control() -> bool {
    toplevel_control_with(current().ok().as_ref(), || {
        crate::system::wlroots::capability::current().is_some_and(|found| found.toplevel_control())
    })
}

/// As [`toplevel_control`] for a resolved `route`, with `control` answering
/// whether the compositor advertises toplevel control.
pub(crate) fn toplevel_control_with(route: Option<&Route>, control: impl FnOnce() -> bool) -> bool {
    route == Some(&Route::Wlroots) && control()
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
