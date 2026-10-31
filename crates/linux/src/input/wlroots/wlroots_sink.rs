use agent_desktop_core::{AdapterError, Deadline};

use super::key_device::KeyDevice;
use super::keymap_layout::{KeymapLayout, modifier_mask};
use super::pointer_device::PointerDevice;
use crate::input::keyboard::keymap::Keymap;
use crate::input::keyboard::keymap_edit::KeymapEdit;
use crate::input::mouse::pointer_plan::{BUTTON_LEFT, BUTTON_MIDDLE, BUTTON_RIGHT};
use crate::input::mouse::wheel_plan::{
    BUTTON_WHEEL_DOWN, BUTTON_WHEEL_LEFT, BUTTON_WHEEL_RIGHT, BUTTON_WHEEL_UP,
};
use crate::input::x11::pointer_read::PointerRead;
use crate::input::x11::sink::InputSink;
use crate::input::x11::sink_error::SinkError;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const EVDEV_OFFSET: u8 = 8;

/// Plan steps on a wlroots compositor: keys on a virtual keyboard whose
/// keymap holds the four modifiers, buttons, wheel notches and absolute
/// motion on a virtual pointer. Plans address keys by the X keycodes of
/// [`KeymapLayout::as_keymap`].
pub(crate) struct WlrootsSink<D> {
    device: D,
    layout: KeymapLayout,
    held: Vec<u32>,
    deadline: Deadline,
}

fn request(error: AdapterError) -> SinkError {
    SinkError::Request(match error.platform_detail {
        Some(detail) => format!("{}: {detail}", error.message),
        None => error.message,
    })
}

impl<D: KeyDevice + PointerDevice> WlrootsSink<D> {
    /// Takes `device` and uploads the modifier keymap, before any key.
    pub(crate) fn new(mut device: D, deadline: Deadline) -> Result<Self, AdapterError> {
        let layout = KeymapLayout::default();
        device.upload_keymap(&layout.text())?;
        PointerDevice::sync(&mut device, deadline)?;
        Ok(Self {
            device,
            layout,
            held: Vec::new(),
            deadline,
        })
    }

    fn sync(&mut self) -> Result<(), SinkError> {
        PointerDevice::sync(&mut self.device, self.deadline).map_err(request)
    }
}

impl<D: KeyDevice + PointerDevice> InputSink for WlrootsSink<D> {
    fn key(&mut self, code: u8, down: bool) -> Result<(), SinkError> {
        let keysym = self.layout.keysym_at(code).ok_or_else(|| {
            SinkError::Request(format!("keycode {code} is outside the virtual keymap"))
        })?;
        self.device
            .key(u32::from(code - EVDEV_OFFSET), down)
            .map_err(request)?;
        if down {
            self.held.push(keysym);
        } else {
            self.held.retain(|held| *held != keysym);
        }
        if modifier_mask(keysym) != 0 {
            let mask = self
                .held
                .iter()
                .fold(0, |mask, held| mask | modifier_mask(*held));
            self.device.modifiers(mask).map_err(request)?;
        }
        Ok(())
    }

    fn button(&mut self, code: u8, down: bool) -> Result<(), SinkError> {
        let wheel = |vertical: bool, steps: i32| (vertical, steps);
        let pressed = match code {
            BUTTON_LEFT => BTN_LEFT,
            BUTTON_MIDDLE => BTN_MIDDLE,
            BUTTON_RIGHT => BTN_RIGHT,
            BUTTON_WHEEL_UP | BUTTON_WHEEL_DOWN | BUTTON_WHEEL_LEFT | BUTTON_WHEEL_RIGHT => {
                if !down {
                    return Ok(());
                }
                let (vertical, steps) = match code {
                    BUTTON_WHEEL_UP => wheel(true, -1),
                    BUTTON_WHEEL_DOWN => wheel(true, 1),
                    BUTTON_WHEEL_LEFT => wheel(false, -1),
                    _ => wheel(false, 1),
                };
                return self.device.wheel(vertical, steps).map_err(request);
            }
            other => {
                return Err(SinkError::Request(format!(
                    "button {other} has no virtual pointer equivalent"
                )));
            }
        };
        self.device.button(pressed, down).map_err(request)
    }

    fn move_to(&mut self, x: i16, y: i16) -> Result<(), SinkError> {
        self.device
            .motion(f64::from(x), f64::from(y))
            .map_err(request)
    }

    fn round_trip(&mut self) -> Result<(), SinkError> {
        self.sync()
    }

    fn pointer(&mut self) -> Result<PointerRead, SinkError> {
        Err(SinkError::Request(
            "a virtual pointer has no position read".to_owned(),
        ))
    }

    fn keys_down(&mut self) -> Result<[u8; 32], SinkError> {
        Err(SinkError::Request(
            "a virtual keyboard has no key state read".to_owned(),
        ))
    }

    fn keymap(&mut self) -> Result<Keymap, SinkError> {
        Ok(self.layout.as_keymap())
    }

    fn ensure_text_state(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn edit_keymap(&mut self, edit: &KeymapEdit) -> Result<(), SinkError> {
        Err(SinkError::Request(format!(
            "the virtual keymap is fixed; keycode {} cannot be remapped",
            edit.keycode
        )))
    }

    fn root_size(&mut self) -> Result<(u16, u16), SinkError> {
        let layout = self
            .device
            .layout()
            .ok_or_else(|| SinkError::Request("no output has a size".to_owned()))?;
        let clamp = |value: f64| value.clamp(0.0, f64::from(u16::MAX)) as u16;
        Ok((
            clamp(layout.x + layout.width),
            clamp(layout.y + layout.height),
        ))
    }

    fn reads_held_input(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "wlroots_sink_tests.rs"]
mod tests;
