//! Which part of a captured toplevel is the window. A toplevel capture holds
//! the toplevel's whole surface, and a client that draws its own decorations
//! draws a black shadow around the window. Where the capture keeps alpha the
//! shadow is translucent; where it does not, the compositor reads the
//! translucent black as plain black. The content box is the box that holds
//! every opaque pixel that is not plain black, so it leaves the shadow out.
//! It is taken only when its size is the window's reported size.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, Rect};

use super::window_unit::{CLIENT_TOLERANCE, agrees};
use crate::system::wlroots::captured_image::CapturedImage;
use crate::tree::Extents;

const OPAQUE: u8 = u8::MAX;

fn content(pixel: &[u8]) -> bool {
    pixel[3] == OPAQUE && pixel[..3] != [0, 0, 0]
}

/// The smallest box that holds every opaque pixel that is not plain black,
/// `None` when there is no such pixel.
pub(crate) fn content_box(image: &CapturedImage) -> Option<Extents> {
    let row_bytes = usize::try_from(image.width).ok()? * 4;
    if row_bytes == 0 {
        return None;
    }
    let mut found: Option<(usize, usize, usize, usize)> = None;
    for (y, row) in image.rgba.chunks_exact(row_bytes).enumerate() {
        let mut drawn = row
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, pixel)| content(pixel))
            .map(|(x, _)| x);
        let Some(first) = drawn.next() else {
            continue;
        };
        let last = drawn.next_back().unwrap_or(first);
        found = Some(match found {
            None => (first, y, last, y),
            Some((left, top, right, _)) => (left.min(first), top, right.max(last), y),
        });
    }
    let (left, top, right, bottom) = found?;
    Some(Extents {
        x: i32::try_from(left).ok()?,
        y: i32::try_from(top).ok()?,
        width: i32::try_from(right - left + 1).ok()?,
        height: i32::try_from(bottom - top + 1).ok()?,
    })
}

fn whole(image: &CapturedImage) -> Extents {
    Extents {
        x: 0,
        y: 0,
        width: i32::try_from(image.width).unwrap_or(i32::MAX),
        height: i32::try_from(image.height).unwrap_or(i32::MAX),
    }
}

fn unproven() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "No part of the window's surface matches the size the window reports",
    )
    .with_suggestion("Capture the window with 'screenshot --window-id' instead")
    .with_disposition(DeliverySemantics::not_delivered())
}

/// The part of `image` that is the window: the whole surface or its content
/// box, whichever agrees with `size` (points, at `scale` pixels per point).
/// When neither agrees, `strict` refuses and otherwise the whole surface is
/// returned, shadow margins and all.
pub(crate) fn window_area(
    image: &CapturedImage,
    size: Option<Rect>,
    scale: f64,
    strict: bool,
) -> Result<Extents, AdapterError> {
    let surface = whole(image);
    if let Some(size) = size {
        let pixels = Rect {
            width: size.width * scale,
            height: size.height * scale,
            ..size
        };
        let fits = |area: &Extents| agrees(*area, pixels, CLIENT_TOLERANCE);
        let mut candidates = std::iter::once(surface).chain(content_box(image));
        if let Some(area) = candidates.find(fits) {
            return Ok(area);
        }
    }
    if strict {
        return Err(unproven());
    }
    Ok(surface)
}

/// The pixels of `area` without their alpha, three bytes per pixel.
pub(crate) fn rgb_in(image: &CapturedImage, area: Extents) -> Vec<u8> {
    let stride = image.width as usize * 4;
    let (x, y) = (area.x.max(0) as usize, area.y.max(0) as usize);
    let (width, height) = (area.width.max(0) as usize, area.height.max(0) as usize);
    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in image.rgba.chunks_exact(stride.max(1)).skip(y).take(height) {
        let start = (x * 4).min(row.len());
        let end = ((x + width) * 4).min(row.len());
        rgb.extend(
            row[start..end]
                .chunks_exact(4)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]),
        );
    }
    rgb
}

#[cfg(test)]
#[path = "wlroots_unit_tests.rs"]
mod tests;
