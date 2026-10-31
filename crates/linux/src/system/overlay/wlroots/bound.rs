use wayland_client::QueueHandle;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

use super::stage_state::StageState;
use crate::system::wlroots::link::{Global, bind};

const COMPOSITOR_NEWEST: u32 = 4;
const SHM_NEWEST: u32 = 1;
const LAYER_SHELL_NEWEST: u32 = 4;

/// The globals a marker is drawn with.
pub(crate) struct Bound {
    pub(crate) compositor: WlCompositor,
    pub(crate) shm: WlShm,
    pub(crate) layer_shell: ZwlrLayerShellV1,
}

impl Bound {
    /// Binds them all, or answers `None` when the compositor lacks one.
    pub(crate) fn bind(
        registry: &WlRegistry,
        globals: &[Global],
        handle: &QueueHandle<StageState>,
    ) -> Option<Self> {
        Some(Self {
            compositor: bind(registry, globals, COMPOSITOR_NEWEST, handle, ())?,
            shm: bind(registry, globals, SHM_NEWEST, handle, ())?,
            layer_shell: bind(registry, globals, LAYER_SHELL_NEWEST, handle, ())?,
        })
    }
}
