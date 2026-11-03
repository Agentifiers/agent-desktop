use super::{Step, displacement};
use crate::tree::Extents;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn step(x: i32, y: i32, width: i32, height: i32, children: usize) -> Step {
    Step {
        extents: rect(x, y, width, height),
        children,
    }
}

/// The parent-coordinate rectangles and child counts read from GNOME
/// GNOME Calculator on GTK 4, maximized on a 1280x720 output, from the 7
/// button's parent up to the frame's child. The keypad's stack page and its
/// child both read (12, 12): the button is drawn at (302, 520) while its
/// window extents read (314, 532).
fn calculator_seven() -> (Extents, Vec<Step>) {
    let ancestors = vec![
        step(0, 0, 676, 236, 24),
        step(12, 12, 676, 236, 1),
        step(12, 12, 676, 236, 1),
        step(0, 0, 700, 260, 1),
        step(0, 414, 700, 260, 1),
        step(290, 0, 700, 674, 2),
        step(0, 46, 1280, 674, 1),
        step(0, 0, 1280, 720, 3),
        step(0, 0, 1280, 720, 1),
        step(0, 0, 1280, 720, 1),
        step(0, 0, 1280, 720, 1),
    ];
    (rect(0, 48, 132, 44), ancestors)
}

#[test]
fn the_calculator_keypad_page_is_counted_once() {
    let (node, ancestors) = calculator_seven();
    assert_eq!(displacement(node, &ancestors), Some((12, 12)));
}

#[test]
fn a_node_with_no_page_above_it_is_where_it_reads() {
    let ancestors = [step(10, 20, 300, 200, 3), step(0, 46, 1280, 674, 2)];
    assert_eq!(displacement(rect(4, 4, 40, 20), &ancestors), Some((0, 0)));
}

#[test]
fn nested_pages_each_add_their_offset() {
    let ancestors = [
        step(8, 6, 100, 50, 1),
        step(8, 6, 100, 50, 1),
        step(30, 40, 400, 300, 1),
        step(30, 40, 400, 300, 1),
        step(0, 0, 800, 600, 1),
    ];
    assert_eq!(displacement(rect(2, 2, 10, 10), &ancestors), Some((38, 46)));
}

#[test]
fn a_page_or_wrapper_at_its_parent_corner_adds_nothing() {
    let ancestors = [
        step(0, 0, 640, 480, 1),
        step(0, 0, 640, 480, 1),
        step(0, 0, 640, 480, 1),
    ];
    assert_eq!(displacement(rect(0, 0, 640, 480), &ancestors), Some((0, 0)));
}

#[test]
fn a_container_listing_several_children_is_never_a_page() {
    let ancestors = [step(12, 12, 676, 236, 2), step(0, 0, 700, 260, 1)];
    assert_eq!(
        displacement(rect(12, 12, 676, 236), &ancestors),
        Some((0, 0))
    );
}

#[test]
fn a_page_only_moves_the_nodes_beneath_it() {
    let ancestors = [step(0, 0, 700, 260, 1), step(0, 414, 700, 260, 1)];
    assert_eq!(
        displacement(rect(12, 12, 676, 236), &ancestors),
        Some((0, 0))
    );
}

#[test]
fn an_offset_past_the_coordinate_range_is_unknown() {
    let ancestors = [
        step(i32::MAX, 0, 10, 10, 1),
        step(i32::MAX, 0, 10, 10, 1),
        step(i32::MAX, 0, 10, 10, 1),
    ];
    assert_eq!(displacement(rect(i32::MAX, 0, 10, 10), &ancestors), None);
}
