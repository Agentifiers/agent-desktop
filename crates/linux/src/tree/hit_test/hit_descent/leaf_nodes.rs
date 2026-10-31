//! The nodes of a descent on a wlroots compositor, with the parent edges of
//! a leaf's hidden content. Chromium exposes a button as a leaf that lists
//! no children, yet once its renderer has answered a point query at the
//! button it names the button's text node there, and that node names the
//! button its `Parent`. The edge no child list confirms is taken only when
//! the parent, as the wrapped source lists it, has no child, so no listed
//! sibling can be the node, and the node lies inside the parent, so the
//! point inside the node is a point inside the parent. Any other
//! unconfirmed edge stays unverified.

use agent_desktop_core::AdapterError;

use super::{Described, Hit, NodeSource};
use crate::tree::AtspiElement;
use crate::tree::hit_test::hit_guard::encloses;

pub(crate) struct LeafNodes<N> {
    pub(crate) nodes: N,
}

impl<N: NodeSource> LeafNodes<N> {
    fn leaf_owner(&mut self, node: &AtspiElement) -> Result<Option<AtspiElement>, AdapterError> {
        let Some(parent) = self.nodes.claimed_parent(node)? else {
            return Ok(None);
        };
        if parent.bus_name() != node.bus_name()
            || !self
                .nodes
                .children(&parent)?
                .is_some_and(|children| children.is_empty())
        {
            return Ok(None);
        }
        let inner = self.nodes.describe(node)?.window_extents;
        let outer = self.nodes.describe(&parent)?.window_extents;
        Ok(inner
            .zip(outer)
            .is_some_and(|(inner, outer)| encloses(outer, inner))
            .then_some(parent))
    }
}

impl<N: NodeSource> NodeSource for LeafNodes<N> {
    fn at_point(&mut self, from: &AtspiElement, point: (i32, i32)) -> Result<Hit, AdapterError> {
        self.nodes.at_point(from, point)
    }

    fn verified_parent(
        &mut self,
        node: &AtspiElement,
    ) -> Result<Option<AtspiElement>, AdapterError> {
        match self.nodes.verified_parent(node)? {
            Some(parent) => Ok(Some(parent)),
            None => self.leaf_owner(node),
        }
    }

    fn claimed_parent(
        &mut self,
        node: &AtspiElement,
    ) -> Result<Option<AtspiElement>, AdapterError> {
        self.nodes.claimed_parent(node)
    }

    fn children(&mut self, node: &AtspiElement) -> Result<Option<Vec<AtspiElement>>, AdapterError> {
        self.nodes.children(node)
    }

    fn describe(&mut self, node: &AtspiElement) -> Result<Described, AdapterError> {
        self.nodes.describe(node)
    }
}

#[cfg(test)]
#[path = "leaf_nodes_tests.rs"]
mod tests;
