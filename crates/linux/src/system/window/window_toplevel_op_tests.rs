use std::cell::Cell;

use agent_desktop_core::{ProcessId, WindowState};

use super::*;
use crate::system::process::process_facts::ProcessFacts;
use crate::system::window::window_id;
use crate::system::wlroots::toplevel_session::fake::{Answer, FakeToplevels, toplevel};

const INSTANCE: &str = "boot:7";

struct World {
    lookup: Lookup,
    stopped: bool,
}

fn world() -> World {
    World {
        lookup: Lookup::Found(ProcessFacts {
            executable: None,
            comm: "gnome-calculator".to_owned(),
            exe_basename: Some("gnome-calculator".to_owned()),
            instance: INSTANCE.to_owned(),
        }),
        stopped: false,
    }
}

impl ToplevelOpWorld for World {
    fn lookup(&self, _: u32) -> Lookup {
        self.lookup.clone()
    }

    fn stopped(&self, _: u32) -> bool {
        self.stopped
    }

    fn app_names(&self, _: u32) -> Vec<String> {
        vec!["gnome-calculator".to_owned()]
    }
}

fn window() -> WindowInfo {
    WindowInfo {
        id: window_id::encode(":1.4", "/org/gnome/Calculator/a11y/1").expect("an id"),
        title: "Calculator".to_owned(),
        app: "gnome-calculator".to_owned(),
        pid: ProcessId::new(300),
        process_instance: Some(INSTANCE.to_owned()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn desktop(answer: Answer) -> FakeToplevels {
    FakeToplevels::new(
        vec![
            toplevel(4, "Calculator", "org.gnome.Calculator"),
            toplevel(6, "Notes", "org.gnome.TextEditor"),
        ],
        answer,
    )
}

fn budget() -> Deadline {
    Deadline::after(10_000).expect("a deadline builds")
}

fn operate(
    world: &World,
    session: &mut FakeToplevels,
    op: WindowOp,
) -> Result<OperationReport, AdapterError> {
    run(world, || Ok(session), &window(), &op, budget())
}

fn cause_of(error: &AdapterError) -> &str {
    error.details.as_ref().expect("details")["cause"]
        .as_str()
        .expect("a cause")
}

#[test]
fn minimize_is_verified_from_the_compositor_state() {
    let mut session = desktop(Answer::Obey);
    let report = operate(&world(), &mut session, WindowOp::Minimize).expect("minimized");
    assert_eq!(
        report.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        report.result.details.expect("details")["verification_scope"],
        "window_minimized_state"
    );
    assert_eq!(session.sent, vec![(4, ToplevelRequest::SetMinimized(true))]);
    assert!(session.toplevels[0].flags.minimized);
}

#[test]
fn maximize_is_verified_from_the_compositor_state() {
    let mut session = desktop(Answer::Obey);
    let report = operate(&world(), &mut session, WindowOp::Maximize).expect("maximized");
    assert_eq!(
        report.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(session.sent, vec![(4, ToplevelRequest::SetMaximized(true))]);
}

#[test]
fn restore_lifts_a_minimized_maximized_window_and_clears_maximization() {
    let mut session = desktop(Answer::Obey);
    session.toplevels[0].flags.minimized = true;
    session.toplevels[0].flags.maximized = true;
    operate(&world(), &mut session, WindowOp::Restore).expect("restored");
    assert_eq!(
        session.sent,
        vec![
            (4, ToplevelRequest::SetMinimized(false)),
            (4, ToplevelRequest::SetMaximized(false)),
        ]
    );
    let flags = session.toplevels[0].flags;
    assert!(!flags.minimized && !flags.maximized);
}

#[test]
fn restoring_a_plain_window_sends_nothing() {
    let mut session = desktop(Answer::Obey);
    operate(&world(), &mut session, WindowOp::Restore).expect("already restored");
    assert!(session.sent.is_empty());
}

#[test]
fn a_state_the_compositor_does_not_apply_is_a_readback_mismatch() {
    let mut session = desktop(Answer::Ignore);
    let error = operate(&world(), &mut session, WindowOp::Maximize).expect_err("ignored");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(cause_of(&error), "readback_mismatch");
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["requested"]["maximized"], true);
    assert_eq!(details["observed"]["maximized"], false);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(session.settles, 1);
}

#[test]
fn a_window_that_closes_during_the_operation_is_not_found() {
    let mut session = desktop(Answer::Vanish);
    let error = operate(&world(), &mut session, WindowOp::Minimize).expect_err("closed");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn move_and_resize_are_refused_before_the_compositor_is_asked() {
    for op in [
        WindowOp::Move { x: 10.0, y: 20.0 },
        WindowOp::Resize {
            width: 400.0,
            height: 300.0,
        },
    ] {
        let opened = Cell::new(false);
        let error = run(
            &world(),
            || {
                opened.set(true);
                Ok(desktop(Answer::Obey))
            },
            &window(),
            &op,
            budget(),
        )
        .expect_err("no geometry control");
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(cause_of(&error), "wlroots_no_geometry_control");
        let details = error.details.as_ref().expect("details");
        assert_eq!(details["physical_delivery_started"], false);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert!(!opened.get());
    }
}

#[test]
fn an_invalid_size_is_still_invalid_args() {
    let mut session = desktop(Answer::Obey);
    let op = WindowOp::Resize {
        width: f64::NAN,
        height: 1.0,
    };
    let error = operate(&world(), &mut session, op).expect_err("invalid");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn twins_are_an_ambiguous_target_and_nothing_is_sent() {
    let mut session = desktop(Answer::Obey);
    session
        .toplevels
        .push(toplevel(8, "Calculator", "org.gnome.Calculator"));
    let error = operate(&world(), &mut session, WindowOp::Minimize).expect_err("ambiguous");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(cause_of(&error), "ambiguous_window");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(session.sent.is_empty());
}

#[test]
fn a_window_without_a_toplevel_is_refused() {
    let mut session = desktop(Answer::Obey);
    session.toplevels.remove(0);
    let error = operate(&world(), &mut session, WindowOp::Minimize).expect_err("missing");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(cause_of(&error), "window_not_mapped");
    assert!(session.sent.is_empty());
}

#[test]
fn a_replaced_process_is_stale_before_any_request() {
    let world = World {
        lookup: Lookup::Gone,
        ..world()
    };
    let mut session = desktop(Answer::Obey);
    let error = operate(&world, &mut session, WindowOp::Minimize).expect_err("stale");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(session.sent.is_empty());
}

#[test]
fn a_stopped_process_is_unresponsive_before_any_request() {
    let world = World {
        stopped: true,
        ..world()
    };
    let mut session = desktop(Answer::Obey);
    let error = operate(&world, &mut session, WindowOp::Maximize).expect_err("stopped");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(session.sent.is_empty());
}
