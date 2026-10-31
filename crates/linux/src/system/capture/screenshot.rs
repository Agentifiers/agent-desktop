use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, ImageBuffer, ScreenshotTarget, WindowInfo,
};

use crate::system::SharedBus;
use crate::system::capture::bus_lookup::BusLookup;
use crate::system::capture::capture_env::CaptureEnv;
use crate::system::capture::toplevel_route::ToplevelRoute;
use crate::system::capture::toplevel_source::open;
use crate::system::capture::wayland_capture::WaylandCapture;
use crate::system::capture::wayland_portal::capture_wayland;
use crate::system::capture::window_context::real_decode;
use crate::system::capture::{display, png_encode::encode_png, window};
use crate::system::capture::{wlroots_display, wlroots_window};
use crate::system::display::route::{Route, current};
use crate::system::overlay::cursor_overlay;
use crate::system::wlroots::{capability, toplevel_match::app_names};
use crate::system::x11::DisplayReader;
use crate::system::x11::capture::grab::DisplayGrabber;

fn not_delivered(error: AdapterError) -> AdapterError {
    if error.disposition == DeliverySemantics::Unknown {
        error.with_disposition(DeliverySemantics::not_delivered())
    } else {
        error
    }
}

/// How long a person has to answer the portal's first-use dialog. The command's
/// own budget is for calls that need no person; a person has to read the
/// dialog and click, and the portal needs a second more to answer. The portal
/// ends an unanswered dialog at 25 s and honours `Request.Close` only then, so a
/// shorter wait leaves a live dialog whose late Allow writes the desktop to a
/// file nobody removes. No caller deadline, inherited or not, shortens it.
pub(crate) const CONSENT_BUDGET_MS: u64 = 30_000;

pub(crate) fn consent_deadline(command: Deadline) -> Result<Deadline, AdapterError> {
    let consent = Deadline::detached_after(CONSENT_BUDGET_MS)?;
    Ok(if consent.remaining() > command.remaining() {
        consent
    } else {
        command
    })
}

fn wayland_window() -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        "A single window cannot be captured on a Wayland session",
    )
    .with_suggestion("Capture the screen instead, or run the application in an X11 session")
}

/// How `route` captures on Wayland. Only a wlroots compositor is asked which
/// capture protocols it offers; Mutter's route keeps the portal alone.
fn wayland_capture(route: &Route) -> WaylandCapture<'static> {
    let offered = matches!(route, Route::Wlroots)
        .then(capability::current)
        .flatten();
    let offers = |has: fn(&capability::Capabilities) -> bool| offered.as_ref().is_some_and(has);
    WaylandCapture {
        portal: &capture_wayland,
        outputs: offers(capability::Capabilities::screencopy)
            .then_some(&wlroots_display::copy_output as _),
        toplevels: offers(capability::Capabilities::toplevel_capture).then_some(ToplevelRoute {
            open: &open,
            app_names,
        }),
    }
}

/// Captures `target` by the session's route. A window is captured on X11,
/// and on a wlroots compositor that captures single toplevels; every other
/// Wayland window request is refused before any X11 call.
pub(crate) fn capture_with(
    env: &CaptureEnv,
    route: &Route,
    target: ScreenshotTarget,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    capture_through(env, route, target, deadline, &wayland_capture(route))
}

/// As [`capture_with`] with the Wayland capture paths given. A wlroots
/// compositor with screencopy copies the display itself; one without it, and
/// Mutter, ask the portal.
pub(crate) fn capture_through(
    env: &CaptureEnv,
    route: &Route,
    target: ScreenshotTarget,
    deadline: Deadline,
    wayland: &WaylandCapture<'_>,
) -> Result<ImageBuffer, AdapterError> {
    match (route, target) {
        (Route::Wlroots, ScreenshotTarget::ExactWindow(window)) => match &wayland.toplevels {
            Some(toplevels) => wlroots_window::capture(env, &window, false, deadline, toplevels),
            None => Err(wayland_window()),
        },
        (Route::Wlroots, target) => match wayland.outputs {
            Some(copy) => wlroots_display::capture(env, &target, deadline, copy),
            None => (wayland.portal)(&target, consent_deadline(deadline)?),
        },
        (Route::Wayland, ScreenshotTarget::ExactWindow(_)) => Err(wayland_window()),
        (Route::Wayland, target) => (wayland.portal)(&target, consent_deadline(deadline)?),
        (Route::X11(_), ScreenshotTarget::ExactWindow(window)) => {
            window::capture(env, &window, false, deadline)
        }
        (Route::X11(_), target) => display::capture(env, &target, deadline),
    }
    .map_err(not_delivered)
}

/// Captures the window for the debug frame, refusing a window whose drawables
/// match no reported bounds rather than returning an image core would reject.
pub(crate) fn capture_frame_with(
    env: &CaptureEnv,
    route: &Route,
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    capture_frame_through(env, route, window, deadline, &wayland_capture(route))
}

pub(crate) fn capture_frame_through(
    env: &CaptureEnv,
    route: &Route,
    window: &WindowInfo,
    deadline: Deadline,
    wayland: &WaylandCapture<'_>,
) -> Result<ImageBuffer, AdapterError> {
    match (route, &wayland.toplevels) {
        (Route::Wlroots, Some(toplevels)) => {
            wlroots_window::capture(env, window, true, deadline, toplevels)
        }
        (Route::Wayland | Route::Wlroots, _) => Err(wayland_window()),
        (Route::X11(_), _) => window::capture(env, window, true, deadline),
    }
    .map_err(not_delivered)
}

fn with_env<T>(
    bus: &SharedBus,
    route: &Route,
    deadline: Deadline,
    run: impl FnOnce(&CaptureEnv) -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let lookup = BusLookup { bus };
    let env = CaptureEnv {
        grabber: &DisplayGrabber,
        stacking: &DisplayReader,
        lookup: &lookup,
        decode: real_decode,
        encode: encode_png,
        stopped: crate::system::process::proc_stat::is_stopped,
    };
    match route {
        Route::X11(display) => cursor_overlay::with_hidden(display, deadline, || run(&env)),
        Route::Wayland | Route::Wlroots => run(&env),
    }
}

pub(crate) fn screenshot(
    bus: &SharedBus,
    target: ScreenshotTarget,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    let route = current().map_err(not_delivered)?;
    with_env(bus, &route, deadline, |env| {
        capture_with(env, &route, target, deadline)
    })
    .map_err(not_delivered)
}

pub(crate) fn screenshot_window_frame(
    bus: &SharedBus,
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    let route = current().map_err(not_delivered)?;
    with_env(bus, &route, deadline, |env| {
        capture_frame_with(env, &route, window, deadline)
    })
    .map_err(not_delivered)
}

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "screenshot_wlroots_tests.rs"]
mod wlroots_tests;
