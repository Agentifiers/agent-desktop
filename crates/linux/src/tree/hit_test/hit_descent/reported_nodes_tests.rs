use super::*;
use crate::tree::Extents;
use crate::tree::hit_test::hit_descent::tests::{FakeNodes, el};

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

const DEVICE: Reported = Reported {
    scale: 1.6,
    shift: (0, 0),
};

fn browser() -> FakeNodes {
    let mut nodes = FakeNodes::default();
    nodes
        .parent("/pane", "/frame")
        .parent("/document", "/pane")
        .parent("/button", "/document");
    for (path, role, extents) in [
        ("/pane", "panel", rect(0, 87, 579, 802)),
        ("/document", "document web", rect(0, 139, 927, 1284)),
        ("/button", "push button", rect(715, 1339, 193, 65)),
    ] {
        nodes.described.insert(
            path.to_owned(),
            Described {
                role: Some(role.to_owned()),
                window_extents: Some(extents),
                ..Described::default()
            },
        );
    }
    nodes
}

fn extents(nodes: &mut impl NodeSource, path: &str) -> Option<Extents> {
    nodes.describe(&el(path)).expect("no error").window_extents
}

#[test]
fn chromium_web_content_is_turned_back_and_its_own_controls_are_not() {
    let mut nodes = ReportedNodes::new(browser(), DEVICE, ReportedScope::WebContent, &el("/frame"));
    nodes.children(&el("/frame")).expect("no error");
    assert_eq!(extents(&mut nodes, "/pane"), Some(rect(0, 87, 579, 802)));
    nodes.children(&el("/pane")).expect("no error");
    assert_eq!(
        extents(&mut nodes, "/document"),
        Some(rect(0, 87, 579, 802))
    );
    nodes.children(&el("/document")).expect("no error");
    assert_eq!(
        extents(&mut nodes, "/button"),
        Some(rect(447, 837, 121, 41))
    );
}

/// A node no walked child list named, as a point query's landing, is
/// placed by the parents it claims.
#[test]
fn an_unlisted_node_is_placed_by_its_claimed_parents() {
    let mut nodes = ReportedNodes::new(browser(), DEVICE, ReportedScope::WebContent, &el("/frame"));
    assert_eq!(
        extents(&mut nodes, "/button"),
        Some(rect(447, 837, 121, 41))
    );
    assert_eq!(extents(&mut nodes, "/pane"), Some(rect(0, 87, 579, 802)));
}

#[test]
fn a_node_whose_parents_reach_no_known_node_has_no_rectangle() {
    let mut fake = browser();
    fake.parents.remove("/pane");
    let mut nodes = ReportedNodes::new(fake, DEVICE, ReportedScope::WebContent, &el("/frame"));
    assert_eq!(extents(&mut nodes, "/pane"), None);
    assert_eq!(
        extents(&mut nodes, "/button"),
        Some(rect(447, 837, 121, 41))
    );
}

#[test]
fn every_point_query_is_asked_in_the_reported_space() {
    let mut nodes = ReportedNodes::new(browser(), DEVICE, ReportedScope::WebContent, &el("/frame"));
    nodes.at_point(&el("/frame"), (507, 857)).expect("no error");
    nodes
        .at_point(&el("/document"), (507, 857))
        .expect("no error");
    let fake = nodes.nodes;
    assert_eq!(fake.points, vec![(811, 1371), (811, 1371)]);
}

#[test]
fn a_gtk4_page_shift_applies_to_every_node_of_the_window() {
    let page = Reported {
        scale: 1.0,
        shift: (12, 12),
    };
    let mut nodes = ReportedNodes::new(browser(), page, ReportedScope::Window, &el("/frame"));
    assert_eq!(extents(&mut nodes, "/pane"), Some(rect(-12, 75, 579, 802)));
    nodes.at_point(&el("/frame"), (90, 190)).expect("no error");
    assert_eq!(nodes.nodes.points, vec![(102, 202)]);
}

#[test]
fn nodes_reported_where_they_are_drawn_pass_through() {
    let mut nodes = ReportedNodes::new(
        browser(),
        Reported::DRAWN,
        ReportedScope::WebContent,
        &el("/frame"),
    );
    assert_eq!(
        extents(&mut nodes, "/button"),
        Some(rect(715, 1339, 193, 65))
    );
    nodes.at_point(&el("/frame"), (507, 857)).expect("no error");
    assert_eq!(nodes.nodes.points, vec![(507, 857)]);
    assert!(!nodes.nodes.calls.contains(&"claimed_parent"));
}
