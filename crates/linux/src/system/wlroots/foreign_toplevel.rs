//! The compositor's toplevels as `zwlr_foreign_toplevel_manager_v1` lists
//! them, and the requests it carries to one toplevel: activate, close,
//! minimize and maximize. Each toplevel's state is read as of its latest
//! `done`, so a request is confirmed only by what the compositor applied.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};

use super::link::{CoreHandler, Global, GlobalSink, Link, SyncFlag, bind, protocol_failed};
use super::output::Output;
use super::outputs::{OutputHandler, OutputSink, read_outputs};
use super::toplevel_flags::ToplevelFlags;
use super::toplevel_request::ToplevelRequest;
use super::toplevel_session::ToplevelSession;
use super::toplevel_state::ToplevelState;

const MANAGER_NEWEST: u32 = 3;
const SEAT_NEWEST: u32 = 1;

/// One handle with the state being built from its events and the state of
/// its latest `done`.
struct Entry {
    handle: ZwlrForeignToplevelHandleV1,
    pending: ToplevelState,
    current: Option<ToplevelState>,
    closed: bool,
}

#[derive(Default)]
pub(crate) struct ControlState {
    globals: Vec<Global>,
    outputs: Vec<Output>,
    output_names: Vec<(ObjectId, String)>,
    entries: Vec<Entry>,
    keyboard: bool,
    finished: bool,
}

impl GlobalSink for ControlState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

impl OutputSink for ControlState {
    fn outputs(&mut self) -> &mut Vec<Output> {
        &mut self.outputs
    }
}

wayland_client::delegate_dispatch!(ControlState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(ControlState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_dispatch!(ControlState: [WlOutput: usize] => OutputHandler);
wayland_client::delegate_dispatch!(ControlState: [ZxdgOutputV1: usize] => OutputHandler);
wayland_client::delegate_noop!(ControlState: ignore ZxdgOutputManagerV1);

impl Dispatch<WlSeat, ()> for ControlState {
    fn event(
        state: &mut Self,
        _: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            state.keyboard = capabilities.contains(Capability::Keyboard);
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for ControlState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                let pending = ToplevelState {
                    key: toplevel.id().protocol_id(),
                    ..ToplevelState::default()
                };
                state.entries.push(Entry {
                    handle: toplevel,
                    pending,
                    current: None,
                    closed: false,
                });
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    event_created_child!(ControlState, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for ControlState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let names = &state.output_names;
        let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.handle.id() == handle.id())
        else {
            return;
        };
        let pending = &mut entry.pending;
        let name_of = |output: &WlOutput| {
            names
                .iter()
                .find(|(id, _)| *id == output.id())
                .map(|(_, name)| name.clone())
        };
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => pending.title = title,
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => pending.app_id = app_id,
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                pending.flags = ToplevelFlags::from_state_array(&state);
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { output } => {
                if let Some(name) = name_of(&output)
                    && !pending.outputs.contains(&name)
                {
                    pending.outputs.push(name);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { output } => {
                if let Some(name) = name_of(&output) {
                    pending.outputs.retain(|known| *known != name);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Parent { parent } => {
                pending.has_parent = parent.is_some();
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                entry.current = Some(entry.pending.clone());
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => entry.closed = true,
            _ => {}
        }
    }
}

fn unsupported(what: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        format!("The compositor offers no {what}"),
    )
    .with_suggestion("Operate the window yourself; this compositor gives clients no window control")
}

/// One connection that lists the toplevels and carries requests to them.
pub(crate) struct ForeignToplevels {
    link: Link<ControlState>,
    state: ControlState,
    manager: ZwlrForeignToplevelManagerV1,
    seat: WlSeat,
    outputs: Vec<Output>,
}

impl ForeignToplevels {
    pub(crate) fn open(deadline: Deadline) -> Result<Self, AdapterError> {
        let mut link = Link::<ControlState>::open()?;
        let mut state = ControlState::default();
        let registry = link.registry(&mut state, deadline)?;
        let globals = state.globals.clone();
        let outputs = read_outputs(&mut link, &mut state, &registry, &globals, deadline)?;
        state.output_names = outputs
            .iter()
            .map(|(handle, output)| (handle.id(), output.name.clone()))
            .collect();
        let outputs = outputs.into_iter().map(|(_, output)| output).collect();
        let seat: WlSeat = bind(&registry, &globals, SEAT_NEWEST, &link.handle, ())
            .ok_or_else(|| unsupported("wl_seat"))?;
        let manager: ZwlrForeignToplevelManagerV1 =
            bind(&registry, &globals, MANAGER_NEWEST, &link.handle, ())
                .ok_or_else(|| unsupported("zwlr_foreign_toplevel_manager_v1"))?;
        link.roundtrip(&mut state, deadline)?;
        link.roundtrip(&mut state, deadline)?;
        Ok(Self {
            link,
            state,
            manager,
            seat,
            outputs,
        })
    }

    /// The outputs as the connection read them when it opened.
    pub(crate) fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    fn entry(&self, key: u32) -> Option<&Entry> {
        self.state
            .entries
            .iter()
            .find(|entry| entry.pending.key == key)
    }
}

fn gone() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window closed before the request reached it",
    )
    .with_suggestion("Run 'list-windows' to find the window again")
}

impl ToplevelSession for ForeignToplevels {
    fn toplevels(&self) -> Vec<ToplevelState> {
        self.state
            .entries
            .iter()
            .filter(|entry| !entry.closed)
            .filter_map(|entry| entry.current.clone())
            .collect()
    }

    fn send(&mut self, key: u32, request: ToplevelRequest) -> Result<(), AdapterError> {
        let entry = self
            .entry(key)
            .filter(|entry| !entry.closed)
            .ok_or_else(gone)?;
        let handle = &entry.handle;
        match request {
            ToplevelRequest::Activate => handle.activate(&self.seat),
            ToplevelRequest::Close => handle.close(),
            ToplevelRequest::SetMinimized(true) => handle.set_minimized(),
            ToplevelRequest::SetMinimized(false) => handle.unset_minimized(),
            ToplevelRequest::SetMaximized(true) => handle.set_maximized(),
            ToplevelRequest::SetMaximized(false) => handle.unset_maximized(),
        }
        self.link.connection.flush().map_err(protocol_failed)
    }

    fn settle(
        &mut self,
        key: u32,
        wanted: &dyn Fn(&ToplevelState) -> bool,
        deadline: Deadline,
    ) -> Result<Option<ToplevelState>, AdapterError> {
        let reached = |state: &ControlState| {
            state.finished
                || state
                    .entries
                    .iter()
                    .find(|entry| entry.pending.key == key)
                    .is_none_or(|entry| entry.closed || entry.current.as_ref().is_some_and(wanted))
        };
        match self.link.dispatch_until(&mut self.state, deadline, reached) {
            Ok(()) => {}
            Err(_) if deadline.is_expired() => {}
            Err(error) => return Err(error),
        }
        Ok(self
            .entry(key)
            .filter(|entry| !entry.closed)
            .and_then(|entry| entry.current.clone()))
    }

    fn refresh(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        self.link.roundtrip(&mut self.state, deadline)
    }

    fn seat_has_keyboard(&self) -> bool {
        self.state.keyboard
    }
}

impl Drop for ForeignToplevels {
    fn drop(&mut self) {
        for entry in &self.state.entries {
            entry.handle.destroy();
        }
        if !self.state.finished {
            self.manager.stop();
        }
        let _ = self.link.connection.flush();
    }
}
