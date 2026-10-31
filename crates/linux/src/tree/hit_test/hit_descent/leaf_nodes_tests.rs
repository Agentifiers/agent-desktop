use agent_desktop_core::Deadline;

use super::*;
use crate::tree::Extents;
use crate::tree::hit_test::hit_descent::tests::{FakeNodes, el};
use crate::tree::hit_test::hit_descent::{Probe, descend, locate};
use crate::tree::hit_test::hit_verdict::Landing;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Option<Extents> {
    Some(Extents {
        x,
        y,
        width,
        height,
    })
}

fn place(nodes: &mut FakeNodes, path: &str, extents: Option<Extents>) {
    nodes.described.insert(
        path.to_owned(),
        Described {
            window_extents: extents,
            ..Described::default()
        },
    );
}

/// Chrome's page button as the live session read it: the frame's point
/// query names the button's text, which names the button its parent, while
/// the button lists no children.
fn chrome_button() -> FakeNodes {
    let mut nodes = FakeNodes::default();
    nodes
        .parent("/section", "/frame")
        .parent("/target", "/section")
        .claim("/text", "/target")
        .lead("/frame", "/text")
        .lead("/text", "/text");
    place(&mut nodes, "/section", rect(0, 87, 1280, 633));
    place(&mut nodes, "/target", rect(8, 95, 62, 21));
    place(&mut nodes, "/text", rect(16, 98, 46, 15));
    nodes
}

fn landing(nodes: &mut impl NodeSource) -> Landing {
    let deadline = Deadline::after(5_000).expect("a deadline builds");
    let (frame, target) = (el("/frame"), el("/target"));
    let chain = descend(nodes, &frame, (39, 105), deadline).expect("the descent runs");
    let probe = Probe {
        target: &target,
        frame: &frame,
        window_point: (39, 105),
    };
    locate(nodes, &probe, chain.as_deref(), deadline)
        .expect("locate runs")
        .landing
}

#[test]
fn the_hidden_text_of_a_leaf_button_lands_on_the_button() {
    let mut bare = chrome_button();
    assert_eq!(landing(&mut bare), Landing::Unproved);
    let mut leaf = LeafNodes {
        nodes: chrome_button(),
    };
    assert_eq!(landing(&mut leaf), Landing::OnTarget);
}

#[test]
fn a_parent_that_lists_other_children_does_not_own_an_unlisted_node() {
    let mut nodes = chrome_button();
    nodes
        .child_lists
        .insert("/target".to_owned(), vec!["/icon".to_owned()]);
    let mut leaf = LeafNodes { nodes };
    assert!(leaf.verified_parent(&el("/text")).expect("reads").is_none());
    assert_eq!(landing(&mut leaf), Landing::Unproved);
}

#[test]
fn a_node_reaching_outside_its_claimed_parent_is_not_owned() {
    for extents in [rect(16, 98, 80, 15), None] {
        let mut nodes = chrome_button();
        place(&mut nodes, "/text", extents);
        let mut leaf = LeafNodes { nodes };
        assert!(leaf.verified_parent(&el("/text")).expect("reads").is_none());
    }
    let mut nodes = chrome_button();
    place(&mut nodes, "/target", None);
    let mut leaf = LeafNodes { nodes };
    assert!(leaf.verified_parent(&el("/text")).expect("reads").is_none());
}

#[test]
fn a_parent_on_another_bus_does_not_own_the_node() {
    let mut leaf = LeafNodes {
        nodes: chrome_button(),
    };
    let foreign = AtspiElement::new(":1.9", "/text", 1);
    assert!(leaf.verified_parent(&foreign).expect("reads").is_none());
}

#[test]
fn a_listed_edge_is_verified_without_reading_extents() {
    let mut leaf = LeafNodes {
        nodes: chrome_button(),
    };
    assert_eq!(
        leaf.verified_parent(&el("/target"))
            .expect("reads")
            .map(|parent| parent.path().to_owned()),
        Some("/section".to_owned())
    );
    assert!(!leaf.nodes.calls.contains(&"describe"));
}
