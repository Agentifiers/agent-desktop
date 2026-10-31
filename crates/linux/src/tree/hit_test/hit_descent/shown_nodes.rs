//! The bus nodes of a descent on a wlroots compositor. A child that is
//! neither `VISIBLE` nor `SHOWING` draws nothing and takes no pointer event,
//! so it is no sibling a point could reach: Chromium keeps such a panel over
//! its whole window beside the one that holds the page.

use agent_desktop_core::AdapterError;

use super::{BusNodes, Described, Hit, NodeSource};
use crate::system::{ACCESSIBLE_INTERFACE, within};
use crate::tree::AtspiElement;
use crate::tree::properties::node_read::call;
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VISIBLE};

pub(crate) struct ShownNodes<'a> {
    pub(crate) nodes: BusNodes<'a>,
}

/// Whether a node whose state reads `words` can be drawn. A state that could
/// not be read keeps the node, so nothing unread is ruled out.
pub(crate) fn drawn(words: Option<&[u32]>) -> bool {
    words.is_none_or(|words| {
        let bits = StateBits::new(words);
        bits.has(VISIBLE) || bits.has(SHOWING)
    })
}

impl NodeSource for ShownNodes<'_> {
    fn at_point(&mut self, from: &AtspiElement, point: (i32, i32)) -> Result<Hit, AdapterError> {
        self.nodes.at_point(from, point)
    }

    fn verified_parent(
        &mut self,
        node: &AtspiElement,
    ) -> Result<Option<AtspiElement>, AdapterError> {
        self.nodes.verified_parent(node)
    }

    fn claimed_parent(
        &mut self,
        node: &AtspiElement,
    ) -> Result<Option<AtspiElement>, AdapterError> {
        self.nodes.claimed_parent(node)
    }

    fn children(&mut self, node: &AtspiElement) -> Result<Option<Vec<AtspiElement>>, AdapterError> {
        let Some(children) = self.nodes.children(node)? else {
            return Ok(None);
        };
        let connection = self.nodes.connection;
        within(self.nodes.deadline, async {
            let mut shown = Vec::with_capacity(children.len());
            for child in children {
                let words = call(connection, &child, ACCESSIBLE_INTERFACE, "GetState", &())
                    .await
                    .ok()
                    .and_then(|reply| reply.body().deserialize::<Vec<u32>>().ok())
                    .filter(|words| !words.is_empty());
                if drawn(words.as_deref()) {
                    shown.push(child);
                }
            }
            Ok(Some(shown))
        })
    }

    fn describe(&mut self, node: &AtspiElement) -> Result<Described, AdapterError> {
        self.nodes.describe(node)
    }
}

#[cfg(test)]
#[path = "shown_nodes_tests.rs"]
mod tests;
