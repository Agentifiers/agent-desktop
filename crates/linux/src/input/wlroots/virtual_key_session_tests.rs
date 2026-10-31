use agent_desktop_core::{Deadline, DeliverySemantics};

use super::super::fake_devices::FakeDevices;
use super::*;
use crate::input::keyboard::key_names::{KEYSYM_CONTROL_L, KEYSYM_SHIFT_L};
use crate::input::keyboard::keysym_step::KeysymStep;
use crate::input::wayland::keysym_send::run_chunks;

fn deadline() -> Deadline {
    Deadline::after(2_000).expect("deadline")
}

fn steps(keys: &[(u32, bool)]) -> Vec<KeysymStep> {
    keys.iter()
        .map(|(keysym, down)| KeysymStep {
            keysym: *keysym,
            down: *down,
        })
        .collect()
}

fn session(
    devices: &FakeDevices,
    keysyms: &[u32],
) -> VirtualKeySession<FakeDevices, impl FnMut(Deadline) -> Result<FakeDevices, AdapterError>> {
    let opened = devices.clone();
    VirtualKeySession::new(move |_| Ok(opened.clone()), keysyms.to_vec())
}

#[test]
fn a_capital_is_its_own_key_with_no_shift_and_no_lead_tap() {
    let devices = FakeDevices::default();
    let keys = steps(&[(0x42, true), (0x42, false)]);
    let mut session = session(&devices, &[0x42]);
    let delivery = run_chunks(&mut session, &[&keys], &mut || Ok(()), deadline()).expect("sent");
    assert_eq!(delivery.delivered_events, 2);
    assert_eq!(
        devices.events(),
        [
            "keymap:5",
            "sync",
            "key:5:down",
            "sync",
            "key:5:up",
            "sync",
            "close"
        ]
    );
}

#[test]
fn a_chord_sends_the_modifier_state_with_each_modifier_edge() {
    let devices = FakeDevices::default();
    let keys = steps(&[
        (KEYSYM_CONTROL_L, true),
        (KEYSYM_SHIFT_L, true),
        (0x74, true),
        (0x74, false),
        (KEYSYM_SHIFT_L, false),
        (KEYSYM_CONTROL_L, false),
    ]);
    let mut session = session(&devices, &[0x74]);
    run_chunks(&mut session, &[&keys], &mut || Ok(()), deadline()).expect("sent");
    let events = devices.events();
    let sent: Vec<&str> = events
        .iter()
        .map(String::as_str)
        .filter(|event| *event != "sync")
        .collect();
    assert_eq!(
        sent,
        [
            "keymap:5",
            "key:2:down",
            "mods:4",
            "key:1:down",
            "mods:5",
            "key:5:down",
            "key:5:up",
            "key:1:up",
            "mods:4",
            "key:2:up",
            "mods:0",
            "close"
        ]
    );
}

#[test]
fn a_key_outside_the_planned_keymap_uploads_a_new_one_before_it() {
    let devices = FakeDevices::default();
    let mut session = session(&devices, &[]);
    session.create(deadline()).expect("created");
    session.keysym(0x0100_263a, true, deadline()).expect("down");
    assert_eq!(
        devices.events(),
        ["keymap:4", "keymap:5", "key:5:down", "sync"]
    );
}

#[test]
fn a_failed_key_releases_what_is_held_and_closes_the_keyboard() {
    let devices = FakeDevices::failing_on("key:5:down");
    let keys = steps(&[
        (KEYSYM_SHIFT_L, true),
        (0x41, true),
        (0x41, false),
        (KEYSYM_SHIFT_L, false),
    ]);
    let mut session = session(&devices, &[0x41]);
    let error = run_chunks(&mut session, &[&keys], &mut || Ok(()), deadline()).expect_err("failed");
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
    let events = devices.events();
    let failed = events
        .iter()
        .position(|event| event == "key:5:down")
        .expect("tried");
    let after: Vec<&str> = events[failed + 1..]
        .iter()
        .map(String::as_str)
        .filter(|event| *event != "sync")
        .collect();
    assert_eq!(after, ["key:1:up", "mods:0", "close"]);
}

#[test]
fn a_release_of_a_key_never_pressed_sends_nothing() {
    let devices = FakeDevices::default();
    let mut session = session(&devices, &[0x61]);
    session.create(deadline()).expect("created");
    session.keysym(0x61, false, deadline()).expect("no-op");
    assert_eq!(devices.events(), ["keymap:5"]);
}

#[test]
fn a_device_that_cannot_open_sends_nothing_and_is_not_delivered() {
    let mut session = VirtualKeySession::new(
        |_| -> Result<FakeDevices, AdapterError> { Err(AdapterError::internal("no seat")) },
        [0x61],
    );
    let keys = steps(&[(0x61, true), (0x61, false)]);
    let error =
        run_chunks(&mut session, &[&keys], &mut || Ok(()), deadline()).expect_err("refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn the_virtual_keyboard_needs_no_lead_shift() {
    let devices = FakeDevices::default();
    assert!(!session(&devices, &[]).leads_with_shift());
}
