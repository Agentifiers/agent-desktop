use agent_desktop_core::{AdapterError, DeliverySemantics};

use crate::system::display::display_server::{DisplayServer, session_server};

pub(crate) use crate::system::display::route::Route as Backend;

pub(crate) fn backend() -> Result<Backend, AdapterError> {
    select(session_server())
}

pub(crate) fn select(server: DisplayServer) -> Result<Backend, AdapterError> {
    not_delivered(crate::system::display::route::resolve(server))
}

/// [`select`] with `wlroots` answering whether the Wayland compositor
/// advertises the wlroots input protocols.
#[cfg(test)]
pub(crate) fn select_with(
    server: DisplayServer,
    wlroots: impl FnOnce() -> bool,
) -> Result<Backend, AdapterError> {
    not_delivered(crate::system::display::route::resolve_with(server, wlroots))
}

fn not_delivered(route: Result<Backend, AdapterError>) -> Result<Backend, AdapterError> {
    route.map_err(|mut error| {
        error.details.get_or_insert_with(|| serde_json::json!({}))["physical_delivery_started"] =
            false.into();
        error.with_disposition(DeliverySemantics::not_delivered())
    })
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
