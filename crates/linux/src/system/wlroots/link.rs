//! One client connection to the session's Wayland compositor. Every read is
//! bounded by the caller's deadline: the socket is polled for the remaining
//! time and a compositor that stops answering ends in `TIMEOUT`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};

/// The flag a `wl_display.sync` callback raises once the compositor has
/// handled every request sent before it.
pub(crate) type SyncFlag = Arc<AtomicBool>;

/// One global the compositor advertises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Global {
    pub(crate) name: u32,
    pub(crate) interface: String,
    pub(crate) version: u32,
}

/// A state that records the registry's globals.
pub(crate) trait GlobalSink {
    fn globals(&mut self) -> &mut Vec<Global>;
}

/// Dispatches `wl_callback` and `wl_registry` events for any state that
/// delegates them here.
pub(crate) struct CoreHandler;

impl<D> Dispatch<WlCallback, SyncFlag, D> for CoreHandler
where
    D: Dispatch<WlCallback, SyncFlag>,
{
    fn event(
        _: &mut D,
        _: &WlCallback,
        event: wl_callback::Event,
        flag: &SyncFlag,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

impl<D> Dispatch<WlRegistry, (), D> for CoreHandler
where
    D: Dispatch<WlRegistry, ()> + GlobalSink,
{
    fn event(
        state: &mut D,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => state.globals().push(Global {
                name,
                interface,
                version,
            }),
            wl_registry::Event::GlobalRemove { name } => {
                state.globals().retain(|global| global.name != name);
            }
            _ => {}
        }
    }
}

/// A connection and the one event queue its requests use.
pub(crate) struct Link<D> {
    pub(crate) connection: Connection,
    pub(crate) queue: EventQueue<D>,
    pub(crate) handle: QueueHandle<D>,
}

pub(crate) fn connect_failed(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The Wayland compositor could not be reached",
    )
    .with_suggestion("Run the command inside the Wayland session whose WAYLAND_DISPLAY it names")
    .with_platform_detail(detail.to_string())
}

pub(crate) fn protocol_failed(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The Wayland compositor rejected a request or closed the connection",
    )
    .with_suggestion("Retry once the compositor answers; run 'status' to check the session")
    .with_platform_detail(detail.to_string())
}

fn timespec(remaining: Duration) -> Timespec {
    Timespec {
        tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(remaining.subsec_nanos()),
    }
}

impl<D: 'static> Link<D> {
    /// Connects to the compositor named by `WAYLAND_DISPLAY`.
    pub(crate) fn open() -> Result<Self, AdapterError> {
        let connection = Connection::connect_to_env().map_err(connect_failed)?;
        let queue = connection.new_event_queue();
        let handle = queue.handle();
        Ok(Self {
            connection,
            queue,
            handle,
        })
    }

    /// Dispatches events until `done` holds, waiting on the socket for at
    /// most the deadline's remaining time.
    pub(crate) fn dispatch_until(
        &mut self,
        state: &mut D,
        deadline: Deadline,
        mut done: impl FnMut(&D) -> bool,
    ) -> Result<(), AdapterError> {
        loop {
            self.queue
                .dispatch_pending(state)
                .map_err(protocol_failed)?;
            if done(state) {
                return Ok(());
            }
            match self.queue.flush() {
                Ok(()) | Err(WaylandError::Io(_)) => {}
                Err(error) => return Err(protocol_failed(error)),
            }
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };
            let remaining = deadline.remaining();
            if remaining.is_zero() {
                return Err(deadline.timeout_error());
            }
            let readable = {
                let fd = guard.connection_fd();
                let mut fds = [PollFd::new(&fd, PollFlags::IN)];
                match poll(&mut fds, Some(&timespec(remaining))) {
                    Ok(ready) => ready > 0,
                    Err(rustix::io::Errno::INTR) => false,
                    Err(error) => return Err(protocol_failed(error)),
                }
            };
            if readable {
                match guard.read() {
                    Ok(_) => {}
                    Err(WaylandError::Io(error))
                        if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => return Err(protocol_failed(error)),
                }
            } else if deadline.is_expired() {
                return Err(deadline.timeout_error());
            }
        }
    }

    /// Sends every queued request and waits until the compositor has handled
    /// them and the events they caused have been dispatched.
    pub(crate) fn roundtrip(
        &mut self,
        state: &mut D,
        deadline: Deadline,
    ) -> Result<(), AdapterError>
    where
        D: Dispatch<WlCallback, SyncFlag>,
    {
        let flag = SyncFlag::default();
        self.connection
            .display()
            .sync(&self.handle, Arc::clone(&flag));
        self.dispatch_until(state, deadline, |_| flag.load(Ordering::SeqCst))
    }

    /// Reads the compositor's globals with one round trip.
    pub(crate) fn registry(
        &mut self,
        state: &mut D,
        deadline: Deadline,
    ) -> Result<WlRegistry, AdapterError>
    where
        D: Dispatch<WlRegistry, ()> + Dispatch<WlCallback, SyncFlag> + GlobalSink,
    {
        let registry = self.connection.display().get_registry(&self.handle, ());
        self.roundtrip(state, deadline)?;
        Ok(registry)
    }
}

/// Binds the advertised global `interface` at no more than `newest`, or
/// answers `None` when the compositor does not advertise it.
pub(crate) fn bind<I, U, D>(
    registry: &WlRegistry,
    globals: &[Global],
    newest: u32,
    handle: &QueueHandle<D>,
    data: U,
) -> Option<I>
where
    I: Proxy + 'static,
    U: Send + Sync + 'static,
    D: Dispatch<I, U> + 'static,
{
    let global = globals
        .iter()
        .find(|global| global.interface == I::interface().name)?;
    Some(registry.bind::<I, U, D>(global.name, global.version.min(newest), handle, data))
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
