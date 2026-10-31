use std::cell::{Cell, RefCell};
use std::time::Instant;

use agent_desktop_core::{
    AdapterError, AppInfo, DeliverySemantics, InteractionLease, OperationReport,
};

use super::SharedBus;
use crate::system::process::process_lookup::Lookup;
use crate::system::window::window_ops::windows_of_process;
use crate::system::wlroots::foreign_toplevel::ForeignToplevels;
use crate::system::wlroots::toplevel_match::app_names;
use toplevel_world::ToplevelWorld;
use world::{Live, World};

#[cfg(test)]
mod fake;
mod force;
mod graceful;
mod refusal;
mod standing;
mod toplevel_world;
mod world;

/// What the caller asked to close: the listed application and whether to end
/// it without asking.
struct Request<'a> {
    name: &'a str,
    pid: u32,
    instance: Option<&'a str>,
    force: bool,
}

fn refuse_if_protected(world: &impl World, request: &Request<'_>) -> Result<(), AdapterError> {
    if world.is_protected()? {
        return Err(refusal::protected(request.name));
    }
    match world.facts() {
        Lookup::Unreadable => Err(refusal::unreadable(request)),
        Lookup::Found(_) | Lookup::Gone => Ok(()),
    }
}

/// Close requires an instance token and proven session ownership before any send.
/// A process that is gone or replaced is left untouched.
/// The result is true only if a close request or signal was delivered.
fn run(world: &impl World, request: &Request<'_>) -> Result<bool, AdapterError> {
    if request.instance.is_none() {
        return Err(refusal::missing_token(request));
    }
    if world.liveness() == crate::system::process::liveness::Liveness::Gone {
        return Ok(false);
    }
    refuse_if_protected(world, request)?;
    if request.force {
        force::close(world, request)
    } else {
        graceful::close(world, request)
    }
}

fn run_report(
    world: &impl World,
    app: &AppInfo,
    force: bool,
) -> Result<OperationReport, AdapterError> {
    let request = Request {
        name: &app.name,
        pid: app.pid.get(),
        instance: app.process_instance.as_deref(),
        force,
    };
    OperationReport::from_close_dispatch(app, run(world, &request))
}

pub(crate) fn close_app(
    bus: &SharedBus,
    app: &AppInfo,
    force: bool,
    lease: &InteractionLease,
) -> Result<OperationReport, AdapterError> {
    let pid = app.pid.get();
    let deadline = lease.deadline();
    let live = Live {
        bus,
        pid,
        instance: app.process_instance.as_deref().unwrap_or_default(),
        deadline,
        started: Instant::now(),
    };
    if force || !crate::system::display::route::toplevel_control() {
        return run_report(&live, app, force);
    }
    let session = match ForeignToplevels::open(deadline) {
        Ok(session) => session,
        Err(error) => {
            let error = error.with_disposition(DeliverySemantics::not_delivered());
            return OperationReport::from_close_dispatch(app, Err(error));
        }
    };
    let frames = || {
        let windows = windows_of_process(bus, pid, deadline).ok()?;
        Some(windows.into_iter().map(|window| window.title).collect())
    };
    let world = ToplevelWorld {
        inner: live,
        session: RefCell::new(session),
        names: app_names(pid),
        frames,
        deadline,
        ambiguous: Cell::new(None),
    };
    run_report(&world, app, force)
}

#[cfg(test)]
#[path = "close_tests.rs"]
mod tests;

#[cfg(test)]
pub(super) mod close_live_tests;
pub(super) mod close_plan;
