//! Finds where a toplevel's own pixels are drawn on an output. A wlroots
//! compositor tells no client where a window sits, but it hands out both the
//! window's surface and the output's frame, and on an unscaled output the
//! window's opaque pixels appear unchanged in the frame. Short runs of the
//! surface's pixels are searched for in the frame; every place they occur is
//! scored against a grid of the surface's opaque pixels, and only a clear,
//! unique winner is a position.

use super::captured_image::CapturedImage;

const RUN: usize = 12;
const MIN_DISTINCT: usize = 3;
const BANDS: u32 = 12;
const SLICES: usize = 4;
const ROWS_PER_BAND: u32 = 6;
const GRID: u32 = 6;
const MIN_SAMPLES: usize = 40;
const MIN_AGREEMENT: f64 = 0.3;
const SURE_AGREEMENT: f64 = 0.97;
const RUNNER_UP_SHARE: f64 = 0.8;
const OPAQUE: u8 = 0xff;
/// How far, in pixels on either axis, a candidate may sit from the best one
/// and still be the same match: a flat window shifted by a pixel or two
/// agrees with most of itself, which is no second place.
const SAME_PEAK: i64 = 2;

/// The surface's top-left corner in the output frame's pixels, and the share
/// of its sampled opaque pixels the frame shows there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Located {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) agreement: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SurfaceFix {
    Found(Located),
    /// Two places show the surface about equally well.
    Ambiguous,
    NotFound,
}

fn rgb(pixel: [u8; 4]) -> [u8; 3] {
    [pixel[0], pixel[1], pixel[2]]
}

fn opaque(image: &CapturedImage, x: u32, y: u32) -> Option<[u8; 3]> {
    let pixel = image.pixel(i64::from(x), i64::from(y))?;
    (pixel[3] == OPAQUE).then(|| rgb(pixel))
}

fn samples(surface: &CapturedImage) -> Vec<(u32, u32, [u8; 3])> {
    let mut found = Vec::new();
    for y in (0..surface.height).step_by(GRID as usize) {
        for x in (0..surface.width).step_by(GRID as usize) {
            if let Some(color) = opaque(surface, x, y) {
                found.push((x, y, color));
            }
        }
    }
    found
}

/// A run of `RUN` opaque pixels starting in `columns` with some texture, so
/// it does not match every flat area of the frame.
fn run_at(
    surface: &CapturedImage,
    y: u32,
    columns: std::ops::Range<usize>,
) -> Option<(u32, Vec<[u8; 3]>)> {
    let last = (surface.width as usize).saturating_sub(RUN);
    let mut best: Option<(usize, u32, Vec<[u8; 3]>)> = None;
    for start in (columns.start..columns.end.min(last)).step_by(RUN / 2) {
        let colors: Option<Vec<[u8; 3]>> = (start..start + RUN)
            .map(|x| opaque(surface, x as u32, y))
            .collect();
        let Some(colors) = colors else { continue };
        let mut distinct = colors.clone();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() >= MIN_DISTINCT && best.as_ref().is_none_or(|(n, ..)| distinct.len() > *n)
        {
            best = Some((distinct.len(), start as u32, colors));
        }
    }
    best.map(|(_, start, colors)| (start, colors))
}

/// Runs spread over the surface: one per horizontal slice of each band of
/// rows, so a window partly covered on any side still offers one to find.
fn anchors(surface: &CapturedImage) -> Vec<(u32, u32, Vec<[u8; 3]>)> {
    let band = (surface.height / BANDS).max(1);
    let slice = (surface.width as usize / SLICES).max(1);
    let mut found = Vec::new();
    for index in 0..BANDS {
        let top = index * band;
        for part in 0..SLICES {
            let columns = part * slice..(part + 1) * slice;
            let run = (top..(top + band).min(surface.height))
                .step_by((band / ROWS_PER_BAND).max(1) as usize)
                .find_map(|y| {
                    run_at(surface, y, columns.clone()).map(|(x, colors)| (x, y, colors))
                });
            found.extend(run);
        }
    }
    found
}

fn occurrences(screen: &CapturedImage, run: &[[u8; 3]]) -> Vec<(i64, i64)> {
    let mut hits = Vec::new();
    let width = screen.width as usize;
    let Some(first) = run.first() else {
        return hits;
    };
    for (y, row) in screen.rgba.chunks_exact(width * 4).enumerate() {
        for x in 0..(width + 1).saturating_sub(run.len()) {
            let at = x * 4;
            if row[at..at + 3] != first[..] {
                continue;
            }
            let matched = run
                .iter()
                .enumerate()
                .skip(1)
                .all(|(offset, color)| row[at + offset * 4..at + offset * 4 + 3] == color[..]);
            if matched {
                hits.push((x as i64, y as i64));
            }
        }
    }
    hits
}

fn score(screen: &CapturedImage, grid: &[(u32, u32, [u8; 3])], at: (i64, i64)) -> (usize, usize) {
    grid.iter().fold((0, 0), |(seen, agreed), (x, y, color)| {
        match screen.pixel(at.0 + i64::from(*x), at.1 + i64::from(*y)) {
            Some(pixel) => (seen + 1, agreed + usize::from(rgb(pixel) == *color)),
            None => (seen, agreed),
        }
    })
}

/// Where `surface` is drawn in `screen`.
pub(crate) fn locate(surface: &CapturedImage, screen: &CapturedImage) -> SurfaceFix {
    let grid = samples(surface);
    if grid.len() < MIN_SAMPLES {
        return SurfaceFix::NotFound;
    }
    let mut scored: Vec<((i64, i64), usize, usize)> = Vec::new();
    for (ax, ay, run) in anchors(surface) {
        for (sx, sy) in occurrences(screen, &run) {
            let at = (sx - i64::from(ax), sy - i64::from(ay));
            if scored.iter().any(|(known, ..)| *known == at) {
                continue;
            }
            let (seen, agreed) = score(screen, &grid, at);
            scored.push((at, seen, agreed));
        }
        let sure = scored.iter().any(|(_, seen, agreed)| {
            *seen * 2 >= grid.len() && *agreed as f64 >= SURE_AGREEMENT * grid.len() as f64
        });
        if sure {
            break;
        }
    }
    verdict(scored)
}

/// The fix that candidate places `(place, seen, agreed)` give: the best
/// one, unless it agrees too little or a candidate away from it nearly
/// matches it.
fn verdict(mut scored: Vec<((i64, i64), usize, usize)>) -> SurfaceFix {
    scored.sort_by(|left, right| right.2.cmp(&left.2));
    let Some(&(at, seen, agreed)) = scored.first() else {
        return SurfaceFix::NotFound;
    };
    let agreement = agreed as f64 / seen.max(1) as f64;
    if agreement < MIN_AGREEMENT || agreed < MIN_SAMPLES / 2 {
        return SurfaceFix::NotFound;
    }
    let rival = scored
        .iter()
        .skip(1)
        .filter(|(other, ..)| (other.0 - at.0).abs().max((other.1 - at.1).abs()) > SAME_PEAK)
        .map(|(_, _, other)| *other)
        .max();
    if rival.is_some_and(|other| other as f64 >= RUNNER_UP_SHARE * agreed as f64) {
        return SurfaceFix::Ambiguous;
    }
    match (i32::try_from(at.0), i32::try_from(at.1)) {
        (Ok(x), Ok(y)) => SurfaceFix::Found(Located { x, y, agreement }),
        _ => SurfaceFix::NotFound,
    }
}

/// The share of `surface`'s opaque pixels inside `rect` (surface pixels, as
/// `(x, y, width, height)`) that `screen` shows unchanged with the surface at
/// `at`, or `None` when the rectangle holds no opaque pixel on the screen.
pub(crate) fn shown_share(
    surface: &CapturedImage,
    screen: &CapturedImage,
    at: (i64, i64),
    rect: (i64, i64, i64, i64),
) -> Option<f64> {
    let (left, top, width, height) = rect;
    let (mut seen, mut agreed) = (0_usize, 0_usize);
    for y in top.max(0)..(top + height).min(i64::from(surface.height)) {
        for x in left.max(0)..(left + width).min(i64::from(surface.width)) {
            let Some(own) = surface.pixel(x, y).filter(|pixel| pixel[3] == OPAQUE) else {
                continue;
            };
            let Some(drawn) = screen.pixel(at.0 + x, at.1 + y) else {
                continue;
            };
            seen += 1;
            agreed += usize::from(rgb(drawn) == rgb(own));
        }
    }
    (seen > 0).then(|| agreed as f64 / seen as f64)
}

/// The smallest rectangle `(x, y, width, height)` holding every opaque pixel
/// of `surface`: the window without client-side shadow margins.
pub(crate) fn opaque_bounds(surface: &CapturedImage) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for y in 0..surface.height {
        for x in 0..surface.width {
            if opaque(surface, x, y).is_none() {
                continue;
            }
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((left, top, right, bottom)) => {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }
            });
        }
    }
    bounds.map(|(left, top, right, bottom)| (left, top, right - left + 1, bottom - top + 1))
}

/// A toplevel captured in a format without alpha, as a compositor with a
/// software renderer hands it out, has its transparent client-side shadow
/// drawn black: the shadow is premultiplied, so dropping its alpha leaves no
/// color. When no pixel of `surface` is transparent, its pure black pixels
/// are taken as transparent, so the window's own pixels are what remains. A
/// window drawn black at its edges then no longer matches its frame and
/// proves nothing, rather than proving a wrong place.
pub(crate) fn key_black(mut surface: CapturedImage) -> CapturedImage {
    let carries_alpha = surface.rgba.chunks_exact(4).any(|pixel| pixel[3] != OPAQUE);
    if carries_alpha {
        return surface;
    }
    for pixel in surface.rgba.chunks_exact_mut(4) {
        if pixel[..3] == [0, 0, 0] {
            pixel[3] = 0;
        }
    }
    surface
}

#[cfg(test)]
#[path = "surface_locate_tests.rs"]
mod tests;
