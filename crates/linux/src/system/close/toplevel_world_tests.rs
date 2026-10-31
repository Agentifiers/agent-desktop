use std::time::Duration;

use agent_desktop_core::DeliverySemantics;

use super::super::fake::Fake;
use super::super::{Request, run};
use super::*;
use crate::system::wlroots::toplevel_session::fake::{Answer, FakeToplevels, toplevel};

fn names() -> Vec<String> {
    vec!["gnome-calculator".to_owned()]
}

fn titles(list: &[&str]) -> Vec<String> {
    list.iter().map(|title| (*title).to_owned()).collect()
}

fn request() -> Request<'static> {
    Request {
        name: "gnome-calculator",
        pid: 42,
        instance: Some("t"),
        force: false,
    }
}

fn desktop() -> FakeToplevels {
    FakeToplevels::new(
        vec![
            toplevel(3, "Calculator", "org.gnome.Calculator"),
            toplevel(5, "Notes", "org.gnome.TextEditor"),
        ],
        Answer::Obey,
    )
}

fn world(
    inner: Fake,
    session: FakeToplevels,
    frames: Vec<String>,
) -> ToplevelWorld<Fake, FakeToplevels, impl Fn() -> Option<Vec<String>>> {
    ToplevelWorld {
        inner,
        session: RefCell::new(session),
        names: names(),
        frames: move || Some(frames.clone()),
        deadline: Deadline::after(10_000).expect("a deadline builds"),
        ambiguous: Cell::new(None),
    }
}

fn exits_soon() -> Fake {
    let mut inner = Fake::new(Liveness::Alive);
    inner.gone_after = Some(Duration::from_millis(100));
    inner
}

#[test]
fn the_process_frames_name_its_toplevels() {
    let mut session = desktop();
    session.toplevels[0].has_parent = true;
    let owned = owned_toplevels(
        &session.toplevels,
        &titles(&["Calculator", "Unshown"]),
        &names(),
    )
    .expect("one toplevel");
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].id, 3);
    assert_eq!(owned[0].kind, WindowKind::Dialog);
    assert!(owned[0].transient);
}

#[test]
fn twin_toplevels_of_one_title_and_app_are_counted() {
    let mut session = desktop();
    session
        .toplevels
        .push(toplevel(7, "Calculator", "org.gnome.Calculator"));
    assert_eq!(
        owned_toplevels(&session.toplevels, &titles(&["Calculator"]), &names()),
        Err(2)
    );
}

#[test]
fn a_graceful_close_asks_the_matched_toplevel_and_waits_for_the_exit() {
    let world = world(exits_soon(), desktop(), titles(&["Calculator"]));
    let exited = run(&world, &request()).expect("closed");
    assert!(exited);
    let session = world.session.borrow();
    assert_eq!(session.sent, vec![(3, ToplevelRequest::Close)]);
    assert_eq!(world.inner.count("term"), 0);
}

#[test]
fn twins_are_an_ambiguous_target_and_nothing_is_closed() {
    let mut session = desktop();
    session
        .toplevels
        .push(toplevel(7, "Calculator", "org.gnome.Calculator"));
    let world = world(exits_soon(), session, titles(&["Calculator"]));
    let error = run(&world, &request()).expect_err("ambiguous");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["cause"], "ambiguous_window");
    assert_eq!(details["candidates"], 2);
    assert!(world.session.borrow().sent.is_empty());
}

#[test]
fn a_process_without_a_shown_toplevel_is_windowless_not_a_wayland_refusal() {
    let world = world(
        Fake::new(Liveness::Alive),
        desktop(),
        titles(&["Preferences"]),
    );
    let error = run(&world, &request()).expect_err("windowless");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.message.contains("No window of process 42"));
    assert!(world.session.borrow().sent.is_empty());
}

#[test]
fn an_open_dialog_toplevel_is_left_for_the_user() {
    let mut session = desktop();
    session
        .toplevels
        .push(toplevel(9, "Save Changes?", "org.gnome.Calculator"));
    session.toplevels[2].has_parent = true;
    let world = world(
        Fake::new(Liveness::Alive),
        session,
        titles(&["Calculator", "Save Changes?"]),
    );
    let error = run(&world, &request()).expect_err("dialog pending");
    assert_eq!(
        error.details.as_ref().expect("details")["cause"],
        "dialog_pending"
    );
    assert!(world.session.borrow().sent.is_empty());
}

#[test]
fn a_toplevel_that_vanished_before_the_request_is_skipped() {
    let mut session = desktop();
    session.send_error = Some(AdapterError::new(ErrorCode::WindowNotFound, "gone"));
    let world = world(Fake::new(Liveness::Alive), session, titles(&["Calculator"]));
    let error = run(&world, &request()).expect_err("nothing could be asked");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn an_app_that_stays_open_times_out_with_the_request_delivered() {
    let mut inner = Fake::new(Liveness::Alive);
    inner.budget = Duration::from_millis(200);
    let world = world(inner, desktop(), titles(&["Calculator"]));
    let error = run(&world, &request()).expect_err("still open");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
