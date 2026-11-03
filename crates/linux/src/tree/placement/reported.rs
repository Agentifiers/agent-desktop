//! How a toolkit's window coordinates for one node relate to where the node
//! is drawn in its window. On a wlroots compositor a node is placed only by
//! its window rectangle, so a toolkit that reports a node somewhere else
//! than it draws it moves the pointer, the target outline and the dart with
//! it. Two do: GTK 4 counts a stack page's offset twice ([`page_offset`]),
//! and Chromium on a fractionally scaled output reports web content in
//! device pixels while its window frame stays in layout units
//! ([`web_scale`]). The toolkit's own point queries answer in the space it
//! reports in, so a probe is turned back into that space before it is asked.
//!
//! [`page_offset`]: super::page_offset
//! [`web_scale`]: super::web_scale

use agent_desktop_core::Deadline;

use super::page_offset::page_displacement;
use super::web_scale::web_scale;
use crate::system::SharedBus;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};
use crate::tree::{AtspiElement, Extents};

/// A node's reported window rectangle is its drawn rectangle times `scale`,
/// moved by `shift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reported {
    pub(crate) scale: f64,
    pub(crate) shift: (i32, i32),
}

fn whole(value: f64) -> Option<i32> {
    let rounded = value.round();
    (rounded.is_finite() && rounded >= f64::from(i32::MIN) && rounded <= f64::from(i32::MAX))
        .then_some(rounded as i32)
}

impl Reported {
    /// A node reported where it is drawn.
    pub(crate) const DRAWN: Self = Self {
        scale: 1.0,
        shift: (0, 0),
    };

    /// Where a node the toolkit reports at `rect` is drawn, each edge rounded
    /// to the nearest pixel.
    pub(crate) fn drawn(&self, rect: Extents) -> Option<Extents> {
        if *self == Self::DRAWN {
            return Some(rect);
        }
        if !(self.scale.is_finite() && self.scale > 0.0) {
            return None;
        }
        let edge =
            |value: i32, shift: i32| whole((f64::from(value) - f64::from(shift)) / self.scale);
        let right = rect.x.checked_add(rect.width)?;
        let bottom = rect.y.checked_add(rect.height)?;
        let (x, y) = (edge(rect.x, self.shift.0)?, edge(rect.y, self.shift.1)?);
        Some(Extents {
            x,
            y,
            width: edge(right, self.shift.0)?.checked_sub(x)?,
            height: edge(bottom, self.shift.1)?.checked_sub(y)?,
        })
    }

    /// The point the toolkit reports for a drawn window point.
    pub(crate) fn reported_point(&self, point: (i32, i32)) -> Option<(i32, i32)> {
        if *self == Self::DRAWN {
            return Some(point);
        }
        let axis = |value: i32, shift: i32| whole(f64::from(value) * self.scale + f64::from(shift));
        Some((axis(point.0, self.shift.0)?, axis(point.1, self.shift.1)?))
    }
}

/// How `toolkit` reports `element`, a node of the window `frame`: GTK 4 by
/// the offsets of the stack pages above it, Chromium by the scale its web
/// content is reported in, every other toolkit where it draws. `None` when
/// the accessible parents that decide it cannot be read in time.
pub(crate) fn reported_frame(
    bus: &SharedBus,
    element: &AtspiElement,
    frame: &AtspiElement,
    toolkit: &ToolkitId,
    deadline: Deadline,
) -> Option<Reported> {
    match kind(toolkit) {
        ToolkitKind::Gtk4 => page_displacement(bus, element, frame, deadline)
            .map(|shift| Reported { scale: 1.0, shift }),
        ToolkitKind::Chromium => web_scale(bus, element, frame, deadline).map(|scale| Reported {
            scale,
            shift: (0, 0),
        }),
        _ => Some(Reported::DRAWN),
    }
}

#[cfg(test)]
#[path = "reported_tests.rs"]
mod tests;
