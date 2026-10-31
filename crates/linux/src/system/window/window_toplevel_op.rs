//! Window operations on a wlroots compositor through
//! `zwlr_foreign_toplevel_manager_v1`: minimize, maximize and restore are
//! asked of the window's toplevel and confirmed by the state the compositor
//! reports back. No wlroots protocol lets a client place or size another
//! client's toplevel, so a move or a resize is refused before anything is
//! sent; X11 requests to an Xwayland window are not honoured there either.

use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, OperationReport,
    OperationTarget, WindowInfo, WindowOp,
};
use serde_json::{Value, json};

use super::window_op::window_op_verdict::requested;
use super::window_report::window_readback;
use crate::system::focus::focus_wlroots::target_of;
use crate::system::process::process_lookup::Lookup;
use crate::system::process::{proc_stat, process_facts};
use crate::system::wlroots::foreign_toplevel::ForeignToplevels;
use crate::system::wlroots::toplevel_flags::ToplevelFlags;
use crate::system::wlroots::toplevel_match::{ToplevelMatch, app_names};
use crate::system::wlroots::toplevel_request::ToplevelRequest;
use crate::system::wlroots::toplevel_session::ToplevelSession;

/// How long the compositor gets to report the state a request asked for.
const STATE_GRACE: Duration = Duration::from_millis(2_000);

/// What a toplevel window operation reads of the window's process.
pub(in crate::system) trait ToplevelOpWorld {
    fn lookup(&self, pid: u32) -> Lookup;
    fn stopped(&self, pid: u32) -> bool;
    fn app_names(&self, pid: u32) -> Vec<String>;
}

struct Live;

impl ToplevelOpWorld for Live {
    fn lookup(&self, pid: u32) -> Lookup {
        process_facts::lookup(pid)
    }

    fn stopped(&self, pid: u32) -> bool {
        proc_stat::is_stopped(pid)
    }

    fn app_names(&self, pid: u32) -> Vec<String> {
        app_names(pid)
    }
}

fn refused(code: ErrorCode, cause: &str, message: &str, suggestion: &str) -> AdapterError {
    AdapterError::new(code, message)
        .with_suggestion(suggestion)
        .with_details(json!({ "cause": cause, "physical_delivery_started": false }))
        .with_disposition(DeliverySemantics::not_delivered())
}

fn no_geometry_control() -> AdapterError {
    refused(
        ErrorCode::ActionNotSupported,
        "wlroots_no_geometry_control",
        "No wlroots protocol lets a client move or resize another client's window",
        "Move or resize the window yourself; minimize, maximize and restore work on this compositor",
    )
}

fn no_toplevel(found: ToplevelMatch) -> AdapterError {
    match found {
        ToplevelMatch::Ambiguous(count) => AdapterError::ambiguous_target(
            "More than one compositor toplevel matches this window, so none is changed",
        )
        .with_suggestion("Give the windows different titles, then retry")
        .with_details(json!({
            "cause": "ambiguous_window",
            "candidates": count,
            "physical_delivery_started": false,
        }))
        .with_disposition(DeliverySemantics::not_delivered()),
        _ => refused(
            ErrorCode::ActionNotSupported,
            "window_not_mapped",
            "The compositor lists no toplevel for this window, so it cannot be changed from here",
            "Run 'list-windows' to confirm the window is mapped, then retry",
        ),
    }
}

fn stale(message: &str) -> AdapterError {
    AdapterError::stale_ref_because(message).with_disposition(DeliverySemantics::not_delivered())
}

fn same_instance(world: &impl ToplevelOpWorld, win: &WindowInfo) -> Option<bool> {
    match world.lookup(win.pid.get()) {
        Lookup::Found(facts) => Some(win.process_instance.as_deref() == Some(&facts.instance)),
        Lookup::Gone => Some(false),
        Lookup::Unreadable => None,
    }
}

/// The requests that bring a toplevel in state `flags` to what `op` asks,
/// and the state that confirms it. A restore lifts a minimized window and
/// clears maximization, as the X11 restore does.
fn plan(op: &WindowOp, flags: ToplevelFlags) -> (Vec<ToplevelRequest>, fn(&ToplevelFlags) -> bool) {
    match op {
        WindowOp::Minimize => (vec![ToplevelRequest::SetMinimized(true)], |f| f.minimized),
        WindowOp::Maximize => (vec![ToplevelRequest::SetMaximized(true)], |f| f.maximized),
        _ => {
            let mut requests = Vec::new();
            if flags.minimized {
                requests.push(ToplevelRequest::SetMinimized(false));
            }
            if flags.maximized {
                requests.push(ToplevelRequest::SetMaximized(false));
            }
            (requests, |f| !f.minimized && !f.maximized)
        }
    }
}

fn requested_of(op: &WindowOp) -> Value {
    match op {
        WindowOp::Minimize => json!({ "minimized": true }),
        WindowOp::Maximize => json!({ "maximized": true }),
        _ => json!({ "minimized": false, "maximized": false }),
    }
}

fn observed_of(flags: &ToplevelFlags) -> Value {
    json!({
        "minimized": flags.minimized,
        "maximized": flags.maximized,
        "activated": flags.activated,
        "fullscreen": flags.fullscreen,
    })
}

fn mismatch(op: &WindowOp, flags: &ToplevelFlags) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The compositor left the window other than requested",
    )
    .with_suggestion("Run 'list-windows' to see the window as it is now before retrying")
    .with_details(json!({
        "cause": "readback_mismatch",
        "requested": requested_of(op),
        "observed": observed_of(flags),
    }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn window_gone() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window disappeared or its owner changed during the operation",
    )
    .with_suggestion("Run 'list-windows' to find the window again")
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn send_failed(error: AdapterError, sent: bool) -> AdapterError {
    let disposition = if error.code == ErrorCode::WindowNotFound && !sent {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::uncertain()
    };
    error.with_disposition(disposition)
}

fn dispatch<S: ToplevelSession>(
    world: &impl ToplevelOpWorld,
    open: impl FnOnce() -> Result<S, AdapterError>,
    win: &WindowInfo,
    op: &WindowOp,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    requested(op)?;
    if matches!(op, WindowOp::Move { .. } | WindowOp::Resize { .. }) {
        return Err(no_geometry_control());
    }
    if win.process_instance.is_none() {
        return Err(stale(
            "A window operation needs the process instance the window was listed with",
        ));
    }
    if same_instance(world, win) == Some(false) {
        return Err(stale(
            "The window belongs to a different process instance or the process has exited",
        ));
    }
    if world.stopped(win.pid.get()) {
        return Err(AdapterError::app_unresponsive(&win.app)
            .with_disposition(DeliverySemantics::not_delivered()));
    }
    let mut session =
        open().map_err(|error| error.with_disposition(DeliverySemantics::not_delivered()))?;
    let names = world.app_names(win.pid.get());
    let target = target_of(&session, win, &names).map_err(no_toplevel)?;
    let (requests, holds) = plan(op, target.flags);
    if holds(&target.flags) {
        return Ok(());
    }
    for (index, request) in requests.into_iter().enumerate() {
        session
            .send(target.key, request)
            .map_err(|error| send_failed(error, index > 0))?;
    }
    let observed = session
        .settle(
            target.key,
            &|toplevel| holds(&toplevel.flags),
            deadline.capped(STATE_GRACE),
        )
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?
        .ok_or_else(window_gone)?;
    if same_instance(world, win) == Some(false) {
        return Err(window_gone());
    }
    if holds(&observed.flags) {
        Ok(())
    } else {
        Err(mismatch(op, &observed.flags))
    }
}

pub(in crate::system) fn run<S: ToplevelSession>(
    world: &impl ToplevelOpWorld,
    open: impl FnOnce() -> Result<S, AdapterError>,
    win: &WindowInfo,
    op: &WindowOp,
    deadline: Deadline,
) -> Result<OperationReport, AdapterError> {
    let dispatched = dispatch(world, open, win, op, deadline);
    OperationReport::from_window_dispatch(win, op, window_readback(op), dispatched)
}

pub(crate) fn window_op(
    win: &WindowInfo,
    op: WindowOp,
    lease: &InteractionLease,
) -> Result<OperationReport, AdapterError> {
    OperationTarget::for_window(win)?;
    let deadline = lease.deadline();
    run(
        &Live,
        || ForeignToplevels::open(deadline),
        win,
        &op,
        deadline,
    )
}

#[cfg(test)]
#[path = "window_toplevel_op_tests.rs"]
mod tests;
