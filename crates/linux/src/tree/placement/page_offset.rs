//! GTK 4 counts a stack page's offset twice. A stack page (`GtkStackPage`,
//! and libadwaita's view stack and tab pages) is an accessible that is not a
//! widget: it answers its bounds with its one child's bounds in the stack,
//! and the child answers the same bounds again. The toolkit's window
//! coordinates add up the bounds of every accessible parent, so every node
//! inside a page is reported the page's offset further right and down than
//! it is drawn — twelve pixels each way for every Calculator keypad button.

use agent_desktop_core::{AdapterError, Deadline};
use futures_util::future::join;
use zbus::Connection;

use crate::system::{SharedBus, within};
use crate::tree::hit_test::hit_descent::{DEPTH_CAP, parent_of, same};
use crate::tree::surfaces::window_extents::read_extents_async;
use crate::tree::{AtspiElement, Extents, list_children};

const PARENT_COORDINATES: u32 = 2;

/// One accessible above a node, below its window frame: its rectangle in its
/// parent's coordinates and how many children it lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) extents: Extents,
    pub(crate) children: usize,
}

/// How far the toolkit's window coordinates of a node whose rectangle in its
/// parent is `node` lie right of and below where it is drawn, given its
/// `ancestors` from its parent up to the frame's child. A page is the
/// ancestor that lists one child and reads that child's rectangle as its own;
/// any other container holds its children inside itself, so a child cannot
/// sit at the container's own offset at the container's full size unless
/// the container is a page. A page at its stack's corner adds nothing.
pub(crate) fn displacement(node: Extents, ancestors: &[Step]) -> Option<(i32, i32)> {
    let mut by = (0i32, 0i32);
    let mut child = node;
    for step in ancestors {
        if step.children == 1 && step.extents == child {
            by = (
                by.0.checked_add(step.extents.x)?,
                by.1.checked_add(step.extents.y)?,
            );
        }
        child = step.extents;
    }
    Some(by)
}

async fn read_displacement(
    connection: &Connection,
    element: &AtspiElement,
    frame: &AtspiElement,
) -> Result<Option<(i32, i32)>, AdapterError> {
    if same(element, frame) {
        return Ok(Some((0, 0)));
    }
    let Some(node) = read_extents_async(connection, element, PARENT_COORDINATES).await? else {
        return Ok(None);
    };
    let mut ancestors = Vec::new();
    let mut at = element.clone();
    for _ in 0..DEPTH_CAP {
        let Some(parent) = parent_of(connection, &at).await else {
            return Ok(None);
        };
        if same(&parent, frame) {
            return Ok(displacement(node, &ancestors));
        }
        let (extents, children) = join(
            read_extents_async(connection, &parent, PARENT_COORDINATES),
            list_children(connection, &parent),
        )
        .await;
        let (Some(extents), Ok(children)) = (extents?, children) else {
            return Ok(None);
        };
        ancestors.push(Step {
            extents,
            children: children.len(),
        });
        at = parent;
    }
    Ok(None)
}

/// How far GTK 4 reports `element`, a node of the window `frame`, right of
/// and below where it is drawn: the offsets of the stack pages above it.
/// `None` when its accessible parents up to the frame cannot be read in time.
pub(crate) fn page_displacement(
    bus: &SharedBus,
    element: &AtspiElement,
    frame: &AtspiElement,
    deadline: Deadline,
) -> Option<(i32, i32)> {
    let connection = bus.get(deadline).ok()?;
    within(
        deadline,
        read_displacement(connection.inner(), element, frame),
    )
    .ok()
    .flatten()
}

#[cfg(test)]
#[path = "page_offset_tests.rs"]
mod tests;
