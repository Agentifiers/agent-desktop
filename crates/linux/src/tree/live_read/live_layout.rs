//! Places a frame on a wlroots compositor: the frame's title and its
//! application's names pick the compositor's toplevel, and its window
//! rectangle ties the toplevel's place to the frame's window coordinates.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use crate::system::wlroots::toplevel_match::{WindowIdentity, app_names};
use crate::system::wlroots::window_place::{Placed, WindowPlace, locate};
use crate::tree::placement::layout_place::LayoutPlace;
use crate::tree::placement::reported::Reported;
use crate::tree::{AtspiElement, Extents};

/// The proved place of `frame`, whose window rectangle reads `window` and
/// whose accessible name is `title`. A frame without a rectangle or a title
/// names no toplevel and has none.
pub(crate) fn prove_place(
    frame: &AtspiElement,
    window: Option<Extents>,
    title: Option<&str>,
    deadline: Deadline,
) -> Result<Option<WindowPlace>, AdapterError> {
    let (Some(window), Some(title)) = (window, title.filter(|title| !title.is_empty())) else {
        return Ok(None);
    };
    let names = app_names(frame.pid());
    let identity = WindowIdentity {
        title,
        app_names: &names,
    };
    locate(identity, window, deadline)
}

/// [`prove_place`] as a placement: a proof that failed or found nothing
/// leaves the frame unproven.
pub(crate) fn layout_of(
    frame: &AtspiElement,
    window: Option<Extents>,
    title: Option<&str>,
    deadline: Deadline,
) -> LayoutPlace {
    match prove_place(frame, window, title, deadline) {
        Ok(Some(place)) => LayoutPlace::Proven(place.origin()),
        Ok(None) | Err(_) => LayoutPlace::Unproven,
    }
}

/// A window rectangle a toolkit reports at `reported`, moved to where the
/// node is drawn by the toolkit's frame for it, which `frame` reads only
/// when there is a rectangle to move. `None` when that frame cannot be read:
/// the node is then not placed, as an unproven window is not.
pub(crate) fn drawn_rect(
    reported: Option<Extents>,
    frame: impl FnOnce() -> Option<Reported>,
) -> Option<Option<Extents>> {
    match reported {
        None => Some(None),
        Some(rect) => frame()?.drawn(rect).map(Some),
    }
}

/// The refusal for a target whose window the compositor shows somewhere no
/// proof reaches, so no pointer point exists for it. No event was sent. An
/// active maximized window is placed from the compositor's state alone, so
/// the suggestion names the commands that make it one.
pub(crate) fn origin_unproven(frame: Option<&AtspiElement>) -> AdapterError {
    let window_id = frame
        .and_then(|frame| crate::system::window::window_id::encode(frame.bus_name(), frame.path()));
    let id = window_id.as_deref().unwrap_or("<id>");
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The compositor does not reveal where this window is drawn, so no pointer event was sent",
    )
    .with_suggestion(format!(
        "Run 'maximize --window-id {id}' and 'focus-window --window-id {id}', then retry; or run the action without --headed to act through accessibility; or click a point you located yourself with 'mouse-click --xy'"
    ))
    .with_details(json!({
        "kind": "wlroots_window_origin_unproven",
        "window_id": window_id,
        "retryable": false,
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "live_layout_tests.rs"]
mod tests;
