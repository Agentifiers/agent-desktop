//! Dimming an already-composed region of the surface: the label card as it
//! eases in, and the whole overlay as it rests. Carried unchanged from the
//! Windows renderer, which also fades in the pixels rather than through the
//! compositor; text is drawn at full strength first and then dimmed with the
//! card beneath it.

use super::raster::Surface;
use agent_desktop_core::Rect;

/// Multiplies every channel of every pixel in `rect` by `factor`.
///
/// The surface is premultiplied, so alpha and colour scale together and a
/// uniform multiply is the correct dim - scaling alpha alone would leave the
/// colour too strong and the region would darken as it faded rather than
/// receding.
pub(crate) fn dim_region(surface: &mut Surface, rect: &Rect, factor: f64) {
    if factor >= 1.0 {
        return;
    }
    let factor = factor.clamp(0.0, 1.0);
    let left = rect.x.floor().max(0.0) as i32;
    let top = rect.y.floor().max(0.0) as i32;
    let right = (rect.x + rect.width).ceil().min(f64::from(surface.width)) as i32;
    let bottom = (rect.y + rect.height).ceil().min(f64::from(surface.height)) as i32;

    for y in top..bottom {
        for x in left..right {
            let index = (y as usize) * (surface.width as usize) + (x as usize);
            let Some(pixel) = surface.pixels.get(index).copied() else {
                continue;
            };
            surface.pixels[index] = dim_pixel(pixel, factor);
        }
    }
}

/// The whole surface, for the fade the overlay plays when it rests.
pub(crate) fn dim_surface(surface: &mut Surface, factor: f64) {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(surface.width),
        height: f64::from(surface.height),
    };
    dim_region(surface, &bounds, factor);
}

fn dim_pixel(pixel: u32, factor: f64) -> u32 {
    let scale = |shift: u32| {
        let channel = f64::from(((pixel >> shift) & 0xFF) as u8);
        ((channel * factor).round().clamp(0.0, 255.0) as u32) << shift
    };
    scale(24) | scale(16) | scale(8) | scale(0)
}

#[cfg(test)]
#[path = "fade_tests.rs"]
mod tests;
