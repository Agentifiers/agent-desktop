//! The label written onto the card: one line, left aligned, centred
//! vertically and cut with an ellipsis when it does not fit, as the Windows
//! renderer asks GDI to draw it. The size is the card's font size in pixels,
//! as GDI takes it.

use agent_desktop_core::Rect;

use super::raster::Surface;

const ELLIPSIS: char = '…';

fn advance(font: &fontdue::Font, text: &str, px: f32) -> f64 {
    text.chars()
        .map(|glyph| f64::from(font.metrics(glyph, px).advance_width))
        .sum()
}

/// The longest prefix of `label` that fits `width`, with an ellipsis when cut.
pub(crate) fn fitted(font: &fontdue::Font, label: &str, px: f32, width: f64) -> String {
    if advance(font, label, px) <= width {
        return label.to_owned();
    }
    let mut kept: Vec<char> = label.chars().collect();
    while !kept.is_empty() {
        kept.pop();
        let candidate: String = kept.iter().chain(std::iter::once(&ELLIPSIS)).collect();
        if advance(font, &candidate, px) <= width {
            return candidate;
        }
    }
    String::new()
}

fn blend(surface: &mut Surface, x: i32, y: i32, rgb: [f64; 3], coverage: f64) {
    if coverage <= 0.0 || x < 0 || y < 0 || x >= surface.width || y >= surface.height {
        return;
    }
    let index = (y as usize) * (surface.width as usize) + (x as usize);
    let Some(under) = surface.pixels.get(index).copied() else {
        return;
    };
    let channel = |shift: u32, source: f64| {
        let below = f64::from(((under >> shift) & 0xFF) as u8) / 255.0;
        ((source * coverage + below * (1.0 - coverage)).clamp(0.0, 1.0) * 255.0).round() as u32
    };
    let alpha = channel(24, 1.0);
    let red = channel(16, rgb[0]);
    let green = channel(8, rgb[1]);
    let blue = channel(0, rgb[2]);
    surface.pixels[index] = (alpha << 24) | (red << 16) | (green << 8) | blue;
}

/// Writes `label` into `rect` of `surface` in `rgb`.
pub(crate) fn draw_label(
    surface: &mut Surface,
    rect: &Rect,
    label: &str,
    rgb: [f64; 3],
    (px, font): (f64, &fontdue::Font),
) {
    let size = px.round().max(1.0) as f32;
    let shown = fitted(font, label, size, rect.width);
    let Some(line) = font.horizontal_line_metrics(size) else {
        return;
    };
    let height = f64::from(line.ascent - line.descent);
    let baseline = rect.y + (rect.height - height) / 2.0 + f64::from(line.ascent);
    let mut pen = rect.x;
    for glyph in shown.chars() {
        let (metrics, coverage) = font.rasterize(glyph, size);
        let left = (pen + metrics.xmin as f64).round() as i32;
        let top = (baseline - metrics.height as f64 - metrics.ymin as f64).round() as i32;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let value = coverage[row * metrics.width + column];
                blend(
                    surface,
                    left + column as i32,
                    top + row as i32,
                    rgb,
                    f64::from(value) / 255.0,
                );
            }
        }
        pen += f64::from(metrics.advance_width);
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
