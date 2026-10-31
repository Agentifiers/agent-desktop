use agent_desktop_core::{AdapterError, CursorOverlayStyle};
use wayland_client::QueueHandle;
use wayland_client::protocol::wl_shm::{Format, WlShm};

use super::arrow::arrow;
use super::stage_state::StageState;
use crate::system::wlroots::shm::{BufferSpec, ShmBuffer};

/// A marker's pixels in a buffer the compositor can show, and the style
/// they were drawn from.
pub(crate) struct Painted {
    pub(crate) buffer: ShmBuffer,
    pub(crate) style: CursorOverlayStyle,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl Painted {
    pub(crate) fn draw(
        shm: &WlShm,
        style: &CursorOverlayStyle,
        handle: &QueueHandle<StageState>,
    ) -> Result<Self, AdapterError> {
        let raster = arrow(style);
        let spec = BufferSpec {
            width: raster.width,
            height: raster.height,
            stride: raster.width * 4,
            format: Format::Argb8888 as u32,
        };
        let buffer = ShmBuffer::create(shm, spec, handle)?;
        buffer.write(&raster.to_shm_bytes())?;
        Ok(Self {
            buffer,
            style: style.clone(),
            width: i32::try_from(raster.width).unwrap_or(i32::MAX),
            height: i32::try_from(raster.height).unwrap_or(i32::MAX),
        })
    }
}
