use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::key_device::KeyDevice;
use super::keymap_layout::{KeymapLayout, modifier_mask};
use crate::input::wayland::key_session::KeySession;

/// A keyboard delivery through a wlroots virtual keyboard. The keymap is
/// generated from the keysyms the delivery needs, so every key is its own
/// keycode and the modifier state sent with each modifier edge is exact.
pub(crate) struct VirtualKeySession<D, O> {
    open: O,
    device: Option<D>,
    layout: KeymapLayout,
    held: Vec<u32>,
}

impl<D, O> VirtualKeySession<D, O>
where
    D: KeyDevice,
    O: FnMut(Deadline) -> Result<D, AdapterError>,
{
    pub(crate) fn new(open: O, keysyms: impl IntoIterator<Item = u32>) -> Self {
        Self {
            open,
            device: None,
            layout: KeymapLayout::with_keysyms(keysyms),
            held: Vec::new(),
        }
    }

    fn device(&mut self) -> Result<&mut D, AdapterError> {
        self.device
            .as_mut()
            .ok_or_else(|| AdapterError::internal("The virtual keyboard was not created"))
    }

    fn depressed(&self) -> u32 {
        self.held
            .iter()
            .fold(0, |mask, keysym| mask | modifier_mask(*keysym))
    }
}

fn layout_full() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "More distinct keys are held at once than a keymap can address",
    )
    .with_details(json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

impl<D, O> KeySession for VirtualKeySession<D, O>
where
    D: KeyDevice,
    O: FnMut(Deadline) -> Result<D, AdapterError>,
{
    fn create(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let mut device = (self.open)(deadline)?;
        device.upload_keymap(&self.layout.text())?;
        self.device = Some(device);
        Ok(())
    }

    fn start(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        self.device()?.sync(deadline)
    }

    fn keysym(&mut self, keysym: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError> {
        let code = if down {
            let before = self.layout.clone();
            let code = self
                .layout
                .admit(keysym, &self.held)
                .ok_or_else(layout_full)?;
            if self.layout != before {
                let text = self.layout.text();
                self.device()?.upload_keymap(&text)?;
            }
            code
        } else {
            match self.layout.evdev_code(keysym) {
                Some(code) if self.held.contains(&keysym) => code,
                _ => return Ok(()),
            }
        };
        self.device()?.key(code, down)?;
        if down {
            self.held.push(keysym);
        } else if let Some(at) = self.held.iter().rposition(|held| *held == keysym) {
            self.held.remove(at);
        }
        if modifier_mask(keysym) != 0 {
            let mask = self.depressed();
            self.device()?.modifiers(mask)?;
        }
        self.device()?.sync(deadline)
    }

    fn stop(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        self.held.clear();
        match self.device.take() {
            Some(mut device) => device.close(deadline),
            None => Ok(()),
        }
    }

    fn leads_with_shift(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "virtual_key_session_tests.rs"]
mod tests;
