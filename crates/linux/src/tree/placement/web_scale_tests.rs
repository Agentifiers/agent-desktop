use super::scale_of;
use crate::tree::Extents;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

/// Chromium maximized on a 928x1424 output at a scale of 410 256ths (logical
/// 579x889): the page's container reads layout units and the document the
/// same area in device pixels, at the scale of 192 120ths Wayland hands the
/// client.
#[test]
fn a_document_in_device_pixels_reads_the_wayland_scale() {
    assert_eq!(
        scale_of(rect(0, 139, 927, 1284), rect(0, 87, 579, 802)),
        Some(1.6)
    );
}

#[test]
fn a_document_in_layout_units_reads_one() {
    assert_eq!(
        scale_of(rect(0, 87, 1280, 633), rect(0, 87, 1280, 633)),
        Some(1.0)
    );
    assert_eq!(
        scale_of(rect(0, 131, 1920, 950), rect(0, 87, 1280, 633)),
        Some(1.5)
    );
}

#[test]
fn a_document_that_does_not_fill_its_container_at_one_scale_reads_nothing() {
    assert_eq!(
        scale_of(rect(0, 139, 927, 802), rect(0, 87, 579, 802)),
        None
    );
    assert_eq!(
        scale_of(rect(40, 139, 927, 1284), rect(0, 87, 579, 802)),
        None
    );
    assert_eq!(scale_of(rect(0, 0, 0, 0), rect(0, 87, 579, 802)), None);
    assert_eq!(scale_of(rect(0, 139, 927, 1284), rect(0, 87, 0, 802)), None);
}
