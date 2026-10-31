use agent_desktop_core::{Deadline, Modifier, MouseButton, Point};

use super::super::fake_devices::FakeDevices;
use super::*;
use crate::input::mouse::pointer_plan::plan_click;
use crate::input::mouse::wheel_plan::plan_wheel;
use crate::input::step::Step;

fn sink(devices: &FakeDevices) -> WlrootsSink<FakeDevices> {
    WlrootsSink::new(devices.clone(), Deadline::after(2_000).unwrap()).expect("sink")
}

fn run(sink: &mut WlrootsSink<FakeDevices>, steps: &[Step]) {
    for step in steps {
        match step {
            Step::Key { code, down } => sink.key(*code, *down).expect("key"),
            Step::Button { code, down } => sink.button(*code, *down).expect("button"),
            Step::MoveTo { x, y } => sink.move_to(*x as i16, *y as i16).expect("move"),
            _ => {}
        }
    }
}

fn sent(devices: &FakeDevices) -> Vec<String> {
    devices
        .events()
        .into_iter()
        .filter(|event| event != "sync")
        .collect()
}

#[test]
fn the_sink_uploads_the_modifier_keymap_before_anything_else() {
    let devices = FakeDevices::default();
    sink(&devices);
    assert_eq!(devices.events(), ["keymap:4", "sync"]);
}

#[test]
fn a_ctrl_click_holds_control_around_a_left_press_at_the_point() {
    let devices = FakeDevices::default();
    let mut sink = sink(&devices);
    let map = sink.keymap().expect("keymap");
    let plan = plan_click(
        &map,
        Point { x: 300.0, y: 200.0 },
        &MouseButton::Left,
        1,
        &[Modifier::Ctrl],
    )
    .expect("plan");
    run(&mut sink, &plan.steps);
    assert_eq!(
        sent(&devices),
        [
            "keymap:4",
            "key:2:down",
            "mods:4",
            "motion:300:200",
            "button:0x110:down",
            "button:0x110:up",
            "key:2:up",
            "mods:0"
        ]
    );
}

#[test]
fn right_and_middle_clicks_use_their_evdev_buttons() {
    let devices = FakeDevices::default();
    let mut sink = sink(&devices);
    sink.button(3, true).expect("right");
    sink.button(2, true).expect("middle");
    assert_eq!(
        sent(&devices)[1..],
        ["button:0x111:down", "button:0x112:down"]
    );
}

#[test]
fn wheel_buttons_become_discrete_axis_notches_on_press_only() {
    let devices = FakeDevices::default();
    let mut sink = sink(&devices);
    let map = sink.keymap().expect("keymap");
    let plans = plan_wheel(&map, Point { x: 10.0, y: 10.0 }, 1.0, -2.0, &[]).expect("plan");
    for plan in &plans {
        run(&mut sink, &plan.steps);
    }
    assert_eq!(
        sent(&devices)[1..],
        [
            "motion:10:10",
            "wheel:v:1",
            "wheel:v:1",
            "motion:10:10",
            "wheel:h:1"
        ]
    );
}

#[test]
fn the_sink_cannot_read_held_input_and_says_so() {
    let devices = FakeDevices::default();
    let mut sink = sink(&devices);
    assert!(!sink.reads_held_input());
    assert!(sink.pointer().is_err());
    assert!(sink.keys_down().is_err());
    assert_eq!(sink.root_size().expect("size"), (1280, 720));
}

#[test]
fn a_keycode_outside_the_modifier_keymap_and_a_remap_are_refused() {
    let devices = FakeDevices::default();
    let mut sink = sink(&devices);
    assert!(sink.key(40, true).is_err());
    assert!(
        sink.edit_keymap(&KeymapEdit {
            keycode: 9,
            keysyms: vec![0x61],
        })
        .is_err()
    );
    assert!(sink.button(9, true).is_err());
}
