use std::cell::RefCell;

use agent_desktop_core::{DisplayInfo, ErrorCode};

use super::*;
use crate::system::capture::window::fakes::*;
use crate::system::capture::wlroots_window::fakes::{image, output};

fn two() -> Vec<DisplayInfo> {
    vec![
        display_at("A", 0.0, 1920.0, 1.0, true),
        display_at("B", 1920.0, 1280.0, 2.0, false),
    ]
}

fn outputs() -> Vec<Output> {
    vec![output("A", 0, 1920, 1.0), output("B", 1920, 1280, 2.0)]
}

fn run(rig: &Rig, target: ScreenshotTarget) -> (Result<ImageBuffer, AdapterError>, Vec<String>) {
    let asked = RefCell::new(Vec::new());
    let copy = |name: &str, _: Deadline| {
        asked.borrow_mut().push(name.to_owned());
        let found = outputs().into_iter().find(|output| output.name == name);
        found
            .map(|output| (output, image(3, 2, None)))
            .ok_or_else(|| AdapterError::internal("no such output"))
    };
    let result = capture(&rig.env(), &target, deadline(), &copy);
    (result, asked.into_inner())
}

#[test]
fn the_full_screen_is_the_primary_output_in_the_display_scale() {
    let rig = Rig::standard();
    rig.lookup.displays.set(vec![Some(two())]);
    let (result, asked) = run(&rig, ScreenshotTarget::FullScreen);
    let image = result.expect("an image");
    assert_eq!(asked, vec!["A".to_owned()]);
    assert_eq!((image.width, image.height, image.scale_factor), (3, 2, 1.0));
    assert_eq!(&image.data[..3], &[0, 0, 9]);
}

#[test]
fn a_screen_index_copies_the_output_at_that_place_in_the_display_list() {
    let rig = Rig::standard();
    rig.lookup.displays.set(vec![Some(two())]);
    let (result, asked) = run(&rig, ScreenshotTarget::Screen(1));
    assert_eq!(asked, vec!["B".to_owned()]);
    assert_eq!(result.expect("an image").scale_factor, 2.0);
    let expected = two()[1].clone();
    let (result, asked) = run(&rig, ScreenshotTarget::Display { index: 1, expected });
    assert!(result.is_ok());
    assert_eq!(asked, vec!["B".to_owned()]);
}

#[test]
fn a_screen_out_of_range_copies_nothing() {
    let rig = Rig::standard();
    rig.lookup.displays.set(vec![Some(two())]);
    let (result, asked) = run(&rig, ScreenshotTarget::Screen(2));
    assert_eq!(
        result.expect_err("out of range").code,
        ErrorCode::InvalidArgs
    );
    assert!(asked.is_empty());
}

#[test]
fn an_output_that_is_no_longer_the_listed_display_is_refused() {
    let rig = Rig::standard();
    let mut moved = two();
    moved[0].bounds.x = 10.0;
    rig.lookup.displays.set(vec![Some(moved)]);
    let (result, _) = run(&rig, ScreenshotTarget::FullScreen);
    assert_eq!(result.expect_err("moved").code, ErrorCode::InvalidArgs);
}

#[test]
fn a_display_list_that_changes_during_the_copy_is_refused() {
    let rig = Rig::standard();
    let mut after = two();
    after[0].scale = 2.0;
    rig.lookup.displays.set(vec![Some(two()), Some(after)]);
    let (result, asked) = run(&rig, ScreenshotTarget::FullScreen);
    assert_eq!(result.expect_err("changed").code, ErrorCode::InvalidArgs);
    assert_eq!(asked.len(), 1);
}
