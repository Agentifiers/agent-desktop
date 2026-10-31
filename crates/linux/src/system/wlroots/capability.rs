//! Which wlroots protocols the session's compositor advertises. The route is
//! chosen from these globals, never from the compositor's name, so any
//! compositor that offers the protocols takes the same route.

use std::sync::{Mutex, PoisonError};

use agent_desktop_core::{AdapterError, Deadline};
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_registry::WlRegistry;

use super::link::{CoreHandler, Global, GlobalSink, Link, SyncFlag};

pub(crate) const VIRTUAL_KEYBOARD: &str = "zwp_virtual_keyboard_manager_v1";
pub(crate) const VIRTUAL_POINTER: &str = "zwlr_virtual_pointer_manager_v1";
pub(crate) const SEAT: &str = "wl_seat";
pub(crate) const SCREENCOPY: &str = "zwlr_screencopy_manager_v1";
pub(crate) const TOPLEVEL_LIST: &str = "ext_foreign_toplevel_list_v1";
pub(crate) const IMAGE_COPY_CAPTURE: &str = "ext_image_copy_capture_manager_v1";
pub(crate) const TOPLEVEL_CAPTURE_SOURCE: &str =
    "ext_foreign_toplevel_image_capture_source_manager_v1";

const PROBE_BUDGET_MS: u64 = 1_000;

/// The globals the compositor advertised when it was probed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Capabilities {
    pub(crate) globals: Vec<Global>,
}

impl Capabilities {
    pub(crate) fn has(&self, interface: &str) -> bool {
        self.globals
            .iter()
            .any(|global| global.interface == interface)
    }

    /// Whether keys and the pointer can both be synthesized: a seat, a
    /// virtual keyboard and a virtual pointer.
    pub(crate) fn input(&self) -> bool {
        self.has(SEAT) && self.has(VIRTUAL_KEYBOARD) && self.has(VIRTUAL_POINTER)
    }

    /// Whether whole outputs can be copied without a portal.
    pub(crate) fn screencopy(&self) -> bool {
        self.has(SCREENCOPY)
    }

    /// Whether one toplevel can be captured on its own.
    pub(crate) fn toplevel_capture(&self) -> bool {
        self.has(TOPLEVEL_LIST) && self.has(IMAGE_COPY_CAPTURE) && self.has(TOPLEVEL_CAPTURE_SOURCE)
    }
}

#[derive(Default)]
struct ProbeState {
    globals: Vec<Global>,
}

impl GlobalSink for ProbeState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

wayland_client::delegate_dispatch!(ProbeState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(ProbeState: [WlRegistry: ()] => CoreHandler);

/// Reads the compositor's globals over a fresh connection.
pub(crate) fn read(deadline: Deadline) -> Result<Capabilities, AdapterError> {
    let mut link = Link::<ProbeState>::open()?;
    let mut state = ProbeState::default();
    link.registry(&mut state, deadline)?;
    Ok(Capabilities {
        globals: state.globals,
    })
}

type Cached = Option<(String, Capabilities)>;

static CACHE: Mutex<Cached> = Mutex::new(None);

fn session_key() -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    format!("{runtime}\u{0}{display}")
}

/// The capabilities of the current session's compositor, read once per
/// session address and remembered. A compositor that cannot be read within
/// the probe budget answers `None` and is asked again next time.
pub(crate) fn current() -> Option<Capabilities> {
    let key = session_key();
    if let Some((cached_key, capabilities)) = CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        && *cached_key == key
    {
        return Some(capabilities.clone());
    }
    let deadline = Deadline::detached_after(PROBE_BUDGET_MS).ok()?;
    let capabilities = read(deadline).ok()?;
    *CACHE.lock().unwrap_or_else(PoisonError::into_inner) = Some((key, capabilities.clone()));
    Some(capabilities)
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
