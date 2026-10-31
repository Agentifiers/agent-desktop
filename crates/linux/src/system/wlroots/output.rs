use agent_desktop_core::{DisplayInfo, Rect};

/// One compositor output. `x`, `y`, `width` and `height` are in layout
/// coordinates when `logical` is set (read from `zxdg_output_v1`); otherwise
/// the size is the current mode in pixels and the position is unknown.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Output {
    pub(crate) name: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) scale: f64,
    pub(crate) logical: bool,
}

impl Output {
    fn logical_size(&self) -> (i32, i32) {
        if self.logical || self.scale <= 1.0 {
            return (self.width, self.height);
        }
        (
            (f64::from(self.width) / self.scale).round() as i32,
            (f64::from(self.height) / self.scale).round() as i32,
        )
    }

    /// The output's rectangle in layout coordinates.
    pub(crate) fn rect(&self) -> Rect {
        let (width, height) = self.logical_size();
        Rect {
            x: f64::from(self.x),
            y: f64::from(self.y),
            width: f64::from(width),
            height: f64::from(height),
        }
    }
}

/// The smallest rectangle that holds every output: the frame a virtual
/// pointer's absolute motion is expressed in.
pub(crate) fn layout_bounds(outputs: &[Output]) -> Option<Rect> {
    let rects: Vec<Rect> = outputs
        .iter()
        .map(Output::rect)
        .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
        .collect();
    let left = rects.iter().map(|rect| rect.x).reduce(f64::min)?;
    let top = rects.iter().map(|rect| rect.y).reduce(f64::min)?;
    let right = rects
        .iter()
        .map(|rect| rect.x + rect.width)
        .reduce(f64::max)?;
    let bottom = rects
        .iter()
        .map(|rect| rect.y + rect.height)
        .reduce(f64::max)?;
    Some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// The outputs as displays. wlroots has no primary output, so the output at
/// the layout origin is reported primary, or the first one when none sits
/// there; the primary comes first.
pub(crate) fn to_displays(outputs: &[Output]) -> Vec<DisplayInfo> {
    let primary = outputs
        .iter()
        .position(|output| output.x == 0 && output.y == 0)
        .unwrap_or(0);
    let mut displays: Vec<DisplayInfo> = outputs
        .iter()
        .enumerate()
        .map(|(index, output)| DisplayInfo {
            id: output.name.clone(),
            bounds: output.rect(),
            is_primary: index == primary,
            scale: if output.scale > 0.0 {
                output.scale
            } else {
                1.0
            },
        })
        .collect();
    displays.sort_by(|left, right| {
        right
            .is_primary
            .cmp(&left.is_primary)
            .then_with(|| left.id.cmp(&right.id))
    });
    displays
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
