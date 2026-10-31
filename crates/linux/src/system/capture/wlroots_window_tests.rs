use agent_desktop_core::Rect;

use super::fakes::*;
use super::*;
use crate::system::capture::window::fakes::*;
use crate::system::wlroots::captured_image::CapturedImage;
use crate::tree::Extents;

fn shadowed() -> CapturedImage {
    black_margins(
        8,
        6,
        Extents {
            x: 2,
            y: 1,
            width: 4,
            height: 3,
        },
    )
}

fn unbounded(rig: &Rig) {
    rig.lookup.resolved.set(vec![Some(window_info(None))]);
}

fn run(
    rig: &Rig,
    world: &World,
    strict: bool,
    window: &WindowInfo,
) -> Result<ImageBuffer, AdapterError> {
    let open = |_: Deadline| world.open();
    let route = ToplevelRoute {
        open: &open,
        app_names: no_names,
    };
    capture(&rig.env(), window, strict, deadline(), &route)
}

#[test]
fn the_one_toplevel_with_the_window_title_is_copied_without_its_shadow() {
    let rig = Rig::standard();
    unbounded(&rig);
    rig.lookup.frame_sizes.set(vec![Some((4, 3))]);
    let world = World::of(
        vec![
            toplevel("a", "Other", "editor"),
            toplevel("b", "Editor", "editor"),
        ],
        shadowed(),
    );
    let image = run(&rig, &world, false, &target()).expect("a window image");
    assert_eq!(world.copied(), vec!["b".to_owned()]);
    assert_eq!((image.width, image.height), (4, 3));
    assert_eq!(&image.data[..6], &[2, 1, 9, 3, 1, 9]);
    assert_eq!(image.scale_factor, 1.0);
}

#[test]
fn a_window_whose_frame_size_is_unknown_is_its_whole_surface() {
    let rig = Rig::standard();
    unbounded(&rig);
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let image = run(&rig, &world, false, &target()).expect("a window image");
    assert_eq!((image.width, image.height), (8, 6));
}

#[test]
fn twin_titles_are_told_apart_by_the_application_id() {
    let rig = Rig::standard();
    unbounded(&rig);
    let world = World::of(
        vec![
            toplevel("a", "Editor", "foot"),
            toplevel("b", "Editor", "org.example.Editor"),
        ],
        shadowed(),
    );
    run(&rig, &world, false, &target()).expect("a window image");
    assert_eq!(world.copied(), vec!["b".to_owned()]);
}

#[test]
fn twin_titles_of_one_application_are_ambiguous_and_nothing_is_copied() {
    let rig = Rig::standard();
    unbounded(&rig);
    let world = World::of(
        vec![
            toplevel("a", "Editor", "editor"),
            toplevel("b", "Editor", "editor"),
        ],
        shadowed(),
    );
    let error = run(&rig, &world, false, &target()).expect_err("ambiguous");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|d| d["candidate_count"].as_u64()),
        Some(2)
    );
    assert!(world.copied().is_empty());
}

#[test]
fn a_window_no_toplevel_carries_is_not_found_and_nothing_is_copied() {
    let rig = Rig::standard();
    unbounded(&rig);
    let world = World::of(vec![toplevel("a", "Other", "editor")], shadowed());
    let error = run(&rig, &world, false, &target()).expect_err("missing");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(world.copied().is_empty());
}

#[test]
fn a_toplevel_retitled_during_the_copy_is_refused() {
    let rig = Rig::standard();
    unbounded(&rig);
    let mut world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    world.after = Some(vec![toplevel("a", "Saved", "editor")]);
    let error = run(&rig, &world, false, &target()).expect_err("changed");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_window_whose_bounds_move_during_the_copy_is_refused() {
    let rig = Rig::standard();
    let moved = Rect {
        x: 5.0,
        ..frame_bounds().expect("bounds")
    };
    rig.lookup.resolved.set(vec![
        Some(window_info(frame_bounds())),
        Some(window_info(Some(moved))),
    ]);
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let error = run(&rig, &world, false, &target()).expect_err("moved");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn a_window_without_an_instance_token_or_with_a_stopped_process_opens_no_list() {
    let mut rig = Rig::standard();
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let mut window = target();
    window.process_instance = None;
    let error = run(&rig, &world, false, &window).expect_err("no token");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    rig.stopped = always_stopped;
    let error = run(&rig, &world, false, &target()).expect_err("stopped");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(world.opened(), 0);
}

#[test]
fn the_debug_frame_refuses_an_image_that_agrees_with_no_bounds() {
    let rig = Rig::standard();
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let error = run(&rig, &world, true, &target()).expect_err("unproven");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn the_debug_frame_takes_the_opaque_box_that_agrees_with_the_bounds() {
    let rig = Rig::standard();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 4.0,
        height: 3.0,
    };
    rig.lookup
        .resolved
        .set(vec![Some(window_info(Some(bounds)))]);
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let image = run(&rig, &world, true, &target()).expect("a framed image");
    assert_eq!((image.width, image.height), (4, 3));
}

#[test]
fn the_scale_is_the_one_every_display_shares_when_the_window_has_no_bounds() {
    let rig = Rig::standard();
    unbounded(&rig);
    rig.lookup.displays.set(vec![Some(vec![
        display_at("A", 0.0, 1920.0, 2.0, true),
        display_at("B", 1920.0, 1920.0, 2.0, false),
    ])]);
    let world = World::of(vec![toplevel("a", "Editor", "editor")], shadowed());
    let image = run(&rig, &world, false, &target()).expect("an image");
    assert_eq!(image.scale_factor, 2.0);
    rig.lookup.displays.set(vec![Some(vec![
        display_at("A", 0.0, 1920.0, 2.0, true),
        display_at("B", 1920.0, 1920.0, 1.0, false),
    ])]);
    let image = run(&rig, &world, false, &target()).expect("an image");
    assert_eq!(image.scale_factor, 1.0);
}

#[test]
fn identify_reads_only_the_listed_toplevels() {
    let listed = vec![toplevel("x", "Editor", "editor")];
    let window = window_info(None);
    assert_eq!(identify(&listed, &window, &[]).expect("unique"), "x");
}
