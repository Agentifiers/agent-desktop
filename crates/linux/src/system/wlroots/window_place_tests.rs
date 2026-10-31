use std::cell::Cell;

use agent_desktop_core::ErrorCode;

use super::*;
use crate::system::wlroots::toplevel_flags::ToplevelFlags;

fn outputs() -> Vec<Output> {
    vec![Output {
        name: "HEADLESS-1".to_owned(),
        width: 1280,
        height: 720,
        scale: 1.0,
        logical: true,
        ..Output::default()
    }]
}

fn toplevel(title: &str, maximized: bool, activated: bool) -> ToplevelState {
    ToplevelState {
        title: title.to_owned(),
        app_id: "org.gnome.Calculator".to_owned(),
        flags: ToplevelFlags {
            maximized,
            activated,
            ..ToplevelFlags::default()
        },
        outputs: vec!["HEADLESS-1".to_owned()],
        ..ToplevelState::default()
    }
}

fn whole() -> Extents {
    Extents {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    }
}

fn names() -> Vec<String> {
    vec!["gnome-calculator".to_owned()]
}

#[test]
fn an_active_maximized_toplevel_is_placed_without_pixels() {
    let names = names();
    let identity = WindowIdentity {
        title: "Calculator",
        app_names: &names,
    };
    let asked = Cell::new(false);
    let place = choose(
        &[toplevel("Calculator", true, true)],
        &outputs(),
        identity,
        whole(),
        || {
            asked.set(true);
            Ok(None)
        },
    )
    .unwrap()
    .expect("placed");
    assert_eq!(place.origin(), (0, 0));
    let anywhere = Rect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
    assert_eq!(place.shows(anywhere), Some(true));
    assert!(!asked.get());
}

#[test]
fn an_inactive_maximized_toplevel_needs_its_pixels() {
    let names = names();
    let identity = WindowIdentity {
        title: "Calculator",
        app_names: &names,
    };
    let asked = Cell::new(false);
    let place = choose(
        &[toplevel("Calculator", true, false)],
        &outputs(),
        identity,
        whole(),
        || {
            asked.set(true);
            Ok(None)
        },
    )
    .unwrap();
    assert!(place.is_none());
    assert!(asked.get());
}

#[test]
fn a_floating_toplevel_needs_its_pixels_and_their_failure_is_kept() {
    let names = names();
    let identity = WindowIdentity {
        title: "Calculator",
        app_names: &names,
    };
    let error = choose(
        &[toplevel("Calculator", false, true)],
        &outputs(),
        identity,
        whole(),
        || Err(AdapterError::timeout("capture")),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn two_toplevels_that_share_the_title_and_app_are_not_placed_from_state() {
    let names = names();
    let identity = WindowIdentity {
        title: "Calculator",
        app_names: &names,
    };
    let place = choose(
        &[
            toplevel("Calculator", true, true),
            toplevel("Calculator", true, false),
        ],
        &outputs(),
        identity,
        whole(),
        || Ok(None),
    )
    .unwrap();
    assert!(place.is_none());
}

#[test]
fn a_toplevel_with_another_title_is_not_the_window() {
    let names = names();
    let identity = WindowIdentity {
        title: "Calculator",
        app_names: &names,
    };
    let place = choose(
        &[toplevel("Text Editor", true, true)],
        &outputs(),
        identity,
        whole(),
        || Ok(None),
    )
    .unwrap();
    assert!(place.is_none());
}
