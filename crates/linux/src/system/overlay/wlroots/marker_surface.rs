//! One cursor on the compositor: a surface on the overlay layer of one
//! output, anchored to its top-left corner, moved by margins and sized to the
//! frame it shows. It takes no keyboard focus, reserves no space, and its
//! input region is empty, so every click and hover reaches what lies beneath.

use wayland_client::QueueHandle;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Proxy, backend::ObjectId};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

use super::bound::Bound;
use super::stage_state::StageState;
use crate::system::wlroots::shm::ShmBuffer;

const NAMESPACE: &str = "agent-desktop-cursor";
const NO_EXCLUSIVE_ZONE: i32 = -1;
const MAX_IN_FLIGHT: usize = 8;

/// Where and how large a frame is shown on the surface's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) margin: (i32, i32),
    pub(crate) size: (i32, i32),
}

pub(crate) struct MarkerSurface {
    pub(crate) id: u32,
    pub(crate) output: String,
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    in_flight: Vec<ShmBuffer>,
    mapped: bool,
    placed: Option<Placement>,
}

impl MarkerSurface {
    pub(crate) fn create(
        id: u32,
        bound: &Bound,
        output: (&WlOutput, &str),
        handle: &QueueHandle<StageState>,
    ) -> Self {
        let surface = bound.compositor.create_surface(handle, ());
        let layer = bound.layer_shell.get_layer_surface(
            &surface,
            Some(output.0),
            Layer::Overlay,
            NAMESPACE.to_owned(),
            handle,
            id,
        );
        Self {
            id,
            output: output.1.to_owned(),
            surface,
            layer,
            in_flight: Vec::new(),
            mapped: false,
            placed: None,
        }
    }

    pub(crate) const fn is_mapped(&self) -> bool {
        self.mapped
    }

    fn arrange(&self, placement: Placement) {
        self.layer.set_size(
            placement.size.0.unsigned_abs(),
            placement.size.1.unsigned_abs(),
        );
        self.layer
            .set_margin(placement.margin.1, 0, 0, placement.margin.0);
    }

    /// Sends the layer state and commits without a buffer: the commit that
    /// asks the compositor for the configure a map needs.
    pub(crate) fn request_map(
        &mut self,
        bound: &Bound,
        placement: Placement,
        handle: &QueueHandle<StageState>,
    ) {
        self.arrange(placement);
        self.layer.set_anchor(Anchor::Top | Anchor::Left);
        self.layer.set_exclusive_zone(NO_EXCLUSIVE_ZONE);
        self.layer
            .set_keyboard_interactivity(KeyboardInteractivity::None);
        let region = bound.compositor.create_region(handle, ());
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.placed = Some(placement);
    }

    /// Shows `buffer` at `placement`; the first frame after a configure maps
    /// the surface.
    pub(crate) fn show(&mut self, buffer: ShmBuffer, placement: Placement, released: &[ObjectId]) {
        if self.placed != Some(placement) {
            self.arrange(placement);
            self.placed = Some(placement);
        }
        self.surface.attach(Some(&buffer.buffer), 0, 0);
        self.surface
            .damage(0, 0, placement.size.0, placement.size.1);
        self.surface.commit();
        self.mapped = true;
        self.in_flight
            .retain(|held| !released.contains(&held.buffer.id()));
        if self.in_flight.len() >= MAX_IN_FLIGHT {
            self.in_flight.remove(0);
        }
        self.in_flight.push(buffer);
    }

    /// Unmaps the surface; it returns to the state it had when created.
    pub(crate) fn unmap(&mut self) {
        if !self.mapped {
            return;
        }
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.mapped = false;
        self.placed = None;
    }
}

impl Drop for MarkerSurface {
    fn drop(&mut self) {
        self.layer.destroy();
        self.surface.destroy();
    }
}
