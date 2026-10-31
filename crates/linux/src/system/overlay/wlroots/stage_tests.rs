use agent_desktop_core::Point;

use super::locate;
use crate::system::wlroots::output::Output;

fn output(name: &str, x: i32, width: i32) -> Output {
    Output {
        name: name.into(),
        x,
        y: 0,
        width,
        height: 720,
        scale: 1.0,
        logical: true,
    }
}

#[test]
fn a_point_is_placed_on_the_output_that_holds_it_relative_to_its_corner() {
    let outputs = [output("A", 0, 1280), output("B", 1280, 1920)];
    assert_eq!(
        locate(&outputs, &Point { x: 0.0, y: 0.0 }),
        Some((0, (0, 0)))
    );
    assert_eq!(
        locate(&outputs, &Point { x: 640.4, y: 359.6 }),
        Some((0, (640, 360)))
    );
    assert_eq!(
        locate(&outputs, &Point { x: 1280.0, y: 10.0 }),
        Some((1, (0, 10)))
    );
}

#[test]
fn a_point_outside_every_output_has_no_place() {
    let outputs = [output("A", 0, 1280)];
    for (x, y) in [(1280.0, 0.0), (-1.0, 5.0), (5.0, 720.0)] {
        assert_eq!(locate(&outputs, &Point { x, y }), None, "({x}, {y})");
    }
    assert_eq!(locate(&[], &Point { x: 1.0, y: 1.0 }), None);
}
