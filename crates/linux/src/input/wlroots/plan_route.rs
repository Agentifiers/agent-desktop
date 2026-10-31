//! Pointer plans and modifier chords on a wlroots compositor, through the
//! same worker, gates and release guard as the X11 route.

use agent_desktop_core::{AdapterError, InteractionLease};

use super::keymap_layout::KeymapLayout;
use super::virtual_devices::VirtualDevices;
use super::wlroots_sink::WlrootsSink;
use crate::input::delivery::{Delivery, Gate};
use crate::input::keyboard::keymap::Keymap;
use crate::input::plan::Plan;
use crate::input::x11::session::{CallEnv, run_journaled};
use crate::input::x11::worker_slot;

/// The keymap plans are built against: the four modifiers only.
pub(crate) fn keymap() -> Keymap {
    KeymapLayout::default().as_keymap()
}

pub(crate) fn run(
    plans: Vec<Plan>,
    gate: Gate<'_>,
    lease: &InteractionLease,
) -> Result<Delivery, AdapterError> {
    let deadline = lease.deadline();
    run_journaled(
        move || WlrootsSink::new(VirtualDevices::open(true, deadline)?, deadline),
        plans,
        gate,
        CallEnv {
            deadline,
            journal: None,
            slot: worker_slot::shared(),
            release_on_signal: true,
            lease: Some(lease),
        },
    )
}
