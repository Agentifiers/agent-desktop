//! The compositor's toplevels as `ext_foreign_toplevel_list_v1` lists them,
//! and the pixels of one toplevel through
//! `ext_foreign_toplevel_image_capture_source_manager_v1` and
//! `ext_image_copy_capture_manager_v1`. A toplevel capture holds the
//! window's own surfaces, unobstructed by anything stacked above it.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::{
    self, ExtImageCopyCaptureFrameV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::{
    self, ExtImageCopyCaptureSessionV1,
};

use super::captured_image::{CapturedImage, PixelLayout};
use super::link::{CoreHandler, Global, GlobalSink, Link, SyncFlag, bind};
use super::listed_toplevel::ListedToplevel;
use super::shm::{BufferSpec, ShmBuffer};

#[derive(Default)]
struct SessionState {
    size: Option<(u32, u32)>,
    formats: Vec<u32>,
    done: bool,
    stopped: bool,
}

#[derive(Default)]
struct FrameState {
    ready: bool,
    failed: Option<u32>,
    transformed: bool,
}

#[derive(Default)]
pub(crate) struct ListState {
    globals: Vec<Global>,
    toplevels: Vec<(ExtForeignToplevelHandleV1, ListedToplevel)>,
    session: SessionState,
    frame: FrameState,
}

impl GlobalSink for ListState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

wayland_client::delegate_dispatch!(ListState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(ListState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_noop!(ListState: ignore WlShm);
wayland_client::delegate_noop!(ListState: ignore WlShmPool);
wayland_client::delegate_noop!(ListState: ignore WlBuffer);
wayland_client::delegate_noop!(ListState: ignore ExtForeignToplevelImageCaptureSourceManagerV1);
wayland_client::delegate_noop!(ListState: ignore ExtImageCaptureSourceV1);
wayland_client::delegate_noop!(ListState: ignore ExtImageCopyCaptureManagerV1);

impl Dispatch<ExtForeignToplevelListV1, ()> for ListState {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.toplevels.push((toplevel, ListedToplevel::default()));
        }
    }

    event_created_child!(ListState, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for ListState {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, listed)) = state
            .toplevels
            .iter_mut()
            .find(|(known, _)| known.id() == handle.id())
        else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Title { title } => listed.title = title,
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => listed.app_id = app_id,
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                listed.identifier = identifier;
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => listed.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for ListState {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let session = &mut state.session;
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                session.size = Some((width, height));
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat { format } => {
                session.formats.push(u32::from(format));
            }
            ext_image_copy_capture_session_v1::Event::Done => session.done = true,
            ext_image_copy_capture_session_v1::Event::Stopped => session.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for ListState {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let frame = &mut state.frame;
        match event {
            ext_image_copy_capture_frame_v1::Event::Ready => frame.ready = true,
            ext_image_copy_capture_frame_v1::Event::Failed { reason } => {
                frame.failed = Some(match reason {
                    WEnum::Value(reason) => reason as u32,
                    WEnum::Unknown(value) => value,
                });
            }
            ext_image_copy_capture_frame_v1::Event::Transform { transform } => {
                frame.transformed = u32::from(transform) != 0;
            }
            _ => {}
        }
    }
}

fn capture_failed(reason: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The compositor did not capture the window: {reason}"),
    )
    .with_suggestion("Retry once the window is mapped and unchanged")
}

/// One connection that lists the toplevels and can capture each.
pub(crate) struct ToplevelLister {
    link: Link<ListState>,
    state: ListState,
    registry: WlRegistry,
    list: ExtForeignToplevelListV1,
}

impl ToplevelLister {
    pub(crate) fn open(deadline: Deadline) -> Result<Self, AdapterError> {
        let mut link = Link::<ListState>::open()?;
        let mut state = ListState::default();
        let registry = link.registry(&mut state, deadline)?;
        let globals = state.globals.clone();
        let list: ExtForeignToplevelListV1 = bind(&registry, &globals, 1, &link.handle, ())
            .ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::PlatformNotSupported,
                    "The compositor offers no ext_foreign_toplevel_list_v1",
                )
            })?;
        link.roundtrip(&mut state, deadline)?;
        link.roundtrip(&mut state, deadline)?;
        Ok(Self {
            link,
            state,
            registry,
            list,
        })
    }

    /// The toplevels that are still open, in the order the compositor
    /// announced them.
    pub(crate) fn toplevels(&self) -> Vec<ListedToplevel> {
        self.state
            .toplevels
            .iter()
            .map(|(_, listed)| listed.clone())
            .filter(|listed| !listed.closed)
            .collect()
    }

    /// Captures the toplevel whose identifier is `identifier`.
    pub(crate) fn capture(
        &mut self,
        identifier: &str,
        deadline: Deadline,
    ) -> Result<CapturedImage, AdapterError> {
        let handle = self
            .state
            .toplevels
            .iter()
            .find(|(_, listed)| listed.identifier == identifier && !listed.closed)
            .map(|(handle, _)| handle.clone())
            .ok_or_else(|| capture_failed("the window closed"))?;
        let globals = self.state.globals.clone();
        let missing = || {
            AdapterError::new(
                ErrorCode::PlatformNotSupported,
                "The compositor offers no toplevel image capture",
            )
        };
        let sources: ExtForeignToplevelImageCaptureSourceManagerV1 =
            bind(&self.registry, &globals, 1, &self.link.handle, ()).ok_or_else(missing)?;
        let copies: ExtImageCopyCaptureManagerV1 =
            bind(&self.registry, &globals, 1, &self.link.handle, ()).ok_or_else(missing)?;
        let shm: WlShm = bind(&self.registry, &globals, 1, &self.link.handle, ())
            .ok_or_else(|| AdapterError::internal("The compositor offers no wl_shm"))?;
        let source = sources.create_source(&handle, &self.link.handle, ());
        let session = copies.create_session(&source, Options::empty(), &self.link.handle, ());
        let outcome = self.copy(&session, &shm, deadline);
        session.destroy();
        source.destroy();
        copies.destroy();
        sources.destroy();
        outcome
    }

    fn copy(
        &mut self,
        session: &ExtImageCopyCaptureSessionV1,
        shm: &WlShm,
        deadline: Deadline,
    ) -> Result<CapturedImage, AdapterError> {
        self.state.session = SessionState::default();
        self.state.frame = FrameState::default();
        self.link
            .dispatch_until(&mut self.state, deadline, |state| {
                state.session.done || state.session.stopped
            })?;
        if self.state.session.stopped {
            return Err(capture_failed("the capture session stopped"));
        }
        let (width, height) = self
            .state
            .session
            .size
            .ok_or_else(|| capture_failed("no buffer size was offered"))?;
        let layout = PixelLayout::PREFERRED
            .into_iter()
            .find(|layout| self.state.session.formats.contains(&layout.shm_code()))
            .ok_or_else(|| capture_failed("no supported pixel format was offered"))?;
        let buffer = ShmBuffer::create(
            shm,
            BufferSpec {
                width,
                height,
                stride: width * 4,
                format: layout.shm_code(),
            },
            &self.link.handle,
        )?;
        let frame = session.create_frame(&self.link.handle, ());
        frame.attach_buffer(&buffer.buffer);
        frame.damage_buffer(
            0,
            0,
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
        );
        frame.capture();
        let copied = self
            .link
            .dispatch_until(&mut self.state, deadline, |state| {
                state.frame.ready || state.frame.failed.is_some()
            });
        frame.destroy();
        copied?;
        if let Some(reason) = self.state.frame.failed {
            return Err(capture_failed(&format!("failure reason {reason}")));
        }
        if self.state.frame.transformed {
            return Err(capture_failed("the window buffer is transformed"));
        }
        buffer.read(false)
    }
}

impl Drop for ToplevelLister {
    fn drop(&mut self) {
        for (handle, _) in &self.state.toplevels {
            handle.destroy();
        }
        self.list.stop();
        let _ = self.link.connection.flush();
    }
}
