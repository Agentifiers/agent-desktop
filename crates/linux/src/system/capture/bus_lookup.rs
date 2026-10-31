use super::window_context::WindowLookup;
use super::window_xid::{XidOutcome, window_xid};
use crate::system::SharedBus;
use crate::system::window::window_id;
use crate::tree::AtspiElement;
use crate::tree::surfaces::window_extents::read_window_extents;
use agent_desktop_core::{AdapterError, Deadline, DisplayInfo, WindowInfo};

pub(crate) struct BusLookup<'a> {
    pub(crate) bus: &'a SharedBus,
}

impl WindowLookup for BusLookup<'_> {
    fn resolve(&self, window: &WindowInfo, deadline: Deadline) -> Result<WindowInfo, AdapterError> {
        crate::system::resolve_window_strict(self.bus, window, deadline)
    }

    fn xid(&self, window: &WindowInfo, deadline: Deadline) -> XidOutcome {
        window_xid(self.bus, window, deadline)
    }

    fn displays(&self, deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        crate::system::list_displays(deadline)
    }

    fn frame_size(&self, window: &WindowInfo, deadline: Deadline) -> Option<(i32, i32)> {
        let (bus_name, path) = window_id::decode(&window.id).ok()?;
        let frame = AtspiElement::new(bus_name, path, window.pid.get());
        let extents = read_window_extents(self.bus, &frame, deadline)?;
        (extents.width > 0 && extents.height > 0).then_some((extents.width, extents.height))
    }
}
