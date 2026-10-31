//! The X11 overlay's arrow drawn into pixels: the same seven-point outline
//! scaled by the style's size, filled with the fill colour and traced with a
//! one-pixel rim, transparent everywhere else. The tip sits at pixel (0, 0),
//! which is the point the marker is placed on.

use agent_desktop_core::CursorOverlayStyle;

use super::raster::Raster;

const OUTLINE: [(f64, f64); 7] = [
    (0., 0.),
    (0., 24.),
    (6., 18.),
    (11., 29.),
    (16., 26.),
    (11., 16.),
    (21., 16.),
];
const BASE_WIDTH: f64 = 24.0;
const BASE_HEIGHT: f64 = 32.0;

fn opaque(rgb: [f64; 3]) -> u32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    0xff00_0000 | channel(rgb[0]) << 16 | channel(rgb[1]) << 8 | channel(rgb[2])
}

fn vertices(scale: f64) -> Vec<(i32, i32)> {
    OUTLINE
        .iter()
        .map(|(x, y)| ((x * scale) as i32, (y * scale) as i32))
        .collect()
}

fn inside(polygon: &[(i32, i32)], x: f64, y: f64) -> bool {
    let mut crossings = false;
    for (index, &(ax, ay)) in polygon.iter().enumerate() {
        let (bx, by) = polygon[(index + 1) % polygon.len()];
        let (ax, ay, bx, by) = (f64::from(ax), f64::from(ay), f64::from(bx), f64::from(by));
        if (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay) {
            crossings = !crossings;
        }
    }
    crossings
}

fn trace(raster: &mut Raster, from: (i32, i32), to: (i32, i32), pixel: u32) {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (step_x, step_y) = ((to.0 - x).signum(), (to.1 - y).signum());
    let mut error = dx + dy;
    loop {
        raster.set(x, y, pixel);
        if (x, y) == to {
            return;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
    }
}

/// The arrow for `style`, sized `ceil(24 * size)` by `ceil(32 * size)`.
pub(crate) fn arrow(style: &CursorOverlayStyle) -> Raster {
    let scale = style.size();
    let width = (BASE_WIDTH * scale).ceil().max(1.0) as u32;
    let height = (BASE_HEIGHT * scale).ceil().max(1.0) as u32;
    let mut raster = Raster::transparent(width, height);
    let polygon = vertices(scale);
    let fill = opaque(style.fill_rgb());
    for y in 0..height {
        for x in 0..width {
            if inside(&polygon, f64::from(x) + 0.5, f64::from(y) + 0.5) {
                raster.set(x as i32, y as i32, fill);
            }
        }
    }
    let rim = opaque(style.rim_rgb());
    for (index, &from) in polygon.iter().enumerate() {
        trace(&mut raster, from, polygon[(index + 1) % polygon.len()], rim);
    }
    raster
}

#[cfg(test)]
#[path = "arrow_tests.rs"]
mod tests;
