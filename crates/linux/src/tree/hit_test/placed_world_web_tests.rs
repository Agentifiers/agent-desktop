use super::*;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::Extents;
use crate::tree::hit_test::hit_descent::Described;
use crate::tree::hit_test::hit_descent::tests::{FakeNodes, el};

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

/// Chromium at Amp's fractional scale in a 579x889 window: its own pane
/// reads layout units, its web document and the page button in its far
/// corner read device pixels.
const DEVICE: Reported = Reported {
    scale: 1.6,
    shift: (0, 0),
};
const FAR_BUTTON: Extents = Extents {
    x: 715,
    y: 1339,
    width: 193,
    height: 65,
};

struct Reader {
    facts: Facts,
    nodes: FakeNodes,
}

impl HitWorld for Reader {
    fn nodes(&mut self) -> &mut impl NodeSource {
        &mut self.nodes
    }

    fn facts(
        &mut self,
        _target: &AtspiElement,
        _frame: &AtspiElement,
    ) -> Result<Option<Facts>, AdapterError> {
        Ok(Some(self.facts.clone()))
    }

    fn windowing(&mut self, _frame: &FrameFacts) -> Option<Windowing> {
        None
    }
}

struct Shown;

impl Placed for Shown {
    fn origin(&self) -> (i32, i32) {
        (0, 0)
    }

    fn shows(&self, _rect: Rect) -> Option<bool> {
        Some(true)
    }
}

fn described(role: &str, extents: Extents) -> Described {
    Described {
        role: Some(role.to_owned()),
        window_extents: Some(extents),
        ..Described::default()
    }
}

fn browser() -> FakeNodes {
    let mut nodes = FakeNodes::default();
    nodes
        .parent("/pane", "/frame")
        .parent("/document", "/pane")
        .parent("/target", "/document");
    for (path, role, extents) in [
        ("/pane", "panel", rect(0, 87, 579, 802)),
        ("/document", "document web", rect(0, 139, 927, 1284)),
        ("/target", "push button", FAR_BUTTON),
    ] {
        nodes
            .described
            .insert(path.to_owned(), described(role, extents));
    }
    nodes
        .lead("/frame", "/document")
        .lead("/document", "/target")
        .lead("/target", "/target");
    nodes
}

fn reader() -> Reader {
    Reader {
        facts: Facts {
            screen: FAR_BUTTON,
            window: FAR_BUTTON,
            frame: FrameFacts {
                size_identity: false,
                pid: 1,
                title: "Alignment Page - Chromium".to_owned(),
                extents: None,
                global_position: true,
            },
            minimized: false,
            toolkit: ToolkitId {
                name: Some("Chromium".to_owned()),
                version: Some("1.0".to_owned()),
            },
        },
        nodes: FakeNodes::default(),
    }
}

fn hit(nodes: FakeNodes, point: (i32, i32)) -> Result<HitTestResult, AdapterError> {
    let placing = Placing {
        facts: reader(),
        nodes,
        locate: |_: &FrameFacts| -> Result<Option<Shown>, AdapterError> { Ok(Some(Shown)) },
        reported: DEVICE,
    };
    let deadline = Deadline::after(5_000).expect("deadline");
    hit_test_placed(placing, &el("/target"), &el("/frame"), point, deadline)
}

/// The button drawn at (447, 837, 121, 41) in the window's far corner reads
/// (715, 1339) in device pixels, outside the pane that holds it in layout
/// units. Each node is compared with the probe in its own unit, and the
/// toolkit is asked about the point in device pixels.
#[test]
fn a_far_page_button_is_reached_through_the_browser_pane_that_holds_it() {
    assert_eq!(
        hit(browser(), (507, 857)).expect("no error"),
        HitTestResult::ReachesTarget
    );
}
