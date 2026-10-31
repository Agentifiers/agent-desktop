//! The helper connection's event state: the registry's globals, the outputs,
//! and which marker surfaces the compositor configured or closed. A
//! configure is acknowledged the moment it arrives; the next commit applies it.
//! A released frame buffer is recorded so it can be freed.

use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_buffer;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};

use crate::system::wlroots::link::{CoreHandler, Global, GlobalSink, SyncFlag};
use crate::system::wlroots::output::Output;
use crate::system::wlroots::outputs::{OutputHandler, OutputSink};

#[derive(Debug, Default)]
pub(crate) struct StageState {
    pub(crate) globals: Vec<Global>,
    pub(crate) outputs: Vec<Output>,
    pub(crate) configured: Vec<u32>,
    pub(crate) closed: Vec<u32>,
    pub(crate) next_id: u32,
    pub(crate) released: Vec<ObjectId>,
}

impl GlobalSink for StageState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

impl OutputSink for StageState {
    fn outputs(&mut self) -> &mut Vec<Output> {
        &mut self.outputs
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, u32> for StageState {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                layer.ack_configure(serial);
                state.configured.push(*id);
            }
            zwlr_layer_surface_v1::Event::Closed => state.closed.push(*id),
            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, ()> for StageState {
    fn event(
        state: &mut Self,
        buffer: &WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.released.push(buffer.id());
        }
    }
}

wayland_client::delegate_dispatch!(StageState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(StageState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_dispatch!(StageState: [WlOutput: usize] => OutputHandler);
wayland_client::delegate_dispatch!(StageState: [ZxdgOutputV1: usize] => OutputHandler);
wayland_client::delegate_noop!(StageState: ignore ZxdgOutputManagerV1);
wayland_client::delegate_noop!(StageState: ignore WlCompositor);
wayland_client::delegate_noop!(StageState: ignore WlRegion);
wayland_client::delegate_noop!(StageState: ignore WlSurface);
wayland_client::delegate_noop!(StageState: ignore WlShm);
wayland_client::delegate_noop!(StageState: ignore WlShmPool);
wayland_client::delegate_noop!(StageState: ignore ZwlrLayerShellV1);
