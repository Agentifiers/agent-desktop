//! One marker on the compositor: a small surface on the overlay layer of
//! one output, anchored to the output's top-left corner and moved by its
//! margins. It takes no keyboard focus, reserves no space, and its input
//! region is empty, so every click and hover reaches what lies beneath.

use wayland_client::QueueHandle;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

use super::bound::Bound;
use super::painted::Painted;
use super::stage_state::StageState;

const NAMESPACE: &str = "agent-desktop-cursor";
const NO_EXCLUSIVE_ZONE: i32 = -1;

pub(crate) struct MarkerSurface {
    pub(crate) id: u32,
    pub(crate) output: String,
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    pub(crate) painted: Option<Painted>,
    mapped: bool,
    placed: Option<(i32, i32)>,
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
            painted: None,
            mapped: false,
            placed: None,
        }
    }

    pub(crate) const fn is_mapped(&self) -> bool {
        self.mapped
    }

    /// Sends the layer state and commits without a buffer: the commit that
    /// asks the compositor for the configure a map needs.
    pub(crate) fn request_map(
        &mut self,
        bound: &Bound,
        margin: (i32, i32),
        handle: &QueueHandle<StageState>,
    ) {
        let (width, height) = self
            .painted
            .as_ref()
            .map_or((1, 1), |painted| (painted.width, painted.height));
        self.layer
            .set_size(width.unsigned_abs(), height.unsigned_abs());
        self.layer.set_anchor(Anchor::Top | Anchor::Left);
        self.layer.set_exclusive_zone(NO_EXCLUSIVE_ZONE);
        self.layer
            .set_keyboard_interactivity(KeyboardInteractivity::None);
        self.layer.set_margin(margin.1, 0, 0, margin.0);
        let region = bound.compositor.create_region(handle, ());
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.placed = Some(margin);
    }

    /// Shows the painted buffer once the compositor has configured the
    /// surface.
    pub(crate) fn finish_map(&mut self) {
        if let Some(painted) = &self.painted {
            self.surface.attach(Some(&painted.buffer.buffer), 0, 0);
            self.surface.damage(0, 0, painted.width, painted.height);
        }
        self.surface.commit();
        self.mapped = true;
    }

    /// Moves a shown marker, and shows `repainted` pixels when the style
    /// changed. Commits only when something did.
    pub(crate) fn update(&mut self, margin: (i32, i32), repainted: bool) {
        if !repainted && self.placed == Some(margin) {
            return;
        }
        if repainted && let Some(painted) = &self.painted {
            self.layer
                .set_size(painted.width.unsigned_abs(), painted.height.unsigned_abs());
            self.surface.attach(Some(&painted.buffer.buffer), 0, 0);
            self.surface.damage(0, 0, painted.width, painted.height);
        }
        self.layer.set_margin(margin.1, 0, 0, margin.0);
        self.surface.commit();
        self.placed = Some(margin);
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
