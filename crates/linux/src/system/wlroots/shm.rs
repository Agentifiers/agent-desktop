//! A `wl_shm` buffer the compositor copies pixels into, or one the client
//! draws for its own surface. The pixels move through the memory file with
//! positioned reads and writes, so no mapping is held.

use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use agent_desktop_core::AdapterError;
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Dispatch, QueueHandle, WEnum};

use super::captured_image::{CapturedImage, PixelLayout};
use crate::system::capture::pixels::MAX_PIXELS;

/// The size and layout of a buffer the compositor asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferSpec {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: u32,
    pub(crate) format: u32,
}

pub(crate) struct ShmBuffer {
    file: File,
    pool: WlShmPool,
    pub(crate) buffer: WlBuffer,
    spec: BufferSpec,
}

fn refused(reason: impl Into<String>) -> AdapterError {
    AdapterError::new(agent_desktop_core::ErrorCode::ActionFailed, reason)
}

fn byte_length(spec: BufferSpec) -> Result<usize, AdapterError> {
    let pixels = u64::from(spec.width) * u64::from(spec.height);
    if pixels == 0 || pixels > MAX_PIXELS {
        return Err(refused(format!(
            "The compositor offered a {}x{} buffer, outside the capture limits",
            spec.width, spec.height
        )));
    }
    if u64::from(spec.stride) < u64::from(spec.width) * 4 {
        return Err(refused(
            "The compositor offered a buffer stride narrower than its width",
        ));
    }
    usize::try_from(u64::from(spec.stride) * u64::from(spec.height))
        .map_err(|_| refused("The capture buffer is too large"))
}

impl ShmBuffer {
    /// Allocates a buffer for `spec` in an anonymous memory file.
    pub(crate) fn create<D>(
        shm: &WlShm,
        spec: BufferSpec,
        handle: &QueueHandle<D>,
    ) -> Result<Self, AdapterError>
    where
        D: Dispatch<WlShmPool, ()> + Dispatch<WlBuffer, ()> + 'static,
    {
        let length = byte_length(spec)?;
        let size = i32::try_from(length).map_err(|_| refused("The capture buffer is too large"))?;
        let fd = memfd_create("agent-desktop-capture", MemfdFlags::CLOEXEC)
            .map_err(|error| AdapterError::internal(format!("memfd_create failed: {error}")))?;
        let file = File::from(fd);
        file.set_len(length as u64).map_err(|error| {
            AdapterError::internal(format!("sizing the capture buffer: {error}"))
        })?;
        let pool = shm.create_pool(file.as_fd(), size, handle, ());
        let buffer = pool.create_buffer(
            0,
            i32::try_from(spec.width).unwrap_or(i32::MAX),
            i32::try_from(spec.height).unwrap_or(i32::MAX),
            i32::try_from(spec.stride).unwrap_or(i32::MAX),
            WEnum::<Format>::from(spec.format)
                .into_result()
                .unwrap_or(Format::Argb8888),
            handle,
            (),
        );
        Ok(Self {
            file,
            pool,
            buffer,
            spec,
        })
    }

    /// The pixels the compositor wrote, as an image.
    pub(crate) fn read(&self, y_inverted: bool) -> Result<CapturedImage, AdapterError> {
        let layout = PixelLayout::from_shm(self.spec.format).ok_or_else(|| {
            refused(format!(
                "The compositor's pixel format {:#x} is not supported for capture",
                self.spec.format
            ))
        })?;
        let mut bytes = vec![0_u8; byte_length(self.spec)?];
        self.file
            .read_exact_at(&mut bytes, 0)
            .map_err(|error| AdapterError::internal(format!("reading the capture: {error}")))?;
        CapturedImage::from_rows(
            &bytes,
            self.spec.width,
            self.spec.height,
            self.spec.stride as usize,
            layout,
            y_inverted,
        )
    }

    /// Fills the buffer from its first byte with `bytes`, laid out as its
    /// spec says, for a buffer the client draws.
    pub(crate) fn write(&self, bytes: &[u8]) -> Result<(), AdapterError> {
        if bytes.len() != byte_length(self.spec)? {
            return Err(refused("The drawn pixels do not fill the buffer"));
        }
        self.file
            .write_all_at(bytes, 0)
            .map_err(|error| AdapterError::internal(format!("filling the buffer: {error}")))
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}

#[cfg(test)]
#[path = "shm_tests.rs"]
mod tests;
