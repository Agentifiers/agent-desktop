//! Rounded rectangles - the label card and the highlight around a target -
//! composited per pixel so their corners keep their coverage. Carried
//! unchanged from the Windows renderer; the text is drawn on top afterwards.

use agent_desktop_core::Rect;

use super::geometry;
use super::raster::{self, Paint, Surface, SurfaceMapping};

/// The outline around the element being acted on, at the opacity its own
/// curve reports.
pub(crate) fn draw_highlight(
    surface: &mut Surface,
    mapping: &SurfaceMapping,
    target: &Rect,
    opacity: f64,
    accent: [f64; 3],
) {
    if opacity <= 0.0 {
        return;
    }
    let scale = mapping.scale;
    let outer = mapping.to_local(&geometry::highlight_rect(target, scale));
    let radius = geometry::HIGHLIGHT_CORNER_RADIUS * scale;
    let border = geometry::HIGHLIGHT_BORDER_WIDTH * scale;
    let inner = Rect {
        x: outer.x + border,
        y: outer.y + border,
        width: (outer.width - border * 2.0).max(0.0),
        height: (outer.height - border * 2.0).max(0.0),
    };
    raster::fill_region_outside(
        surface,
        &outer,
        &hollow_interior(&outer, border, radius),
        &Paint {
            rgb: accent,
            alpha: opacity,
        },
        |x, y| {
            in_rounded_rect(&outer, radius, x, y)
                && !in_rounded_rect(&inner, (radius - border).max(0.0), x, y)
        },
    );
}

/// The part of the outline's bounding rectangle the outline cannot reach.
///
/// The ring lies between `outer` and `inner`, so everything inside `inner` is
/// unpaintable, and the useful question is how much of `inner` a rectangle
/// can claim. The answer is not square: away from the corners the ring is
/// only `border` wide, and it bends inward by the corner radius solely at the
/// four corners - which sit entirely within the full-width bands this leaves
/// above and below. So the ends are held back by `border + radius` and the
/// sides by nothing but the border, and outlining a wide element walks two
/// thin columns per row instead of two thick ones.
///
/// Both insets carry a further `border` of slack over the boundary they have
/// to clear, so this is a fact about the shape rather than a claim about
/// floating point. Where the element is too small for `inner` to have a
/// corner radius that large, one extent or the other comes back negative,
/// which encloses nothing - so a degenerate target paints through the same
/// walk as every other.
fn hollow_interior(outer: &Rect, border: f64, radius: f64) -> Rect {
    let sides = border * 2.0;
    let ends = border + radius;
    Rect {
        x: outer.x + sides,
        y: outer.y + ends,
        width: outer.width - sides * 2.0,
        height: outer.height - ends * 2.0,
    }
}

/// The label bubble's body: an opaque rounded rectangle with a rim, drawn
/// per-pixel so its corners keep their coverage. The text is drawn on top.
pub(crate) fn draw_bubble(surface: &mut Surface, rect: &Rect, fill: [f64; 3], rim: [f64; 3]) {
    let radius = geometry::BUBBLE_CORNER_RADIUS;
    let border = geometry::BUBBLE_BORDER_WIDTH;
    let inner = Rect {
        x: rect.x + border,
        y: rect.y + border,
        width: (rect.width - border * 2.0).max(0.0),
        height: (rect.height - border * 2.0).max(0.0),
    };
    let outer = *rect;
    raster::fill_region(
        surface,
        &outer,
        &Paint {
            rgb: rim,
            alpha: 1.0,
        },
        |x, y| in_rounded_rect(&outer, radius, x, y),
    );
    raster::fill_region(
        surface,
        &inner,
        &Paint {
            rgb: fill,
            alpha: 1.0,
        },
        |x, y| in_rounded_rect(&inner, (radius - border).max(0.0), x, y),
    );
}

pub(super) fn in_rounded_rect(rect: &Rect, radius: f64, x: f64, y: f64) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let left = rect.x + radius;
    let right = rect.x + rect.width - radius;
    let top = rect.y + radius;
    let bottom = rect.y + rect.height - radius;
    if x < rect.x || y < rect.y || x > rect.x + rect.width || y > rect.y + rect.height {
        return false;
    }
    let nearest_x = x.clamp(left, right);
    let nearest_y = y.clamp(top, bottom);
    (x - nearest_x).hypot(y - nearest_y) <= radius
}

#[cfg(test)]
#[path = "rounded_tests.rs"]
mod tests;
