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

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
