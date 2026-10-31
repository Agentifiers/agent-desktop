//! Window capture on a wlroots compositor through
//! `ext_foreign_toplevel_image_capture_source_manager_v1`: the toplevel's own
//! pixels, unobstructed, with no portal. Neither wlroots toplevel protocol
//! names a process, so the accessibility window is tied to its toplevel by
//! title, and by application id among toplevels that share the title.

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, DisplayInfo, ErrorCode, ImageBuffer, Rect,
    WindowInfo,
};
use serde_json::json;

use super::display::scale_for;
use super::toplevel_route::ToplevelRoute;
use super::window::admit;
use super::wlroots_unit::{rgb_in, window_area};
use crate::system::capture::capture_env::CaptureEnv;
use crate::system::wlroots::listed_toplevel::ListedToplevel;
use crate::system::wlroots::toplevel_match::{ToplevelMatch, WindowIdentity, match_toplevel};

fn missing(title: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("The compositor lists no window titled '{title}'"),
    )
    .with_suggestion("Run 'list-windows' to refresh window IDs, then retry")
    .with_details(json!({ "title": title }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn ambiguous(title: &str, count: usize) -> AdapterError {
    AdapterError::ambiguous_target(format!(
        "{count} compositor windows titled '{title}' could be this window"
    ))
    .with_suggestion("Close the other windows with this title, or capture the screen instead")
    .with_details(json!({ "title": title, "candidate_count": count }))
}

fn changed() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The window changed while it was being captured",
    )
    .with_suggestion("Retry once the window is unchanged")
    .with_disposition(DeliverySemantics::not_delivered())
}

/// The identifier of the one toplevel that is `window`.
pub(crate) fn identify(
    listed: &[ListedToplevel],
    window: &WindowInfo,
    app_names: &[String],
) -> Result<String, AdapterError> {
    let pairs = listed
        .iter()
        .map(|toplevel| (toplevel.title.as_str(), toplevel.app_id.as_str()));
    let identity = WindowIdentity {
        title: &window.title,
        app_names,
    };
    match match_toplevel(pairs, identity) {
        ToplevelMatch::Unique(index) => listed
            .get(index)
            .map(|toplevel| toplevel.identifier.clone())
            .ok_or_else(|| missing(&window.title)),
        ToplevelMatch::Ambiguous(count) => Err(ambiguous(&window.title, count)),
        ToplevelMatch::Missing => Err(missing(&window.title)),
    }
}

fn scale_of(displays: &[DisplayInfo], window: &WindowInfo) -> f64 {
    if let Some(bounds) = window.bounds {
        return scale_for(displays, bounds);
    }
    match displays.split_first() {
        Some((first, rest)) if rest.iter().all(|other| other.scale == first.scale) => first.scale,
        _ => 1.0,
    }
}

/// The size the window reports: its bounds, or, for a toolkit that reports no
/// position on Wayland, the size of its accessibility frame.
fn reported_size(env: &CaptureEnv, window: &WindowInfo, deadline: Deadline) -> Option<Rect> {
    window.bounds.or_else(|| {
        let (width, height) = env.lookup.frame_size(window, deadline)?;
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: f64::from(width),
            height: f64::from(height),
        })
    })
}

/// Captures `window` as its compositor toplevel. The window is resolved
/// strictly first; the toplevel must still carry the window's title after the
/// copy and the window's bounds must be unchanged. The image is the part of
/// the surface whose size is the size the window reports; `strict` refuses
/// an image no part of which agrees with it.
pub(crate) fn capture(
    env: &CaptureEnv,
    window: &WindowInfo,
    strict: bool,
    deadline: Deadline,
    route: &ToplevelRoute<'_>,
) -> Result<ImageBuffer, AdapterError> {
    admit(env, window, deadline)?;
    let resolved = env.lookup.resolve(window, deadline)?;
    let mut app_names = vec![resolved.app.clone()];
    app_names.extend((route.app_names)(resolved.pid.get()));
    let mut source = (route.open)(deadline)?;
    let identifier = identify(&source.listed(), &resolved, &app_names)?;
    let image = source.copy(&identifier, deadline)?;
    let same = source
        .listed()
        .iter()
        .any(|toplevel| toplevel.identifier == identifier && toplevel.title == resolved.title);
    drop(source);
    deadline.ensure_remaining()?;
    let after = env.lookup.resolve(&resolved, deadline)?;
    if !same || after.bounds != resolved.bounds {
        return Err(changed());
    }
    let displays = env.lookup.displays(deadline).unwrap_or_default();
    let scale = scale_of(&displays, &resolved);
    let size = reported_size(env, &resolved, deadline);
    let area = window_area(&image, size, scale, strict)?;
    let rgb = rgb_in(&image, area);
    drop(image);
    (env.encode)(
        &rgb,
        u32::try_from(area.width).unwrap_or(0),
        u32::try_from(area.height).unwrap_or(0),
        scale,
    )
}

#[cfg(test)]
#[path = "wlroots_window_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wlroots_fakes_tests.rs"]
pub(crate) mod fakes;
