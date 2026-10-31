use super::*;
use crate::system::wlroots::toplevel_flags::ToplevelFlags;

fn output(name: &str, x: i32, y: i32) -> Output {
    Output {
        name: name.to_owned(),
        x,
        y,
        width: 1280,
        height: 720,
        scale: 1.0,
        logical: true,
    }
}

fn toplevel(maximized: bool, fullscreen: bool, outputs: &[&str]) -> ToplevelState {
    ToplevelState {
        title: "Calculator".to_owned(),
        flags: ToplevelFlags {
            maximized,
            fullscreen,
            activated: true,
            minimized: false,
        },
        outputs: outputs.iter().map(|name| (*name).to_owned()).collect(),
        ..ToplevelState::default()
    }
}

fn frame(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_maximized_client_decorated_frame_starts_at_the_output_corner() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1280, 720),
    );
    assert_eq!(found, Some((0, 0)));
}

#[test]
fn a_server_title_bar_puts_the_frame_below_it() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1280, 694),
    );
    assert_eq!(found, Some((0, 26)));
}

#[test]
fn the_output_position_and_the_frame_offset_both_count() {
    let outputs = [output("A", 0, 0), output("B", 1280, 100)];
    let found = derive(
        &toplevel(false, true, &["B"]),
        &outputs,
        frame(4, 2, 1280, 720),
    );
    assert_eq!(found, Some((1276, 98)));
}

#[test]
fn a_frame_narrower_than_the_output_is_not_placed() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1248, 720),
    );
    assert_eq!(found, None);
}

#[test]
fn a_frame_taller_than_the_output_is_not_placed() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1280, 863),
    );
    assert_eq!(found, None);
}

#[test]
fn a_gap_taller_than_a_title_bar_is_not_placed() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1280, 600),
    );
    assert_eq!(found, None);
}

#[test]
fn a_fullscreen_toplevel_has_no_title_bar() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let found = derive(
        &toplevel(false, true, &["HEADLESS-1"]),
        &outputs,
        frame(0, 0, 1280, 694),
    );
    assert_eq!(found, None);
}

#[test]
fn a_floating_or_minimized_toplevel_is_not_placed() {
    let outputs = [output("HEADLESS-1", 0, 0)];
    let floating = toplevel(false, false, &["HEADLESS-1"]);
    assert_eq!(derive(&floating, &outputs, frame(0, 0, 1280, 720)), None);
    let mut minimized = toplevel(true, false, &["HEADLESS-1"]);
    minimized.flags.minimized = true;
    assert_eq!(derive(&minimized, &outputs, frame(0, 0, 1280, 720)), None);
}

#[test]
fn the_output_must_be_named_unless_it_is_the_only_one() {
    let one = [output("HEADLESS-1", 0, 0)];
    let unnamed = toplevel(true, false, &[]);
    assert_eq!(derive(&unnamed, &one, frame(0, 0, 1280, 720)), Some((0, 0)));
    let two = [output("A", 0, 0), output("B", 1280, 0)];
    assert_eq!(derive(&unnamed, &two, frame(0, 0, 1280, 720)), None);
    let spanning = toplevel(true, false, &["A", "B"]);
    assert_eq!(derive(&spanning, &two, frame(0, 0, 1280, 720)), None);
    let elsewhere = toplevel(true, false, &["C"]);
    assert_eq!(derive(&elsewhere, &two, frame(0, 0, 1280, 720)), None);
}

#[test]
fn an_output_without_a_layout_position_is_not_used() {
    let mut unplaced = output("HEADLESS-1", 0, 0);
    unplaced.logical = false;
    let found = derive(
        &toplevel(true, false, &["HEADLESS-1"]),
        &[unplaced],
        frame(0, 0, 1280, 720),
    );
    assert_eq!(found, None);
}
