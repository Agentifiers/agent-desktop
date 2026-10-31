//! The cursor overlay on a wlroots compositor. A Wayland surface lives only
//! as long as its client, so each session's markers are held by a helper
//! process the first enable or present starts; commands reach it over a
//! private socket. The cursor looks and moves like the Windows and macOS
//! overlays - the dart gliding along the shared hand path, the click ripple,
//! the target highlight and the label card - is routed to agents like the
//! X11 overlay, and is kept out of every capture this tool takes.

mod bound;
mod canvas;
mod client;
mod clock;
mod director;
mod effect;
mod endpoint;
mod look;
mod marker_surface;
mod paint;
mod poll;
mod presentation;
mod request;
mod rest;
mod serve;
mod session_gate;
mod spawn;
mod stage;
mod stage_state;

use std::path::PathBuf;

use agent_desktop_core::{AdapterError, CursorOverlayControl, Deadline, FileLock};

use request::Request;

pub(crate) use serve::entry_from_env;

const UPDATE_BUDGET_MS: u64 = 4_000;

fn state_root() -> Result<PathBuf, AdapterError> {
    agent_desktop_core::session::agent_desktop_dir().map_err(|error| {
        AdapterError::internal("The cursor overlay state directory is not usable")
            .with_platform_detail(error.to_string())
    })
}

/// The socket of the helper that serves `session_id` on this compositor.
pub(crate) fn socket_for(session_id: &str) -> Result<PathBuf, AdapterError> {
    endpoint::socket_in(&endpoint::directory()?, &state_root()?, session_id)
}

fn unrendered(cause: &str) -> serde_json::Value {
    serde_json::json!({"rendered": false, "cause": cause})
}

/// Applies `control` to the session's markers on a wlroots compositor,
/// starting the session's helper for an enable or a present when none runs.
pub(crate) fn update(control: &CursorOverlayControl) -> Result<(), AdapterError> {
    if !crate::system::wlroots::capability::current().is_some_and(|found| found.cursor_overlay()) {
        return Err(AdapterError::not_supported(
            "cursor overlay on a compositor without a layer shell",
        )
        .with_details(unrendered("layer_shell_missing")));
    }
    if control
        .style()
        .is_some_and(|style| !style.images().is_empty())
    {
        return Err(
            AdapterError::not_supported("custom cursor images on wlroots")
                .with_details(unrendered("custom_images")),
        );
    }
    let deadline = Deadline::after(UPDATE_BUDGET_MS)?;
    let root = state_root()?;
    let socket = endpoint::socket_in(&endpoint::directory()?, &root, control.session_id())?;
    let _lock = FileLock::acquire(
        &root.join(".wlr-cursor.lock"),
        deadline,
        "cursor overlay update",
    )?;
    let request = Request::Control(Box::new(control.clone()));
    if client::exchange(&socket, &request, deadline)?.is_some() {
        return Ok(());
    }
    if control.is_disable()
        || control.is_transient()
        || !session_gate::admits(control.session_id(), control.agent_id())
    {
        return Ok(());
    }
    spawn::start(control.session_id(), &socket, deadline)?;
    client::exchange(&socket, &request, deadline)?
        .map(drop)
        .ok_or_else(|| {
            AdapterError::internal("The cursor overlay helper stopped listening after it started")
        })
}

/// Runs `capture` while every helper on this compositor keeps its markers
/// off the screen, so the capture never shows one, then shows them again.
pub(crate) fn concealed<T>(
    deadline: Deadline,
    capture: impl FnOnce() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let Ok(directory) = endpoint::directory() else {
        return capture();
    };
    let mut held = Vec::new();
    for socket in endpoint::listed(&directory) {
        match client::exchange(&socket, &Request::Conceal, deadline) {
            Ok(Some(stream)) => held.push(stream),
            Ok(None) => {}
            Err(error) => {
                held.into_iter().for_each(client::release);
                return Err(error);
            }
        }
    }
    let result = capture();
    held.into_iter().for_each(client::release);
    result
}
