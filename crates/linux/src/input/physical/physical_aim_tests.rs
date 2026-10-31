use super::*;
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::x11::window::window_x11_state::WindowX11State;
use crate::tree::Extents;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn chromium(layout: Option<LayoutPlace>) -> Placement {
    Placement {
        node: Some(rect(24, 136, 120, 60)),
        frame: Some(rect(0, 0, 1280, 720)),
        frame_iconified: Some(false),
        toolkit: ToolkitId {
            name: Some("Chromium".to_owned()),
            version: Some("140.0".to_owned()),
        },
        layout,
        ..Placement::default()
    }
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn kind(error: &AdapterError) -> &str {
    error
        .details
        .as_ref()
        .and_then(|details| details["kind"].as_str())
        .unwrap_or_default()
}

#[test]
fn a_point_inside_the_node_moved_to_the_proved_origin_is_admitted() {
    let placed = chromium(Some(LayoutPlace::Proven((40, 50))));
    assert!(validate(&at(124.0, 216.0), &placed, false).is_ok());
}

#[test]
fn a_point_outside_the_placed_node_is_refused() {
    let placed = chromium(Some(LayoutPlace::Proven((40, 50))));
    let error = validate(&at(84.0, 166.0), &placed, false).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "pointer_target_not_visible");
}

#[test]
fn an_unproven_place_admits_no_point_even_with_an_x11_client() {
    let mut placed = chromium(Some(LayoutPlace::Unproven));
    placed.x11 = Some(WindowX11State {
        client_id: 7,
        hidden: false,
        client_origin: (0, 0),
        shadow: (0, 0, 0, 0),
    });
    assert!(validate(&at(84.0, 166.0), &placed, false).is_err());
}

#[test]
fn off_the_wlroots_route_the_x11_client_still_places_the_node() {
    let mut placed = chromium(None);
    placed.x11 = Some(WindowX11State {
        client_id: 7,
        hidden: false,
        client_origin: (40, 50),
        shadow: (3, 3, 3, 3),
    });
    assert!(validate(&at(124.0, 216.0), &placed, false).is_ok());
}
