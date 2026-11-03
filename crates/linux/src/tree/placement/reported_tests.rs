use super::Reported;
use crate::tree::Extents;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_node_reported_where_it_is_drawn_is_unchanged() {
    let node = rect(302, 520, 132, 44);
    assert_eq!(Reported::DRAWN.drawn(node), Some(node));
    assert_eq!(Reported::DRAWN.reported_point((368, 542)), Some((368, 542)));
}

/// The Calculator 7 button reads (314, 532) and is drawn at (302, 520): the
/// keypad's stack page counted twice.
#[test]
fn a_gtk4_node_in_a_stack_page_is_drawn_back_by_the_page_offset() {
    let page = Reported {
        scale: 1.0,
        shift: (12, 12),
    };
    assert_eq!(
        page.drawn(rect(314, 532, 132, 44)),
        Some(rect(302, 520, 132, 44))
    );
    assert_eq!(page.reported_point((368, 542)), Some((380, 554)));
}

/// The page button drawn at (200, 237, 240, 90) reads (320, 379, 384, 144)
/// in Chromium's device pixels at a scale of eight fifths.
#[test]
fn chromium_web_content_in_device_pixels_is_drawn_in_layout_units() {
    let device = Reported {
        scale: 1.6,
        shift: (0, 0),
    };
    assert_eq!(
        device.drawn(rect(320, 379, 384, 144)),
        Some(rect(200, 237, 240, 90))
    );
    assert_eq!(device.reported_point((320, 282)), Some((512, 451)));
}

#[test]
fn a_rectangle_or_scale_past_the_coordinate_range_is_unknown() {
    let device = Reported {
        scale: 1.6,
        shift: (0, 0),
    };
    assert_eq!(device.drawn(rect(i32::MAX, 0, 10, 10)), None);
    assert_eq!(device.reported_point((i32::MAX, 0)), None);
    let broken = Reported {
        scale: 0.0,
        shift: (0, 0),
    };
    assert_eq!(broken.drawn(rect(0, 0, 10, 10)), None);
}
