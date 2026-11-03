//! The hit test on a wlroots compositor. No toolkit's screen extents are a
//! position there and no X11 stacking describes the windows, so the target's
//! window rectangle is moved to the origin the compositor's proof gives,
//! which keeps every toolkit's calibration rule true, and the window stands
//! alone in its stacking: whether anything covers the target is answered by
//! the proof, before the accessibility descent looks inside the window.

use agent_desktop_core::{AdapterError, Deadline, HitTestResult, Rect};

use super::hit_descent::NodeSource;
use super::{Facts, HitWorld, Windowing, hit_test_via, timed_out_or_unknown};
use crate::system::wlroots::window_place::Placed;
use crate::system::x11::FrameExtents;
use crate::system::x11::FrameFacts;
use crate::system::x11::stacking::stacked_window::StackedWindow;
use crate::system::x11::stacking::stacking_snapshot::StackingSnapshot;
use crate::system::x11::window::desktop_placement::DesktopPlacement;
use crate::system::x11::window::window_flags::WindowFlags;
use crate::system::x11::window::window_kind::WindowKind;
use crate::tree::AtspiElement;
use crate::tree::placement::frame_geometry::to_pointer_frame;
use crate::tree::placement::reported::Reported;

const OWN: u32 = 1;
/// How many proofs a cover must survive before it intercepts the pointer.
const PROOFS: u32 = 3;
/// How long a repaint is given to settle between proofs; longer than the
/// window an earlier proof is reused for.
const REPAINT: std::time::Duration = std::time::Duration::from_millis(300);

/// What a wlroots hit test reads: the target's facts from `facts`, the
/// descent's nodes from `nodes`, and the frame's place from `locate`.
/// `reported` is how the target's toolkit reports its window coordinates
/// against where it draws it: the target's rectangle is placed where it is
/// drawn, and the descent asks the toolkit in the space it reports in.
pub(crate) struct Placing<W, N, L> {
    pub(crate) facts: W,
    pub(crate) nodes: N,
    pub(crate) locate: L,
    pub(crate) reported: Reported,
}

/// The descent's nodes and the target's facts, with the target placed at
/// the proved origin and its window alone on the screen.
struct PlacedWorld<N> {
    nodes: N,
    facts: Facts,
    origin: (i32, i32),
    reported: Reported,
}

impl<N: NodeSource> HitWorld for PlacedWorld<N> {
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
        let own = StackedWindow {
            id: OWN,
            flags: WindowFlags {
                hidden: false,
                above: false,
                mapped: true,
            },
            desktop: DesktopPlacement::Sticky,
            kind: WindowKind::Normal,
            client: self.facts.screen,
            extents: FrameExtents::default(),
            shadow: (0, 0, 0, 0),
        };
        Some(Windowing {
            client: OWN,
            snapshot: StackingSnapshot {
                current_desktop: None,
                stack: vec![own],
                override_redirect_above: Vec::new(),
                override_redirect_receiving: Vec::new(),
                override_redirect_names: Vec::new(),
            },
            clients: Vec::new(),
            coordinate_origin: Some(self.origin),
            descent_root: None,
            reported: Some(self.reported),
        })
    }
}

/// Whether `point` reaches `target` in `frame`, with the placing's `locate`
/// proving where the compositor draws the frame. A frame no proof places is
/// `Unknown`; a target the proof cannot see is `Unknown`. A target the proof
/// shows covered is proved again after a pause, because the window and the
/// output are captured one after the other and a hover the pointer just
/// caused can repaint the target between them; only a cover every proof
/// shows intercepts the pointer.
pub(crate) fn hit_test_placed<W, N, L, P>(
    placing: Placing<W, N, L>,
    target: &AtspiElement,
    frame: &AtspiElement,
    point: (i32, i32),
    deadline: Deadline,
) -> Result<HitTestResult, AdapterError>
where
    W: HitWorld,
    N: NodeSource,
    L: FnMut(&FrameFacts) -> Result<Option<P>, AdapterError>,
    P: Placed,
{
    let Placing {
        facts: mut reader,
        nodes,
        mut locate,
        reported,
    } = placing;
    let Some(mut facts) = reader.facts(target, frame)? else {
        return timed_out_or_unknown(deadline);
    };
    let Some(window) = reported.drawn(facts.window) else {
        return Ok(HitTestResult::Unknown);
    };
    facts.window = window;
    let mut proofs = 0;
    let (origin, rect) = loop {
        proofs += 1;
        let Some(place) = locate(&facts.frame)? else {
            return Ok(HitTestResult::Unknown);
        };
        let origin = place.origin();
        let Some(rect) = to_pointer_frame(origin, facts.window) else {
            return Ok(HitTestResult::Unknown);
        };
        match place.shows(Rect::from(rect)) {
            Some(true) => break (origin, rect),
            Some(false) if proofs < PROOFS => {
                std::thread::sleep(REPAINT.min(deadline.remaining()));
            }
            Some(false) => {
                return Ok(HitTestResult::InterceptedBy {
                    role: Some("window".to_owned()),
                    name: None,
                    bounds: None,
                });
            }
            None => return Ok(HitTestResult::Unknown),
        }
    };
    facts.screen = rect;
    let mut world = PlacedWorld {
        nodes,
        facts,
        origin,
        reported,
    };
    hit_test_via(&mut world, target, frame, point, deadline)
}

#[cfg(test)]
#[path = "placed_world_tests.rs"]
mod tests;
