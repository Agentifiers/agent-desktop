use agent_desktop_core::ErrorCode;

use super::*;
use crate::system::capture::wlroots_window::fakes::{black_margins, image};

fn area(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn size(width: f64, height: f64) -> Option<Rect> {
    Some(Rect {
        x: 40.0,
        y: 30.0,
        width,
        height,
    })
}

#[test]
fn the_content_box_leaves_out_a_translucent_shadow() {
    let shadowed = image(10, 8, Some(area(2, 1, 5, 4)));
    assert_eq!(content_box(&shadowed), Some(area(2, 1, 5, 4)));
}

#[test]
fn the_content_box_leaves_out_a_shadow_read_as_plain_black() {
    let flat = black_margins(10, 8, area(2, 1, 5, 4));
    assert_eq!(content_box(&flat), Some(area(2, 1, 5, 4)));
}

#[test]
fn a_surface_drawn_to_its_edges_is_its_own_box() {
    let plain = image(6, 4, Some(area(0, 0, 6, 4)));
    assert_eq!(content_box(&plain), Some(area(0, 0, 6, 4)));
}

#[test]
fn a_surface_with_no_content_pixel_has_no_box() {
    assert_eq!(content_box(&image(6, 4, None)), None);
}

#[test]
fn without_a_size_the_window_is_the_whole_surface() {
    let shadowed = image(10, 8, Some(area(2, 1, 5, 4)));
    assert_eq!(
        window_area(&shadowed, None, 1.0, false).expect("an area"),
        area(0, 0, 10, 8)
    );
}

#[test]
fn the_size_picks_the_surface_or_the_box_that_agrees_at_the_scale() {
    let shadowed = black_margins(10, 8, area(2, 1, 5, 4));
    assert_eq!(
        window_area(&shadowed, size(10.0, 8.0), 1.0, true).expect("the surface"),
        area(0, 0, 10, 8)
    );
    assert_eq!(
        window_area(&shadowed, size(2.5, 2.0), 2.0, true).expect("the box"),
        area(2, 1, 5, 4)
    );
}

#[test]
fn a_size_that_agrees_with_nothing_is_refused_when_strict_and_kept_whole_otherwise() {
    let shadowed = image(10, 8, Some(area(2, 1, 5, 4)));
    let error = window_area(&shadowed, size(300.0, 200.0), 1.0, true).expect_err("unproven");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    let error = window_area(&shadowed, None, 1.0, true).expect_err("no size");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        window_area(&shadowed, size(300.0, 200.0), 1.0, false).expect("lenient"),
        area(0, 0, 10, 8)
    );
}

#[test]
fn the_rgb_of_an_area_is_its_rows_without_alpha() {
    let shadowed = image(10, 8, Some(area(2, 1, 5, 4)));
    let rgb = rgb_in(&shadowed, area(2, 1, 2, 2));
    assert_eq!(rgb, vec![2, 1, 9, 3, 1, 9, 2, 2, 9, 3, 2, 9]);
}
