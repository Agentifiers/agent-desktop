use std::cell::Cell;

use agent_desktop_core::{ProcessId, WindowState};

use super::*;
use crate::system::process::process_facts::ProcessFacts;
use crate::system::wlroots::toplevel_session::fake::{Answer, FakeToplevels, toplevel};

const INSTANCE: &str = "boot:42";

struct World {
    lookup: Lookup,
    stopped: bool,
    frame_active: bool,
    frame_dialog: bool,
    frame_reads: Cell<u32>,
    needs_keyboard: bool,
    attach_fails: bool,
    attached: Cell<bool>,
}

impl World {
    fn new() -> Self {
        Self {
            lookup: Lookup::Found(ProcessFacts {
                executable: None,
                comm: "gnome-text-editor".to_owned(),
                exe_basename: Some("gnome-text-editor".to_owned()),
                instance: INSTANCE.to_owned(),
            }),
            stopped: false,
            frame_active: true,
            frame_dialog: false,
            frame_reads: Cell::new(0),
            needs_keyboard: false,
            attach_fails: false,
            attached: Cell::new(false),
        }
    }

    /// A desktop whose seat has no keyboard, so the frame reports itself
    /// active only once one is attached.
    fn keyboardless() -> Self {
        Self {
            needs_keyboard: true,
            ..Self::new()
        }
    }
}

impl ToplevelFocusWorld for World {
    fn lookup(&self, _: u32) -> Lookup {
        self.lookup.clone()
    }

    fn stopped(&self, _: u32) -> bool {
        self.stopped
    }

    fn app_names(&self, _: u32) -> Vec<String> {
        vec![
            "gnome-text-editor".to_owned(),
            "org.gnome.TextEditor.desktop".to_owned(),
        ]
    }

    fn frame(&self) -> Result<FrameView, AdapterError> {
        self.frame_reads.set(self.frame_reads.get() + 1);
        let focused = !self.needs_keyboard || self.attached.get();
        Ok(FrameView {
            minimized: None,
            active: Some(self.frame_active && focused),
            modal: None,
            dialog: self.frame_dialog,
        })
    }

    fn app_frame_active(&self) -> bool {
        false
    }

    fn menu_open(&self) -> bool {
        false
    }

    fn arm_wake(&self) {}

    fn wait_for_change(&self, deadline: Deadline) {
        std::thread::sleep(deadline.remaining());
    }

    fn attach_keyboard(&self, _: Deadline) -> Result<(), AdapterError> {
        if self.attach_fails {
            return Err(AdapterError::new(ErrorCode::ActionFailed, "no keyboard"));
        }
        self.attached.set(true);
        Ok(())
    }
}

fn window(title: &str) -> WindowInfo {
    WindowInfo {
        id: window_id::encode(":1.7", "/org/a11y/atspi/accessible/5").expect("an id"),
        title: title.to_owned(),
        app: "gnome-text-editor".to_owned(),
        pid: ProcessId::new(100),
        process_instance: Some(INSTANCE.to_owned()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn budget() -> Deadline {
    Deadline::after(10_000).expect("a deadline builds")
}

fn desktop(answer: Answer) -> FakeToplevels {
    let mut calculator = toplevel(7, "Calculator", "org.gnome.Calculator");
    calculator.flags.activated = true;
    FakeToplevels::new(
        vec![calculator, toplevel(9, "Notes", "org.gnome.TextEditor")],
        answer,
    )
}

fn focus(world: &World, session: &mut FakeToplevels, title: &str) -> Result<(), AdapterError> {
    run(world, || Ok(session), &window(title), budget())
}

fn cause_of(error: &AdapterError) -> &str {
    error.details.as_ref().expect("details")["cause"]
        .as_str()
        .expect("a cause")
}

#[test]
fn activation_is_verified_by_the_compositor_and_the_frame() {
    let mut session = desktop(Answer::Obey);
    focus(&World::new(), &mut session, "Notes").expect("the window is focused");
    assert_eq!(session.sent, vec![(9, ToplevelRequest::Activate)]);
    assert_eq!(session.settles, 1);
    assert!(session.toplevels[1].flags.activated);
}

#[test]
fn an_already_active_window_sends_nothing() {
    let mut session = desktop(Answer::Obey);
    session.toplevels[1].flags.activated = true;
    focus(&World::new(), &mut session, "Notes").expect("already focused");
    assert!(session.sent.is_empty());
}

#[test]
fn a_minimized_window_is_activated_and_shown() {
    let mut session = desktop(Answer::Obey);
    session.toplevels[1].flags.minimized = true;
    session.toplevels[1].flags.activated = true;
    focus(&World::new(), &mut session, "Notes").expect("the window is shown and focused");
    assert_eq!(session.sent, vec![(9, ToplevelRequest::Activate)]);
    assert!(!session.toplevels[1].flags.minimized);
}

#[test]
fn an_ignored_activation_with_another_window_active_is_refused_after_delivery() {
    let mut session = desktop(Answer::Ignore);
    let error = focus(&World::new(), &mut session, "Notes").expect_err("not granted");
    assert_eq!(cause_of(&error), "activation_refused");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(session.sent.len(), 1);
}

#[test]
fn an_ignored_activation_with_nothing_active_is_not_granted() {
    let mut session = desktop(Answer::Ignore);
    session.toplevels[0].flags.activated = false;
    let error = focus(&World::new(), &mut session, "Notes").expect_err("not granted");
    assert_eq!(cause_of(&error), "activation_not_granted");
}

#[test]
fn a_frame_that_never_reports_active_times_out_unverified() {
    let world = World {
        frame_active: false,
        ..World::new()
    };
    let mut session = desktop(Answer::Obey);
    let short = Deadline::after(200).expect("a deadline builds");
    let error = run(&world, || Ok(&mut session), &window("Notes"), short)
        .expect_err("the frame stays inactive");
    assert_eq!(cause_of(&error), "frame_not_active");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(world.frame_reads.get() > 1);
}

#[test]
fn a_dialog_the_compositor_activated_needs_no_active_frame() {
    let world = World {
        frame_active: false,
        frame_dialog: true,
        ..World::new()
    };
    let mut session = desktop(Answer::Obey);
    focus(&world, &mut session, "Notes").expect("the dialog is focused");
}

#[test]
fn twins_of_one_app_and_title_are_refused_before_any_request() {
    let mut session = desktop(Answer::Obey);
    session
        .toplevels
        .push(toplevel(11, "Notes", "org.gnome.TextEditor"));
    let error = focus(&World::new(), &mut session, "Notes").expect_err("ambiguous");
    assert_eq!(cause_of(&error), "ambiguous_window");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(session.sent.is_empty());
}

#[test]
fn a_shared_title_is_resolved_by_the_application() {
    let mut session = desktop(Answer::Obey);
    session
        .toplevels
        .push(toplevel(11, "Notes", "org.example.Notes"));
    focus(&World::new(), &mut session, "Notes").expect("the editor's window is focused");
    assert_eq!(session.sent, vec![(9, ToplevelRequest::Activate)]);
}

#[test]
fn a_window_without_a_toplevel_is_refused_before_any_request() {
    let mut session = desktop(Answer::Obey);
    let error = focus(&World::new(), &mut session, "Elsewhere").expect_err("missing");
    assert_eq!(cause_of(&error), "window_not_mapped");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(session.sent.is_empty());
}

#[test]
fn another_process_instance_is_stale_before_the_compositor_is_asked() {
    let world = World {
        lookup: Lookup::Gone,
        ..World::new()
    };
    let opened = Cell::new(false);
    let error = run(
        &world,
        || {
            opened.set(true);
            Ok(desktop(Answer::Obey))
        },
        &window("Notes"),
        budget(),
    )
    .expect_err("stale");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(!opened.get());
}

#[test]
fn a_stopped_process_is_unresponsive() {
    let world = World {
        stopped: true,
        ..World::new()
    };
    let mut session = desktop(Answer::Obey);
    let error = focus(&world, &mut session, "Notes").expect_err("stopped");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(session.sent.is_empty());
}

#[test]
fn a_request_that_could_not_be_flushed_is_uncertain() {
    let mut session = desktop(Answer::Obey);
    session.send_error = Some(AdapterError::new(ErrorCode::ActionFailed, "broken pipe"));
    let error = focus(&World::new(), &mut session, "Notes").expect_err("not sent");
    assert_eq!(cause_of(&error), "activation_not_sent");
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn a_window_that_closes_while_activating_is_not_found() {
    let mut session = desktop(Answer::Vanish);
    let error = focus(&World::new(), &mut session, "Notes").expect_err("closed");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn without_a_keyboard_one_is_attached_and_the_frame_still_verifies_the_focus() {
    let world = World::keyboardless();
    let mut session = desktop(Answer::Obey);
    session.keyboard = false;
    focus(&world, &mut session, "Notes").expect("focused once the seat has a keyboard");
    assert_eq!(session.sent, vec![(9, ToplevelRequest::Activate)]);
    assert!(world.attached.get());
}

#[test]
fn without_a_keyboard_an_activated_window_needs_only_the_keyboard() {
    let world = World::keyboardless();
    let mut session = desktop(Answer::Obey);
    session.keyboard = false;
    session.toplevels[1].flags.activated = true;
    focus(&world, &mut session, "Notes").expect("already activated");
    assert!(session.sent.is_empty());
    assert!(world.attached.get());
}

#[test]
fn without_a_keyboard_an_ignored_activation_is_refused_before_attaching_one() {
    let world = World::keyboardless();
    let mut session = desktop(Answer::Ignore);
    session.keyboard = false;
    let error = focus(&world, &mut session, "Notes").expect_err("not granted");
    assert_eq!(cause_of(&error), "activation_refused");
    assert!(!world.attached.get());
}

#[test]
fn a_keyboard_that_cannot_be_attached_leaves_the_frame_unverified() {
    let world = World {
        attach_fails: true,
        ..World::keyboardless()
    };
    let mut session = desktop(Answer::Obey);
    session.keyboard = false;
    let error = focus(&world, &mut session, "Notes").expect_err("no keyboard focus");
    assert_eq!(cause_of(&error), "frame_not_active");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.platform_detail.as_deref(), Some("no keyboard"));
}

#[test]
fn a_seat_with_a_keyboard_never_gets_another() {
    let world = World::new();
    let mut session = desktop(Answer::Obey);
    focus(&world, &mut session, "Notes").expect("focused");
    assert!(!world.attached.get());
}
