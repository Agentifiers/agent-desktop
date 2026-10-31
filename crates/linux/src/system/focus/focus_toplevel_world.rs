//! The live desktop the wlroots focus reads: the window's frame through the
//! focus world every route shares, and a virtual keyboard it attaches when
//! the seat has none, held for as long as this world lives.

use std::cell::RefCell;

use agent_desktop_core::{AdapterError, Deadline};

use super::focus_frame_view::FrameView;
use super::focus_window::FocusWorld;
use super::focus_wlroots::ToplevelFocusWorld;
use super::focus_world::LiveWorld;
use crate::input::wlroots::key_device::KeyDevice;
use crate::input::wlroots::keymap_layout::KeymapLayout;
use crate::input::wlroots::virtual_devices::VirtualDevices;
use crate::system::process::process_lookup::Lookup;
use crate::system::wlroots::toplevel_match::app_names;

pub(in crate::system) struct ToplevelLiveWorld<'w, 'a> {
    pub(in crate::system) frame: &'w LiveWorld<'a>,
    pub(in crate::system) keyboard: RefCell<Option<VirtualDevices>>,
}

impl ToplevelFocusWorld for ToplevelLiveWorld<'_, '_> {
    fn lookup(&self, pid: u32) -> Lookup {
        FocusWorld::lookup(self.frame, pid)
    }

    fn stopped(&self, pid: u32) -> bool {
        FocusWorld::stopped(self.frame, pid)
    }

    fn app_names(&self, pid: u32) -> Vec<String> {
        app_names(pid)
    }

    fn frame(&self) -> Result<FrameView, AdapterError> {
        FocusWorld::frame(self.frame)
    }

    fn app_frame_active(&self) -> bool {
        FocusWorld::app_frame_active(self.frame)
    }

    fn menu_open(&self) -> bool {
        FocusWorld::menu_open(self.frame)
    }

    fn arm_wake(&self) {
        FocusWorld::arm_wake(self.frame);
    }

    fn wait_for_change(&self, deadline: Deadline) {
        FocusWorld::wait_for_change(self.frame, deadline);
    }

    fn attach_keyboard(&self, deadline: Deadline) -> Result<(), AdapterError> {
        let mut held = self.keyboard.borrow_mut();
        if held.is_some() {
            return Ok(());
        }
        let mut devices = VirtualDevices::open(false, deadline)?;
        devices.upload_keymap(&KeymapLayout::default().text())?;
        devices.sync(deadline)?;
        *held = Some(devices);
        Ok(())
    }
}
