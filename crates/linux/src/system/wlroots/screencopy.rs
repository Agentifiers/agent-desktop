//! Whole-output capture through `zwlr_screencopy_manager_v1` into a
//! `wl_shm` buffer, without the cursor and without a consent dialog.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use super::captured_image::{CapturedImage, PixelLayout};
use super::link::{CoreHandler, Global, GlobalSink, Link, SyncFlag, bind};
use super::output::Output;
use super::outputs::{OutputHandler, OutputSink, read_outputs};
use super::shm::{BufferSpec, ShmBuffer};

const SCREENCOPY_NEWEST: u32 = 3;
const NO_CURSOR: i32 = 0;

#[derive(Default)]
struct FrameState {
    spec: Option<BufferSpec>,
    buffer_done: bool,
    y_inverted: bool,
    ready: bool,
    failed: bool,
}

#[derive(Default)]
struct CopyState {
    globals: Vec<Global>,
    outputs: Vec<Output>,
    frame: FrameState,
}

impl GlobalSink for CopyState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

impl OutputSink for CopyState {
    fn outputs(&mut self) -> &mut Vec<Output> {
        &mut self.outputs
    }
}

wayland_client::delegate_dispatch!(CopyState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(CopyState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_dispatch!(CopyState: [WlOutput: usize] => OutputHandler);
wayland_client::delegate_dispatch!(CopyState: [ZxdgOutputV1: usize] => OutputHandler);
wayland_client::delegate_noop!(CopyState: ignore ZxdgOutputManagerV1);
wayland_client::delegate_noop!(CopyState: ignore ZwlrScreencopyManagerV1);
wayland_client::delegate_noop!(CopyState: ignore WlShm);
wayland_client::delegate_noop!(CopyState: ignore WlShmPool);
wayland_client::delegate_noop!(CopyState: ignore WlBuffer);

impl Dispatch<ZwlrScreencopyFrameV1, ()> for CopyState {
    fn event(
        state: &mut Self,
        _: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let frame = &mut state.frame;
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let format = u32::from(format);
                if frame.spec.is_none() && PixelLayout::from_shm(format).is_some() {
                    frame.spec = Some(BufferSpec {
                        width,
                        height,
                        stride,
                        format,
                    });
                }
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => frame.buffer_done = true,
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                frame.y_inverted = matches!(flags, WEnum::Value(flags)
                    if flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert));
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => frame.ready = true,
            zwlr_screencopy_frame_v1::Event::Failed => frame.failed = true,
            _ => {}
        }
    }
}

fn failed(output: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The compositor did not copy output {output}"),
    )
    .with_suggestion("Retry the screenshot; the output may have been reconfigured")
}

fn copy_one(
    link: &mut Link<CopyState>,
    state: &mut CopyState,
    manager: &ZwlrScreencopyManagerV1,
    shm: &WlShm,
    target: (&WlOutput, &Output),
    deadline: Deadline,
) -> Result<CapturedImage, AdapterError> {
    let (handle, output) = target;
    let versioned = manager.version() >= 3;
    state.frame = FrameState::default();
    let frame = manager.capture_output(NO_CURSOR, handle, &link.handle, ());
    link.dispatch_until(state, deadline, |state| {
        state.frame.failed
            || (state.frame.spec.is_some() && (state.frame.buffer_done || !versioned))
    })?;
    let Some(spec) = state.frame.spec.filter(|_| !state.frame.failed) else {
        frame.destroy();
        return Err(failed(&output.name));
    };
    let buffer = ShmBuffer::create(shm, spec, &link.handle)?;
    frame.copy(&buffer.buffer);
    let copied = link.dispatch_until(state, deadline, |state| {
        state.frame.ready || state.frame.failed
    });
    frame.destroy();
    copied?;
    if state.frame.failed {
        return Err(failed(&output.name));
    }
    buffer.read(state.frame.y_inverted)
}

/// Copies every output `wanted` accepts, each with the output it shows.
pub(crate) fn capture_outputs(
    wanted: impl Fn(&Output) -> bool,
    deadline: Deadline,
) -> Result<Vec<(Output, CapturedImage)>, AdapterError> {
    let mut link = Link::<CopyState>::open()?;
    let mut state = CopyState::default();
    let registry = link.registry(&mut state, deadline)?;
    let globals = state.globals.clone();
    let manager: ZwlrScreencopyManagerV1 =
        bind(&registry, &globals, SCREENCOPY_NEWEST, &link.handle, ()).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::PlatformNotSupported,
                "The compositor offers no screencopy protocol",
            )
        })?;
    let shm: WlShm = bind(&registry, &globals, 1, &link.handle, ())
        .ok_or_else(|| AdapterError::internal("The compositor offers no wl_shm"))?;
    let outputs = read_outputs(&mut link, &mut state, &registry, &globals, deadline)?;
    let mut copies = Vec::new();
    for (handle, output) in outputs.iter().filter(|(_, output)| wanted(output)) {
        let image = copy_one(
            &mut link,
            &mut state,
            &manager,
            &shm,
            (handle, output),
            deadline,
        )?;
        copies.push((output.clone(), image));
    }
    manager.destroy();
    Ok(copies)
}
