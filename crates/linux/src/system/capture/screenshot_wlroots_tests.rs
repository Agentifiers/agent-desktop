use std::cell::Cell;

use agent_desktop_core::{DeliverySemantics, ErrorCode};

use super::tests::{portal_only, x11};
use super::*;
use crate::system::capture::wayland_capture::CopyOutput;
use crate::system::capture::window::fakes::*;
use crate::system::capture::wlroots_window::fakes::{World, image, no_names, output, toplevel};
use crate::system::wlroots::captured_image::CapturedImage;
use crate::system::wlroots::output::Output;

fn portal_refuses(_: &ScreenshotTarget, _: Deadline) -> Result<ImageBuffer, AdapterError> {
    Err(AdapterError::new(ErrorCode::Timeout, "the portal stub"))
}

fn copy_a(name: &str, _: Deadline) -> Result<(Output, CapturedImage), AdapterError> {
    assert_eq!(name, "A");
    Ok((output("A", 0, 1920, 1.0), image(4, 2, None)))
}

fn copy_never(_: &str, _: Deadline) -> Result<(Output, CapturedImage), AdapterError> {
    panic!("only a wlroots route copies an output")
}

fn screencopy(copy: CopyOutput<'_>) -> WaylandCapture<'_> {
    WaylandCapture {
        outputs: Some(copy),
        ..portal_only(&portal_refuses)
    }
}

#[test]
fn a_wlroots_compositor_with_screencopy_copies_the_display_without_the_portal() {
    let rig = Rig::standard();
    let portal_calls = Cell::new(0);
    let portal = |_: &ScreenshotTarget, _: Deadline| {
        portal_calls.set(portal_calls.get() + 1);
        portal_refuses(&ScreenshotTarget::FullScreen, deadline())
    };
    let wayland = WaylandCapture {
        outputs: Some(&copy_a),
        ..portal_only(&portal)
    };
    let image = capture_through(
        &rig.env(),
        &Route::Wlroots,
        ScreenshotTarget::FullScreen,
        deadline(),
        &wayland,
    )
    .expect("an image");
    assert_eq!((image.width, image.height), (4, 2));
    assert_eq!(portal_calls.get(), 0);
    assert_eq!(rig.grabber.grab_count(), 0);
}

#[test]
fn a_wlroots_compositor_without_screencopy_keeps_the_portal_and_its_consent_wait() {
    let rig = Rig::standard();
    let waited = Cell::new(None);
    let portal = |_: &ScreenshotTarget, given: Deadline| {
        waited.set(Some(given.remaining()));
        portal_refuses(&ScreenshotTarget::FullScreen, given)
    };
    let error = capture_through(
        &rig.env(),
        &Route::Wlroots,
        ScreenshotTarget::Screen(0),
        Deadline::after(1).expect("a deadline"),
        &portal_only(&portal),
    )
    .expect_err("the stub refuses");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let waited = waited.get().expect("the portal was called");
    assert!(waited > std::time::Duration::from_millis(CONSENT_BUDGET_MS - 2_000));
}

#[test]
fn mutter_never_copies_an_output_or_a_toplevel() {
    let rig = Rig::standard();
    let world = World::of(vec![toplevel("a", "Editor", "editor")], image(2, 2, None));
    let open = |_: Deadline| world.open();
    let wayland = WaylandCapture {
        toplevels: Some(ToplevelRoute {
            open: &open,
            app_names: no_names,
        }),
        ..screencopy(&copy_never)
    };
    let error = capture_through(
        &rig.env(),
        &Route::Wayland,
        ScreenshotTarget::FullScreen,
        deadline(),
        &wayland,
    )
    .expect_err("the portal stub refuses");
    assert_eq!(error.code, ErrorCode::Timeout);
    let window = ScreenshotTarget::ExactWindow(target());
    let error = capture_through(&rig.env(), &Route::Wayland, window, deadline(), &wayland)
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    let error = capture_frame_through(&rig.env(), &Route::Wayland, &target(), deadline(), &wayland)
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(world.opened(), 0);
    assert_eq!(rig.x11_touched(), 0);
}

#[test]
fn x11_grabs_the_root_whatever_wayland_capture_is_offered() {
    let rig = Rig::standard();
    let image = capture_through(
        &rig.env(),
        &x11(),
        ScreenshotTarget::FullScreen,
        deadline(),
        &screencopy(&copy_never),
    )
    .expect("an image");
    assert_eq!(rig.grabber.grab_count(), 1);
    assert_eq!((image.width, image.height), (1920, 1080));
}

#[test]
fn a_wlroots_window_without_toplevel_capture_is_refused_before_any_read() {
    let rig = Rig::standard();
    let wayland = screencopy(&copy_never);
    let window = ScreenshotTarget::ExactWindow(target());
    let error = capture_through(&rig.env(), &Route::Wlroots, window, deadline(), &wayland)
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let error = capture_frame_through(&rig.env(), &Route::Wlroots, &target(), deadline(), &wayland)
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(rig.x11_touched(), 0);
}

#[test]
fn a_wlroots_window_is_copied_as_its_toplevel() {
    let rig = Rig::standard();
    rig.lookup.resolved.set(vec![Some(window_info(None))]);
    let world = World::of(vec![toplevel("a", "Editor", "editor")], image(3, 2, None));
    let open = |_: Deadline| world.open();
    let wayland = WaylandCapture {
        toplevels: Some(ToplevelRoute {
            open: &open,
            app_names: no_names,
        }),
        ..screencopy(&copy_never)
    };
    let window = ScreenshotTarget::ExactWindow(target());
    let image = capture_through(&rig.env(), &Route::Wlroots, window, deadline(), &wayland)
        .expect("a window image");
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(world.copied(), vec!["a".to_owned()]);
    assert_eq!(rig.grabber.grab_count(), 0);
}
