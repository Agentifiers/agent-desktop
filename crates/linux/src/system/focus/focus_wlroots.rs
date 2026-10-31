//! Focus on a wlroots compositor: the window's toplevel is found by title and
//! application among those `zwlr_foreign_toplevel_manager_v1` lists, asked to
//! activate, and the focus counts only once the compositor reports the
//! toplevel activated and the window's own frame reports itself active. A
//! seat without a keyboard gives no client keyboard focus, so no frame could
//! report itself active there: the focus attaches a virtual keyboard that
//! sends no key, which the compositor enters into the activated window.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo};

use super::focus_cause::{Cause, refusal};
use super::focus_frame_view::FrameView;
use super::focus_window::{FRAME_REPORT_GRACE, check_owner};
use crate::system::process::process_lookup::Lookup;
use crate::system::window::window_id;
use crate::system::window::window_op::window_op_verdict::POLL;
use crate::system::wlroots::toplevel_match::{ToplevelMatch, WindowIdentity};
use crate::system::wlroots::toplevel_request::ToplevelRequest;
use crate::system::wlroots::toplevel_session::{ToplevelSession, pick};
use crate::system::wlroots::toplevel_state::ToplevelState;
use crate::system::x11::control::activate::ACTIVATION_GRACE;

/// What the wlroots focus reads of the window's process and frame.
pub(in crate::system) trait ToplevelFocusWorld {
    fn lookup(&self, pid: u32) -> Lookup;
    fn stopped(&self, pid: u32) -> bool;
    /// Every name the window's application is known by.
    fn app_names(&self, pid: u32) -> Vec<String>;
    fn frame(&self) -> Result<FrameView, AdapterError>;
    /// Whether any frame of the window's application reports ACTIVE.
    fn app_frame_active(&self) -> bool;
    fn menu_open(&self) -> bool;
    fn arm_wake(&self);
    fn wait_for_change(&self, deadline: Deadline);
    /// Gives the seat a keyboard that sends no key, held while the focus
    /// runs, so the activated window can take keyboard focus.
    fn attach_keyboard(&self, deadline: Deadline) -> Result<(), AdapterError>;
}

/// Whether the window reports itself active once the compositor activated
/// its toplevel. A dialog that never reports active has the compositor's
/// activation as its evidence.
fn frame_reports_active(world: &impl ToplevelFocusWorld) -> bool {
    let reported = world
        .frame()
        .ok()
        .is_some_and(|view| view.active == Some(true) || view.modal == Some(true) || view.dialog);
    reported || world.app_frame_active() || world.menu_open()
}

fn shown_and_active(toplevel: &ToplevelState) -> bool {
    toplevel.flags.activated && !toplevel.flags.minimized
}

/// The toplevel of `win`, or the refusal when none or several match.
pub(in crate::system) fn target_of(
    session: &impl ToplevelSession,
    win: &WindowInfo,
    names: &[String],
) -> Result<ToplevelState, ToplevelMatch> {
    let identity = WindowIdentity {
        title: &win.title,
        app_names: names,
    };
    pick(&session.toplevels(), identity)
}

fn send_failed(error: AdapterError) -> AdapterError {
    if error.code == ErrorCode::WindowNotFound {
        return error.with_disposition(DeliverySemantics::not_delivered());
    }
    let detail = error.message;
    refusal(Cause::ActivationNotSent)
        .with_platform_detail(detail)
        .with_disposition(DeliverySemantics::uncertain())
}

fn closed_meanwhile() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window closed while it was being activated",
    )
    .with_suggestion("Run 'list-windows' to find the window again")
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn wait_for_frame(world: &impl ToplevelFocusWorld, deadline: Deadline) -> Result<(), AdapterError> {
    let frame_deadline = deadline.capped(FRAME_REPORT_GRACE);
    loop {
        if frame_reports_active(world) {
            return Ok(());
        }
        if frame_deadline.is_expired() {
            return Err(refusal(Cause::FrameNotActive)
                .with_disposition(DeliverySemantics::delivered_unverified()));
        }
        world.wait_for_change(frame_deadline.capped(POLL));
    }
}

/// Asks the compositor to activate `target` and waits until it reports the
/// toplevel activated and shown.
fn activate(
    session: &mut impl ToplevelSession,
    target: &ToplevelState,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    session
        .send(target.key, ToplevelRequest::Activate)
        .map_err(send_failed)?;
    let granted = session
        .settle(
            target.key,
            &shown_and_active,
            deadline.capped(ACTIVATION_GRACE),
        )
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?
        .ok_or_else(closed_meanwhile)?;
    if shown_and_active(&granted) {
        return Ok(());
    }
    let other_active = session
        .toplevels()
        .iter()
        .any(|toplevel| toplevel.key != target.key && toplevel.flags.activated);
    let cause = if other_active {
        Cause::ActivationRefused
    } else {
        Cause::ActivationNotGranted
    };
    Err(refusal(cause).with_disposition(DeliverySemantics::delivered_unverified()))
}

/// Activates the window's toplevel and returns only once the compositor
/// reports it activated and the window's frame reports itself active.
pub(in crate::system) fn run<S: ToplevelSession>(
    world: &impl ToplevelFocusWorld,
    open: impl FnOnce() -> Result<S, AdapterError>,
    win: &WindowInfo,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let not_delivered =
        |error: AdapterError| error.with_disposition(DeliverySemantics::not_delivered());
    window_id::decode(&win.id).map_err(not_delivered)?;
    check_owner(world.lookup(win.pid.get()), win)?;
    if world.stopped(win.pid.get()) {
        return Err(AdapterError::app_unresponsive(&win.app));
    }
    let mut session = open().map_err(not_delivered)?;
    let names = world.app_names(win.pid.get());
    let target = target_of(&session, win, &names).map_err(|found| {
        refusal(match found {
            ToplevelMatch::Ambiguous(_) => Cause::AmbiguousWindow,
            _ => Cause::WindowNotMapped,
        })
    })?;
    let keyboard = session.seat_has_keyboard();
    let activated = shown_and_active(&target);
    if activated && keyboard && frame_reports_active(world) {
        return Ok(());
    }
    world.arm_wake();
    let sent = !activated || keyboard;
    if sent {
        activate(&mut session, &target, deadline)?;
    }
    if !keyboard {
        world.attach_keyboard(deadline).map_err(|error| {
            let disposition = if sent {
                DeliverySemantics::delivered_unverified()
            } else {
                DeliverySemantics::not_delivered()
            };
            refusal(Cause::FrameNotActive)
                .with_platform_detail(error.message)
                .with_disposition(disposition)
        })?;
    }
    wait_for_frame(world, deadline)
}

#[cfg(test)]
#[path = "focus_wlroots_tests.rs"]
mod tests;
