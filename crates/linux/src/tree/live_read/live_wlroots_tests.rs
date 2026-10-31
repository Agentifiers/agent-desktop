use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, NativeHandle};

use super::pointer_refusal::pointer_refusal_via;
use super::*;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::observe::node_evidence::evidence_for;
use crate::tree::placement::layout_place::LayoutPlace;
use crate::tree::vocabulary::role_numbers::PUSH_BUTTON;
use crate::tree::vocabulary::state_bits::{ENABLED, SHOWING, VISIBLE};
use crate::tree::{AtspiElement, Extents, Field, element::ancestry};

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

/// A Chromium node on a wlroots compositor: its screen extents carry the
/// toolkit's own guess of the window position, which is no position.
struct Wlroots {
    layout: LayoutPlace,
    iconified: bool,
}

impl NodeReader for Wlroots {
    fn read(&mut self, element: &AtspiElement) -> Result<NodeEvidence, AdapterError> {
        let mut read = read_of(PUSH_BUTTON, "Press me", &["click"]);
        read.node.traits.states = Field::Present(words(&[ENABLED, VISIBLE, SHOWING]));
        read.node.extents = Field::Present(rect(34, 146, 120, 60));
        Ok(NodeEvidence {
            evidence: evidence_for(&read, element),
            read,
            liveness: Liveness::Alive,
        })
    }

    fn stopped_app(&mut self, _pid: u32) -> Option<String> {
        None
    }

    fn placement(&mut self, _element: &AtspiElement, _deadline: Deadline) -> Placement {
        Placement {
            node: Some(rect(24, 136, 120, 60)),
            frame: Some(rect(0, 0, 1280, 720)),
            frame_iconified: Some(self.iconified),
            toolkit: ToolkitId {
                name: Some("Chromium".to_owned()),
                version: Some("140.0".to_owned()),
            },
            wayland_unplaced: self.layout == LayoutPlace::Unproven,
            layout: Some(self.layout),
            ..Placement::default()
        }
    }

    fn global_position(&mut self, _element: &AtspiElement, _deadline: Deadline) -> bool {
        false
    }
}

fn handle() -> NativeHandle {
    let mut frame = AtspiElement::new(":1.9", "/org/a11y/atspi/accessible/199", 7);
    frame.set_ancestry(ancestry::of_root(&frame.clone()));
    let mut element = AtspiElement::new(":1.9", "/org/a11y/atspi/accessible/301", 7);
    element.set_ancestry(ancestry::of_root(&frame));
    element.into_native_handle()
}

fn deadline() -> Deadline {
    Deadline::after(1_000).expect("deadline")
}

fn reader(layout: LayoutPlace) -> Wlroots {
    Wlroots {
        layout,
        iconified: false,
    }
}

#[test]
fn a_proved_origin_places_the_window_rectangle_and_never_the_toolkit_guess() {
    let bounds = bounds_via(
        &mut reader(LayoutPlace::Proven((0, 0))),
        &handle(),
        deadline(),
    )
    .expect("bounds read")
    .expect("placed");
    assert_eq!((bounds.x, bounds.y), (24.0, 136.0));
    let moved = bounds_via(
        &mut reader(LayoutPlace::Proven((40, 50))),
        &handle(),
        deadline(),
    )
    .expect("bounds read")
    .expect("placed");
    assert_eq!((moved.x, moved.y), (64.0, 186.0));
    let pointer = pointer_via(
        &mut reader(LayoutPlace::Proven((0, 0))),
        &handle(),
        deadline(),
    )
    .expect("pointer read");
    assert_eq!(pointer.bounds, Some(bounds));
    assert!(
        pointer_refusal_via(
            &mut reader(LayoutPlace::Proven((0, 0))),
            &handle(),
            deadline()
        )
        .expect("the read answers")
        .is_none()
    );
}

#[test]
fn a_proved_origin_proves_the_node_on_screen() {
    let mut wlroots = reader(LayoutPlace::Proven((0, 0)));
    let element = AtspiElement::new(":1.9", "/org/a11y/atspi/accessible/301", 7);
    let node = wlroots.read(&element).expect("read");
    let placed = wlroots.placement(&element, deadline());
    let live = live_project::live_element(&node.read, &node.evidence, &placed, false)
        .expect("element projects");
    assert_eq!(live.state.offscreen, Some(false));
    assert_eq!(
        live.bounds.map(|rect| (rect.x, rect.y)),
        Some((24.0, 136.0))
    );
    let unproven = reader(LayoutPlace::Unproven).placement(&element, deadline());
    let live = live_project::live_element(&node.read, &node.evidence, &unproven, false)
        .expect("element projects");
    assert_eq!(live.bounds, None);
}

#[test]
fn an_unproven_origin_has_no_bounds_and_refuses_the_pointer_by_name() {
    assert_eq!(
        bounds_via(&mut reader(LayoutPlace::Unproven), &handle(), deadline()).expect("read"),
        None
    );
    let refusal = pointer_refusal_via(&mut reader(LayoutPlace::Unproven), &handle(), deadline())
        .expect("the read answers")
        .expect("an unproven origin is refused");
    assert_eq!(refusal.code, ErrorCode::ActionFailed);
    assert_eq!(refusal.disposition, DeliverySemantics::not_delivered());
    let details = refusal.details.expect("details");
    assert_eq!(details["kind"], "wlroots_window_origin_unproven");
    assert_eq!(details["physical_delivery_started"], false);
    assert!(details["window_id"].is_string());
}

#[test]
fn a_minimized_window_is_not_refused_as_unplaced() {
    let mut minimized = Wlroots {
        layout: LayoutPlace::Unproven,
        iconified: true,
    };
    assert!(
        pointer_refusal_via(&mut minimized, &handle(), deadline())
            .expect("the read answers")
            .is_none()
    );
}

#[test]
fn a_pointer_aimed_into_an_unproven_window_is_refused_not_scrolled() {
    let error = pointer_via(&mut reader(LayoutPlace::Unproven), &handle(), deadline())
        .expect_err("an unproven origin has no pointer position");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.expect("details")["kind"],
        "wlroots_window_origin_unproven"
    );
    let mut minimized = Wlroots {
        layout: LayoutPlace::Unproven,
        iconified: true,
    };
    let observed = pointer_via(&mut minimized, &handle(), deadline()).expect("observation");
    assert_eq!(observed.bounds, None);
}
