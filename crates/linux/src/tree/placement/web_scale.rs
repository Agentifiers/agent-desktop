//! Chromium on a fractionally scaled Wayland output reports its web content
//! in device pixels: a page's nodes read the output scale times their place
//! in the window, while the window frame and the browser's own controls read
//! layout units. The tab's document fills a container of the browser window,
//! so the document reads that container's rectangle times the scale, and the
//! ratio of the two is the scale its content is reported in. Wayland hands a
//! fractional scale to clients in 120ths, so the ratio is taken to the
//! nearest 120th.

use agent_desktop_core::{AdapterError, Deadline};
use futures_util::future::join;
use zbus::Connection;

use crate::system::{ACCESSIBLE_INTERFACE, SharedBus, within};
use crate::tree::hit_test::hit_descent::{DEPTH_CAP, parent_of, same};
use crate::tree::properties::node_read::call;
use crate::tree::surfaces::window_extents::read_window_extents_async;
use crate::tree::vocabulary::role_numbers::is_web_document;
use crate::tree::{AtspiElement, Extents};

const STEPS_PER_UNIT: f64 = 120.0;
const AXIS_AGREEMENT: f64 = 0.01;
const ORIGIN_SLACK: f64 = 2.0;

/// The scale a document reported at `document` is in, when its container
/// reads `container` in layout units: both axes must give the same ratio and
/// the document's corner must sit at the container's corner times it.
pub(crate) fn scale_of(document: Extents, container: Extents) -> Option<f64> {
    if document.width <= 0 || document.height <= 0 || container.width <= 0 || container.height <= 0
    {
        return None;
    }
    let across = f64::from(document.width) / f64::from(container.width);
    let down = f64::from(document.height) / f64::from(container.height);
    if (across - down).abs() > AXIS_AGREEMENT {
        return None;
    }
    let scale = ((across + down) / 2.0 * STEPS_PER_UNIT).round() / STEPS_PER_UNIT;
    let placed = |reported: i32, layout: i32| {
        (f64::from(reported) - f64::from(layout) * scale).abs() <= ORIGIN_SLACK
    };
    (scale > 0.0 && placed(document.x, container.x) && placed(document.y, container.y))
        .then_some(scale)
}

async fn role_of(connection: &Connection, element: &AtspiElement) -> Option<u32> {
    call(connection, element, ACCESSIBLE_INTERFACE, "GetRole", &())
        .await
        .ok()?
        .body()
        .deserialize::<u32>()
        .ok()
}

/// The outermost web document at or above `element` below `frame`, and the
/// accessible that holds it; `Some(None)` for a node outside web content.
async fn outermost_document(
    connection: &Connection,
    element: &AtspiElement,
    frame: &AtspiElement,
) -> Option<Option<(AtspiElement, AtspiElement)>> {
    let mut found = None;
    let mut at = element.clone();
    for _ in 0..DEPTH_CAP {
        if same(&at, frame) {
            return Some(found);
        }
        let (role, parent) = join(role_of(connection, &at), parent_of(connection, &at)).await;
        let parent = parent?;
        if is_web_document(Some(role?)) {
            found = Some((at, parent.clone()));
        }
        at = parent;
    }
    None
}

async fn read_scale(
    connection: &Connection,
    element: &AtspiElement,
    frame: &AtspiElement,
) -> Result<Option<f64>, AdapterError> {
    let Some(found) = outermost_document(connection, element, frame).await else {
        return Ok(None);
    };
    let Some((document, container)) = found else {
        return Ok(Some(1.0));
    };
    let (document, container) = join(
        read_window_extents_async(connection, &document),
        read_window_extents_async(connection, &container),
    )
    .await;
    Ok(document?
        .zip(container?)
        .and_then(|(document, container)| scale_of(document, container)))
}

/// The scale Chromium reports `element`, a node of the window `frame`, in:
/// its web content's scale, or one for a node of the browser's own
/// controls. `None` when the accessible parents that decide it cannot be
/// read in time or the document does not fill its container at one scale.
pub(crate) fn web_scale(
    bus: &SharedBus,
    element: &AtspiElement,
    frame: &AtspiElement,
    deadline: Deadline,
) -> Option<f64> {
    let connection = bus.get(deadline).ok()?;
    within(deadline, read_scale(connection.inner(), element, frame))
        .ok()
        .flatten()
}

#[cfg(test)]
#[path = "web_scale_tests.rs"]
mod tests;
