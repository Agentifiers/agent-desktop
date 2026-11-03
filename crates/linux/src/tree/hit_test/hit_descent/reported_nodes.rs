//! The nodes of a descent on a wlroots compositor, read in the space the
//! target is drawn in. A toolkit that reports its target somewhere else than
//! it draws it answers point queries in the space it reports in, so every
//! probe is turned into that space before it is asked, and every rectangle
//! a node reports is turned back to where it is drawn before a probe is
//! compared with it. GTK 4 reports every node of the window in one space.
//! Chromium on a fractionally scaled output answers point queries in device
//! pixels and reports its web content in them, while its own controls keep
//! reporting layout units: only a web document and the nodes below it are
//! turned back, so a page element near the far edge is compared with the
//! browser's controls that hold it in one unit.

use std::collections::HashMap;

use agent_desktop_core::AdapterError;

use super::{DEPTH_CAP, Described, Hit, NodeSource};
use crate::tree::AtspiElement;
use crate::tree::placement::reported::Reported;
use crate::tree::vocabulary::role_numbers::names_web_document;

/// Which nodes report in the reported space: every node of the window, or
/// only web content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReportedScope {
    Window,
    WebContent,
}

pub(crate) struct ReportedNodes<N> {
    nodes: N,
    reported: Reported,
    scope: ReportedScope,
    web: HashMap<String, bool>,
    listed_by: HashMap<String, bool>,
}

impl<N: NodeSource> ReportedNodes<N> {
    /// `nodes` read for a target `reported` as it is in the window `frame`,
    /// which is no web content.
    pub(crate) fn new(
        nodes: N,
        reported: Reported,
        scope: ReportedScope,
        frame: &AtspiElement,
    ) -> Self {
        Self {
            nodes,
            reported,
            scope,
            web: HashMap::from([(frame.object_identity(), false)]),
            listed_by: HashMap::new(),
        }
    }

    fn known(&self, node: &AtspiElement, role: Option<&str>) -> Option<bool> {
        let identity = node.object_identity();
        self.web.get(&identity).copied().or_else(|| {
            if names_web_document(role) {
                Some(true)
            } else {
                self.listed_by.get(&identity).copied()
            }
        })
    }

    /// Whether `node`, whose role reads `role`, is web content: a web
    /// document, or a node below one. The parent that listed it decides;
    /// otherwise its claimed parents are read up to a node that does.
    /// `None` when no read decides it.
    fn web_content(
        &mut self,
        node: &AtspiElement,
        role: Option<&str>,
    ) -> Result<Option<bool>, AdapterError> {
        let mut unknown = Vec::new();
        let mut at = node.clone();
        let mut answer = self.known(node, role);
        while answer.is_none() && unknown.len() < DEPTH_CAP {
            unknown.push(at.object_identity());
            let Some(parent) = self.nodes.claimed_parent(&at)? else {
                break;
            };
            answer = match self.web.get(&parent.object_identity()) {
                Some(&web) => Some(web),
                None => {
                    let role = self.nodes.describe(&parent)?.role;
                    self.known(&parent, role.as_deref())
                }
            };
            at = parent;
        }
        if let Some(web) = answer {
            for identity in unknown {
                self.web.insert(identity, web);
            }
            self.web.insert(node.object_identity(), web);
        }
        Ok(answer)
    }
}

impl<N: NodeSource> NodeSource for ReportedNodes<N> {
    fn at_point(&mut self, from: &AtspiElement, point: (i32, i32)) -> Result<Hit, AdapterError> {
        match self.reported.reported_point(point) {
            Some(point) => self.nodes.at_point(from, point),
            None => Ok(Hit::Failed),
        }
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
        let children = self.nodes.children(node)?;
        if let (Some(children), Some(&web)) =
            (children.as_ref(), self.web.get(&node.object_identity()))
        {
            for child in children {
                self.listed_by.insert(child.object_identity(), web);
            }
        }
        Ok(children)
    }

    fn describe(&mut self, node: &AtspiElement) -> Result<Described, AdapterError> {
        let mut described = self.nodes.describe(node)?;
        if self.reported == Reported::DRAWN {
            return Ok(described);
        }
        let turned = match self.scope {
            ReportedScope::Window => Some(true),
            ReportedScope::WebContent => self.web_content(node, described.role.as_deref())?,
        };
        described.window_extents = match turned {
            Some(true) => described
                .window_extents
                .and_then(|rect| self.reported.drawn(rect)),
            Some(false) => described.window_extents,
            None => None,
        };
        Ok(described)
    }
}

#[cfg(test)]
#[path = "reported_nodes_tests.rs"]
mod tests;
