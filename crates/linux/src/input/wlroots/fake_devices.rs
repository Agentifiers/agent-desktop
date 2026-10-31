use std::cell::RefCell;
use std::rc::Rc;

use agent_desktop_core::{AdapterError, Deadline, Rect};

use super::key_device::KeyDevice;
use super::pointer_device::PointerDevice;

/// Records what a delivery sent to the virtual devices, in order, and fails
/// the step whose label matches `fail_on`.
#[derive(Clone, Default)]
pub(super) struct FakeDevices {
    log: Rc<RefCell<Vec<String>>>,
    pub(super) fail_on: Option<&'static str>,
}

impl FakeDevices {
    pub(super) fn failing_on(label: &'static str) -> Self {
        Self {
            fail_on: Some(label),
            ..Self::default()
        }
    }

    pub(super) fn events(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn record(&self, label: String) -> Result<(), AdapterError> {
        let failing = self.fail_on == Some(label.as_str());
        self.log.borrow_mut().push(label);
        if failing {
            return Err(AdapterError::internal("the fake failed this step"));
        }
        Ok(())
    }
}

impl KeyDevice for FakeDevices {
    fn upload_keymap(&mut self, text: &str) -> Result<(), AdapterError> {
        let keys = text.matches("key <").count();
        self.record(format!("keymap:{keys}"))
    }

    fn key(&mut self, evdev: u32, down: bool) -> Result<(), AdapterError> {
        self.record(format!("key:{evdev}:{}", if down { "down" } else { "up" }))
    }

    fn modifiers(&mut self, depressed: u32) -> Result<(), AdapterError> {
        self.record(format!("mods:{depressed}"))
    }

    fn sync(&mut self, _deadline: Deadline) -> Result<(), AdapterError> {
        self.record("sync".into())
    }

    fn close(&mut self, _deadline: Deadline) -> Result<(), AdapterError> {
        self.record("close".into())
    }
}

impl PointerDevice for FakeDevices {
    fn layout(&self) -> Option<Rect> {
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        })
    }

    fn motion(&mut self, x: f64, y: f64) -> Result<(), AdapterError> {
        self.record(format!("motion:{x}:{y}"))
    }

    fn button(&mut self, button: u32, down: bool) -> Result<(), AdapterError> {
        self.record(format!(
            "button:{button:#x}:{}",
            if down { "down" } else { "up" }
        ))
    }

    fn wheel(&mut self, vertical: bool, steps: i32) -> Result<(), AdapterError> {
        let axis = if vertical { "v" } else { "h" };
        self.record(format!("wheel:{axis}:{steps}"))
    }

    fn sync(&mut self, _deadline: Deadline) -> Result<(), AdapterError> {
        self.record("sync".into())
    }
}
