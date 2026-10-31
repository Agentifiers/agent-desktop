//! The virtual keyboard and pointer one delivery creates on the compositor's
//! seat through `zwp_virtual_keyboard_v1` and `zwlr_virtual_pointer_v1`.
//! Destroying a device, or losing the connection when the process dies, makes
//! the compositor release every key the device still holds.

use std::fs::File;
use std::io::Write as _;
use std::os::fd::AsFd;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use rustix::fs::{MemfdFlags, memfd_create};
use rustix::time::{ClockId, clock_gettime};
use serde_json::json;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_keyboard::KeymapFormat;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::{Axis, AxisSource, ButtonState};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

use super::key_device::KeyDevice;
use super::pointer_device::PointerDevice;
use crate::system::wlroots::link::{
    CoreHandler, Global, GlobalSink, Link, SyncFlag, bind, protocol_failed,
};
use crate::system::wlroots::output::{Output, layout_bounds};
use crate::system::wlroots::outputs::{OutputHandler, OutputSink, read_outputs};

const WHEEL_STEP: f64 = 15.0;

#[derive(Default)]
pub(crate) struct DeviceState {
    globals: Vec<Global>,
    outputs: Vec<Output>,
}

impl GlobalSink for DeviceState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

impl OutputSink for DeviceState {
    fn outputs(&mut self) -> &mut Vec<Output> {
        &mut self.outputs
    }
}

wayland_client::delegate_dispatch!(DeviceState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(DeviceState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_dispatch!(DeviceState: [WlOutput: usize] => OutputHandler);
wayland_client::delegate_dispatch!(DeviceState: [ZxdgOutputV1: usize] => OutputHandler);
wayland_client::delegate_noop!(DeviceState: ignore ZxdgOutputManagerV1);
wayland_client::delegate_noop!(DeviceState: ignore WlSeat);
wayland_client::delegate_noop!(DeviceState: ignore ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(DeviceState: ignore ZwpVirtualKeyboardV1);
wayland_client::delegate_noop!(DeviceState: ignore ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(DeviceState: ignore ZwlrVirtualPointerV1);

/// The virtual devices of one delivery on one connection.
pub(crate) struct VirtualDevices {
    link: Link<DeviceState>,
    state: DeviceState,
    keyboard: ZwpVirtualKeyboardV1,
    pointer: Option<ZwlrVirtualPointerV1>,
    layout: Option<Rect>,
    keymap: Option<File>,
}

fn missing(protocol: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        format!("The Wayland compositor does not offer {protocol}"),
    )
    .with_suggestion("Run on a compositor that offers the wlroots virtual input protocols")
    .with_details(json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn not_started(mut error: AdapterError) -> AdapterError {
    error.details.get_or_insert_with(|| json!({}))["physical_delivery_started"] = false.into();
    error.with_disposition(DeliverySemantics::not_delivered())
}

/// The compositor clock: the monotonic clock in milliseconds, which wraps
/// like the protocol's 32-bit timestamps.
fn now_ms() -> u32 {
    let now = clock_gettime(ClockId::Monotonic);
    let millis = now.tv_sec as u64 * 1_000 + now.tv_nsec as u64 / 1_000_000;
    millis as u32
}

fn keymap_file(text: &str) -> Result<(File, u32), AdapterError> {
    let fd = memfd_create("agent-desktop-keymap", MemfdFlags::CLOEXEC)
        .map_err(|error| AdapterError::internal(format!("memfd_create failed: {error}")))?;
    let mut file = File::from(fd);
    file.write_all(text.as_bytes())
        .and_then(|()| file.write_all(&[0]))
        .map_err(|error| AdapterError::internal(format!("writing the keymap: {error}")))?;
    Ok((file, text.len() as u32 + 1))
}

impl VirtualDevices {
    /// Creates a virtual keyboard, and a virtual pointer when `pointer` is
    /// set, on the compositor's first seat.
    pub(crate) fn open(pointer: bool, deadline: Deadline) -> Result<Self, AdapterError> {
        let mut link = Link::<DeviceState>::open().map_err(not_started)?;
        let mut state = DeviceState::default();
        let registry = link.registry(&mut state, deadline).map_err(not_started)?;
        let globals = state.globals.clone();
        let seat: WlSeat =
            bind(&registry, &globals, 1, &link.handle, ()).ok_or_else(|| missing("a seat"))?;
        let keyboards: ZwpVirtualKeyboardManagerV1 = bind(&registry, &globals, 1, &link.handle, ())
            .ok_or_else(|| missing("zwp_virtual_keyboard_manager_v1"))?;
        let keyboard = keyboards.create_virtual_keyboard(&seat, &link.handle, ());
        let (pointer, layout) = if pointer {
            let pointers: ZwlrVirtualPointerManagerV1 =
                bind(&registry, &globals, 2, &link.handle, ())
                    .ok_or_else(|| missing("zwlr_virtual_pointer_manager_v1"))?;
            let outputs = read_outputs(&mut link, &mut state, &registry, &globals, deadline)
                .map_err(not_started)?;
            let outputs: Vec<Output> = outputs.into_iter().map(|(_, output)| output).collect();
            let layout = layout_bounds(&outputs).ok_or_else(|| missing("a sized output"))?;
            (
                Some(pointers.create_virtual_pointer(Some(&seat), &link.handle, ())),
                Some(layout),
            )
        } else {
            (None, None)
        };
        link.roundtrip(&mut state, deadline).map_err(not_started)?;
        Ok(Self {
            link,
            state,
            keyboard,
            pointer,
            layout,
            keymap: None,
        })
    }

    fn pointer(&self) -> Result<&ZwlrVirtualPointerV1, AdapterError> {
        self.pointer
            .as_ref()
            .ok_or_else(|| AdapterError::internal("The delivery opened no virtual pointer"))
    }
}

impl KeyDevice for VirtualDevices {
    fn upload_keymap(&mut self, text: &str) -> Result<(), AdapterError> {
        let (file, size) = keymap_file(text)?;
        self.keyboard
            .keymap(KeymapFormat::XkbV1.into(), file.as_fd(), size);
        self.keymap = Some(file);
        Ok(())
    }

    fn key(&mut self, evdev: u32, down: bool) -> Result<(), AdapterError> {
        self.keyboard.key(now_ms(), evdev, u32::from(down));
        Ok(())
    }

    fn modifiers(&mut self, depressed: u32) -> Result<(), AdapterError> {
        self.keyboard.modifiers(depressed, 0, 0, 0);
        Ok(())
    }

    fn sync(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        self.link.roundtrip(&mut self.state, deadline)
    }

    fn close(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        if let Some(pointer) = self.pointer.take() {
            pointer.destroy();
        }
        self.keyboard.destroy();
        self.link.roundtrip(&mut self.state, deadline)
    }
}

impl PointerDevice for VirtualDevices {
    fn layout(&self) -> Option<Rect> {
        self.layout
    }

    fn motion(&mut self, x: f64, y: f64) -> Result<(), AdapterError> {
        let layout = self.layout.ok_or_else(|| missing("a sized output"))?;
        let (dx, dy) = (x - layout.x, y - layout.y);
        if !(0.0..layout.width).contains(&dx) || !(0.0..layout.height).contains(&dy) {
            return Err(protocol_failed(
                "the pointer position is outside every output",
            ));
        }
        let pointer = self.pointer()?;
        pointer.motion_absolute(
            now_ms(),
            dx.round() as u32,
            dy.round() as u32,
            layout.width as u32,
            layout.height as u32,
        );
        pointer.frame();
        Ok(())
    }

    fn button(&mut self, button: u32, down: bool) -> Result<(), AdapterError> {
        let pointer = self.pointer()?;
        let state = if down {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        pointer.button(now_ms(), button, state);
        pointer.frame();
        Ok(())
    }

    fn wheel(&mut self, vertical: bool, steps: i32) -> Result<(), AdapterError> {
        let pointer = self.pointer()?;
        let axis = if vertical {
            Axis::VerticalScroll
        } else {
            Axis::HorizontalScroll
        };
        pointer.axis_source(AxisSource::Wheel);
        pointer.axis_discrete(now_ms(), axis, WHEEL_STEP * f64::from(steps), steps);
        pointer.frame();
        Ok(())
    }

    fn sync(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        self.link.roundtrip(&mut self.state, deadline)
    }
}

impl Drop for VirtualDevices {
    fn drop(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            pointer.destroy();
        }
        let _ = self.link.connection.flush();
    }
}
