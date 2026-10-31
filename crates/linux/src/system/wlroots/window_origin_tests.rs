use super::*;

fn image(width: u32, height: u32, margin: u32) -> CapturedImage {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let inside = x >= margin && y >= margin && x < width - margin && y < height - margin;
            rgba.extend_from_slice(&[1, 2, 3, if inside { 0xff } else { 0 }]);
        }
    }
    CapturedImage {
        width,
        height,
        rgba,
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
fn a_frame_the_size_of_the_surface_counts_from_the_surface_corner() {
    let surface = image(1050, 700, 16);
    assert_eq!(
        window_offset(&surface, frame(0, 0, 1050, 700)),
        Some((0, 0))
    );
}

#[test]
fn a_frame_the_size_of_the_opaque_part_counts_from_inside_the_shadow() {
    let surface = image(410, 666, 25);
    assert_eq!(
        window_offset(&surface, frame(0, 0, 360, 616)),
        Some((25, 25))
    );
}

#[test]
fn a_frame_of_any_other_size_proves_no_offset() {
    let surface = image(410, 666, 25);
    assert_eq!(window_offset(&surface, frame(0, 0, 400, 616)), None);
}

#[test]
fn a_frame_that_does_not_start_at_zero_shifts_the_offset() {
    let surface = image(410, 666, 25);
    assert_eq!(
        window_offset(&surface, frame(0, 46, 360, 616)),
        Some((25, -21))
    );
}
