use super::*;

fn noise(seed: u32, width: u32, height: u32) -> CapturedImage {
    let mut state = seed;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..width * height {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let value = (state >> 16) as u8;
        rgba.extend_from_slice(&[value, value.wrapping_mul(3), value ^ 0x5a, 0xff]);
    }
    CapturedImage {
        width,
        height,
        rgba,
    }
}

fn set(image: &mut CapturedImage, x: u32, y: u32, pixel: [u8; 4]) {
    let at = ((y * image.width + x) * 4) as usize;
    image.rgba[at..at + 4].copy_from_slice(&pixel);
}

/// A surface with a transparent `margin` around textured opaque content.
fn surface(width: u32, height: u32, margin: u32) -> CapturedImage {
    let mut image = noise(7, width, height);
    for y in 0..height {
        for x in 0..width {
            if x < margin || y < margin || x >= width - margin || y >= height - margin {
                set(&mut image, x, y, [0, 0, 0, 0]);
            }
        }
    }
    image
}

fn draw(screen: &mut CapturedImage, surface: &CapturedImage, at: (u32, u32)) {
    for y in 0..surface.height {
        for x in 0..surface.width {
            let pixel = surface.pixel(i64::from(x), i64::from(y)).unwrap();
            if pixel[3] == 0xff && x + at.0 < screen.width && y + at.1 < screen.height {
                set(screen, x + at.0, y + at.1, pixel);
            }
        }
    }
}

#[test]
fn a_drawn_surface_is_found_at_its_corner() {
    let window = surface(80, 60, 5);
    let mut screen = noise(99, 300, 200);
    draw(&mut screen, &window, (120, 70));
    let SurfaceFix::Found(at) = locate(&window, &screen) else {
        panic!("not found");
    };
    assert_eq!((at.x, at.y), (120, 70));
    assert!(at.agreement > 0.99);
}

#[test]
fn a_partly_covered_surface_is_still_found_where_it_shows() {
    let window = surface(80, 60, 5);
    let mut screen = noise(99, 300, 200);
    draw(&mut screen, &window, (40, 30));
    let cover = noise(3, 50, 60);
    draw(&mut screen, &cover, (40, 30));
    let SurfaceFix::Found(at) = locate(&window, &screen) else {
        panic!("not found");
    };
    assert_eq!((at.x, at.y), (40, 30));
    let shown = shown_share(&window, &screen, (40, 30), (60, 10, 10, 10)).unwrap();
    let hidden = shown_share(&window, &screen, (40, 30), (10, 10, 10, 10)).unwrap();
    assert!(shown > 0.99, "{shown}");
    assert!(hidden < 0.1, "{hidden}");
}

#[test]
fn a_surface_that_is_not_drawn_is_not_found() {
    let window = surface(80, 60, 5);
    let screen = noise(99, 300, 200);
    assert_eq!(locate(&window, &screen), SurfaceFix::NotFound);
}

#[test]
fn the_same_surface_drawn_twice_is_ambiguous() {
    let window = surface(60, 50, 2);
    let mut screen = noise(99, 300, 200);
    draw(&mut screen, &window, (10, 10));
    draw(&mut screen, &window, (150, 100));
    assert_eq!(locate(&window, &screen), SurfaceFix::Ambiguous);
}

#[test]
fn a_surface_with_too_few_opaque_pixels_proves_nothing() {
    let window = surface(20, 20, 9);
    let mut screen = noise(99, 100, 100);
    draw(&mut screen, &window, (10, 10));
    assert_eq!(locate(&window, &screen), SurfaceFix::NotFound);
}

#[test]
fn the_opaque_bounds_skip_the_shadow_margin() {
    let window = surface(80, 60, 5);
    assert_eq!(opaque_bounds(&window), Some((5, 5, 70, 50)));
    let clear = surface(10, 10, 5);
    assert_eq!(opaque_bounds(&clear), None);
}

fn black_framed(width: u32, height: u32, margin: u32) -> CapturedImage {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let inside = x >= margin && y >= margin && x < width - margin && y < height - margin;
            rgba.extend_from_slice(&if inside {
                [200, 201, 202, 0xff]
            } else {
                [0, 0, 0, 0xff]
            });
        }
    }
    CapturedImage {
        width,
        height,
        rgba,
    }
}

#[test]
fn an_alpha_less_capture_has_its_black_shadow_keyed_out() {
    let keyed = key_black(black_framed(80, 60, 5));
    assert_eq!(opaque_bounds(&keyed), Some((5, 5, 70, 50)));
    assert_eq!(keyed.pixel(0, 0), Some([0, 0, 0, 0]));
    assert_eq!(keyed.pixel(10, 10), Some([200, 201, 202, 0xff]));
}

#[test]
fn a_capture_with_alpha_keeps_its_black_pixels() {
    let mut window = black_framed(80, 60, 5);
    window.rgba[3] = 0;
    let kept = key_black(window.clone());
    assert_eq!(kept.rgba, window.rgba);
    assert_eq!(kept.pixel(1, 0), Some([0, 0, 0, 0xff]));
}

#[test]
fn a_flat_window_a_pixel_off_is_the_same_match_not_a_rival() {
    let scored = vec![
        ((436, 26), 6118, 5575),
        ((435, 27), 6118, 6118),
        ((486, 27), 6118, 4342),
    ];
    let SurfaceFix::Found(at) = verdict(scored) else {
        panic!("one place");
    };
    assert_eq!((at.x, at.y), (435, 27));
}

#[test]
fn a_far_candidate_that_nearly_matches_is_a_rival() {
    let scored = vec![((435, 27), 6118, 6118), ((503, 27), 6118, 5575)];
    assert_eq!(verdict(scored), SurfaceFix::Ambiguous);
}
