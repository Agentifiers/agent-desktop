use std::cell::Cell;

use agent_desktop_core::ErrorCode;

use super::*;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::Extents;
use crate::tree::hit_test::hit_descent::tests::{FakeNodes, el};
use crate::tree::hit_test::hit_descent::{Described, LeafNodes};

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn toolkit(name: &str, version: &str) -> ToolkitId {
    ToolkitId {
        name: Some(name.to_owned()),
        version: Some(version.to_owned()),
    }
}

fn facts(toolkit: ToolkitId, screen: Extents, window: Extents) -> Facts {
    Facts {
        screen,
        window,
        frame: FrameFacts {
            size_identity: false,
            pid: 1,
            title: "Calculator".to_owned(),
            extents: None,
            global_position: false,
        },
        minimized: false,
        toolkit,
    }
}

fn gtk4() -> Facts {
    facts(
        toolkit("gtk", "4.14.5"),
        rect(0, 0, 80, 40),
        rect(100, 200, 80, 40),
    )
}

fn chromium() -> Facts {
    facts(
        toolkit("Chromium", "140.0"),
        rect(126, 226, 80, 40),
        rect(116, 216, 80, 40),
    )
}

struct World {
    nodes: FakeNodes,
    facts: Option<Facts>,
    windowing_asked: bool,
}

impl World {
    fn new(facts: Option<Facts>) -> Self {
        let mut nodes = FakeNodes::default();
        nodes.parent("/pane", "/frame").parent("/target", "/pane");
        let target = facts
            .as_ref()
            .map_or(rect(0, 0, 1, 1), |facts| facts.window);
        for (path, extents) in [("/pane", rect(0, 0, 1280, 720)), ("/target", target)] {
            nodes.described.insert(
                path.to_owned(),
                Described {
                    window_extents: Some(extents),
                    ..Described::default()
                },
            );
        }
        nodes
            .lead("/frame", "/pane")
            .lead("/pane", "/target")
            .lead("/target", "/target");
        Self {
            nodes,
            facts,
            windowing_asked: false,
        }
    }
}

impl HitWorld for &mut World {
    fn nodes(&mut self) -> &mut impl NodeSource {
        &mut self.nodes
    }

    fn facts(
        &mut self,
        _target: &AtspiElement,
        _frame: &AtspiElement,
    ) -> Result<Option<Facts>, AdapterError> {
        Ok(self.facts.clone())
    }

    fn windowing(&mut self, _frame: &FrameFacts) -> Option<Windowing> {
        self.windowing_asked = true;
        None
    }
}

struct Place {
    origin: (i32, i32),
    shows: Option<bool>,
    asked: Cell<Option<Rect>>,
}

impl Place {
    fn at(origin: (i32, i32), shows: Option<bool>) -> Self {
        Self {
            origin,
            shows,
            asked: Cell::new(None),
        }
    }
}

impl Placed for &Place {
    fn origin(&self) -> (i32, i32) {
        self.origin
    }

    fn shows(&self, rect: Rect) -> Option<bool> {
        self.asked.set(Some(rect));
        self.shows
    }
}

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn run_displaced(
    world: &mut World,
    place: Option<&Place>,
    point: (i32, i32),
    reported: Reported,
) -> HitTestResult {
    let nodes = std::mem::take(&mut world.nodes);
    let placing = Placing {
        facts: world,
        nodes,
        locate: |_: &FrameFacts| Ok(place),
        reported,
    };
    hit_test_placed(placing, &el("/target"), &el("/frame"), point, deadline()).expect("no error")
}

fn run(world: &mut World, place: Option<&Place>, point: (i32, i32)) -> HitTestResult {
    run_displaced(world, place, point, Reported::DRAWN)
}

#[test]
fn a_gtk4_target_reaches_at_its_window_rectangle_moved_to_the_origin() {
    let mut world = World::new(Some(gtk4()));
    let place = Place::at((40, 30), Some(true));
    assert_eq!(
        run(&mut world, Some(&place), (180, 250)),
        HitTestResult::ReachesTarget
    );
    assert_eq!(
        place.asked.get(),
        Some(Rect {
            x: 140.0,
            y: 230.0,
            width: 80.0,
            height: 40.0,
        })
    );
    assert!(!world.windowing_asked);
}

const PAGE: Reported = Reported {
    scale: 1.0,
    shift: (12, 12),
};

/// A GTK 4 target inside a stack page reads its window rectangle the page's
/// offset further right and down than it is drawn; the frame's origin moved
/// back by that displacement places it where it is drawn and asks the
/// toolkit about the point in the toolkit's own window coordinates.
#[test]
fn a_gtk4_target_in_a_stack_page_is_placed_back_by_the_page_offset() {
    let mut world = World::new(Some(gtk4()));
    let place = Place::at((40, 30), Some(true));
    assert_eq!(
        run_displaced(&mut world, Some(&place), (130, 220), PAGE),
        HitTestResult::ReachesTarget
    );
    assert_eq!(
        place.asked.get(),
        Some(Rect {
            x: 128.0,
            y: 218.0,
            width: 80.0,
            height: 40.0,
        })
    );
    let mut undisplaced = World::new(Some(gtk4()));
    assert_eq!(
        run(&mut undisplaced, Some(&place), (130, 220)),
        HitTestResult::Unknown
    );
}

#[test]
fn chromium_screen_extents_are_replaced_by_the_placed_rectangle() {
    let mut world = World::new(Some(chromium()));
    let place = Place::at((0, 0), Some(true));
    assert_eq!(
        run(&mut world, Some(&place), (150, 230)),
        HitTestResult::ReachesTarget
    );
    let mut world = World::new(Some(chromium()));
    assert_eq!(
        run(&mut world, Some(&place), (200, 260)),
        HitTestResult::Unknown
    );
}

/// Chrome after its renderer answered a point query at the button: the
/// frame's query names the button's text, which names the button its parent
/// while the button lists no children.
fn chrome_hidden_text(world: &mut World) {
    world.nodes.leads.clear();
    world
        .nodes
        .claim("/text", "/target")
        .lead("/frame", "/text")
        .lead("/text", "/text");
    world.nodes.described.insert(
        "/text".to_owned(),
        Described {
            window_extents: Some(rect(124, 224, 60, 28)),
            ..Described::default()
        },
    );
}

#[test]
fn chromium_button_text_at_the_point_reaches_the_leaf_button_it_belongs_to() {
    let place = Place::at((0, 0), Some(true));
    let mut bare = World::new(Some(chromium()));
    chrome_hidden_text(&mut bare);
    assert_eq!(
        run(&mut bare, Some(&place), (150, 230)),
        HitTestResult::Unknown
    );
    let mut world = World::new(Some(chromium()));
    chrome_hidden_text(&mut world);
    let nodes = LeafNodes {
        nodes: std::mem::take(&mut world.nodes),
    };
    let placing = Placing {
        facts: &mut world,
        nodes,
        locate: |_: &FrameFacts| -> Result<Option<&Place>, AdapterError> { Ok(Some(&place)) },
        reported: Reported::DRAWN,
    };
    let result = hit_test_placed(
        placing,
        &el("/target"),
        &el("/frame"),
        (150, 230),
        deadline(),
    )
    .expect("no error");
    assert_eq!(result, HitTestResult::ReachesTarget);
}

#[test]
fn a_target_the_proof_shows_covered_is_intercepted_by_a_window() {
    let mut world = World::new(Some(gtk4()));
    let place = Place::at((40, 30), Some(false));
    assert_eq!(
        run(&mut world, Some(&place), (180, 250)),
        HitTestResult::InterceptedBy {
            role: Some("window".to_owned()),
            name: None,
            bounds: None,
        }
    );
}

#[test]
fn a_target_the_proof_cannot_see_is_unknown() {
    let mut world = World::new(Some(gtk4()));
    let place = Place::at((40, 30), None);
    assert_eq!(
        run(&mut world, Some(&place), (180, 250)),
        HitTestResult::Unknown
    );
}

#[test]
fn an_unplaced_frame_or_unread_facts_are_unknown() {
    let mut world = World::new(Some(gtk4()));
    assert_eq!(run(&mut world, None, (180, 250)), HitTestResult::Unknown);
    let mut world = World::new(None);
    let place = Place::at((40, 30), Some(true));
    assert_eq!(
        run(&mut world, Some(&place), (180, 250)),
        HitTestResult::Unknown
    );
    assert_eq!(place.asked.get(), None);
}

#[test]
fn a_point_outside_the_placed_target_is_unknown() {
    let mut world = World::new(Some(gtk4()));
    let place = Place::at((40, 30), Some(true));
    assert_eq!(
        run(&mut world, Some(&place), (100, 200)),
        HitTestResult::Unknown
    );
}

#[test]
fn a_failed_proof_is_the_hit_test_failure() {
    let mut world = World::new(Some(gtk4()));
    let nodes = std::mem::take(&mut world.nodes);
    let placing = Placing {
        facts: &mut world,
        nodes,
        locate: |_: &FrameFacts| -> Result<Option<&Place>, AdapterError> {
            Err(AdapterError::timeout("capture"))
        },
        reported: Reported::DRAWN,
    };
    let error = hit_test_placed(
        placing,
        &el("/target"),
        &el("/frame"),
        (180, 250),
        deadline(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_cover_one_proof_sees_and_the_next_does_not_is_a_repaint() {
    let mut world = World::new(Some(gtk4()));
    let covered = Place::at((40, 30), Some(false));
    let shown = Place::at((40, 30), Some(true));
    let mut proofs = 0;
    let nodes = std::mem::take(&mut world.nodes);
    let placing = Placing {
        facts: &mut world,
        nodes,
        locate: |_: &FrameFacts| -> Result<Option<&Place>, AdapterError> {
            proofs += 1;
            Ok(Some(if proofs == 1 { &covered } else { &shown }))
        },
        reported: Reported::DRAWN,
    };
    let result = hit_test_placed(
        placing,
        &el("/target"),
        &el("/frame"),
        (180, 250),
        deadline(),
    )
    .unwrap();
    assert_eq!(result, HitTestResult::ReachesTarget);
    assert_eq!(proofs, 2);
}
