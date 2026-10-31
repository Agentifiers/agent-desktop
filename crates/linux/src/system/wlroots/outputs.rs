//! The compositor's outputs in layout coordinates: `wl_output` names and
//! scales, and the logical position and size `zxdg_output_v1` reports.

use agent_desktop_core::{AdapterError, Deadline};
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

use super::link::{CoreHandler, Global, GlobalSink, Link, SyncFlag, bind};
use super::output::Output;

const WL_OUTPUT_NEWEST: u32 = 4;
const XDG_OUTPUT_NEWEST: u32 = 3;

#[derive(Default)]
pub(crate) struct OutputState {
    globals: Vec<Global>,
    outputs: Vec<Output>,
}

impl GlobalSink for OutputState {
    fn globals(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}

wayland_client::delegate_dispatch!(OutputState: [WlCallback: SyncFlag] => CoreHandler);
wayland_client::delegate_dispatch!(OutputState: [WlRegistry: ()] => CoreHandler);
wayland_client::delegate_dispatch!(OutputState: [WlOutput: usize] => OutputHandler);
wayland_client::delegate_dispatch!(OutputState: [ZxdgOutputV1: usize] => OutputHandler);
wayland_client::delegate_noop!(OutputState: ignore ZxdgOutputManagerV1);

/// Dispatches `wl_output` and `zxdg_output_v1` events into any
/// [`OutputSink`] state that delegates them here.
pub(crate) struct OutputHandler;

impl<D> Dispatch<WlOutput, usize, D> for OutputHandler
where
    D: Dispatch<WlOutput, usize> + OutputSink,
{
    fn event(
        state: &mut D,
        _: &WlOutput,
        event: wl_output::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        let Some(output) = state.outputs().get_mut(*index) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => output.name = name,
            wl_output::Event::Scale { factor } => output.scale = f64::from(factor),
            wl_output::Event::Mode { width, height, .. } if output.width == 0 => {
                output.width = width;
                output.height = height;
            }
            _ => {}
        }
    }
}

impl<D> Dispatch<ZxdgOutputV1, usize, D> for OutputHandler
where
    D: Dispatch<ZxdgOutputV1, usize> + OutputSink,
{
    fn event(
        state: &mut D,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        let Some(output) = state.outputs().get_mut(*index) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                output.x = x;
                output.y = y;
                output.logical = true;
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                output.width = width;
                output.height = height;
            }
            zxdg_output_v1::Event::Name { name } if output.name.is_empty() => output.name = name,
            _ => {}
        }
    }
}

/// Every output with its logical rectangle, in the order the compositor
/// advertised them, with the handles that name each one in a request.
pub(crate) fn read_outputs<D>(
    link: &mut Link<D>,
    state: &mut D,
    registry: &WlRegistry,
    globals: &[Global],
    deadline: Deadline,
) -> Result<Vec<(WlOutput, Output)>, AdapterError>
where
    D: Dispatch<WlOutput, usize>
        + Dispatch<ZxdgOutputV1, usize>
        + Dispatch<ZxdgOutputManagerV1, ()>
        + Dispatch<WlCallback, SyncFlag>
        + OutputSink
        + 'static,
{
    let handles: Vec<WlOutput> = globals
        .iter()
        .filter(|global| global.interface == "wl_output")
        .enumerate()
        .map(|(index, global)| {
            state.outputs().push(Output::default());
            registry.bind::<WlOutput, usize, D>(
                global.name,
                global.version.min(WL_OUTPUT_NEWEST),
                &link.handle,
                index,
            )
        })
        .collect();
    let manager: Option<ZxdgOutputManagerV1> =
        bind(registry, globals, XDG_OUTPUT_NEWEST, &link.handle, ());
    if let Some(manager) = &manager {
        for (index, handle) in handles.iter().enumerate() {
            manager.get_xdg_output(handle, &link.handle, index);
        }
    }
    link.roundtrip(state, deadline)?;
    link.roundtrip(state, deadline)?;
    let outputs = std::mem::take(state.outputs());
    Ok(handles.into_iter().zip(outputs).collect())
}

/// A state that collects outputs for [`read_outputs`].
pub(crate) trait OutputSink {
    fn outputs(&mut self) -> &mut Vec<Output>;
}

impl OutputSink for OutputState {
    fn outputs(&mut self) -> &mut Vec<Output> {
        &mut self.outputs
    }
}

/// The compositor's outputs over a fresh connection.
pub(crate) fn list(deadline: Deadline) -> Result<Vec<Output>, AdapterError> {
    let mut link = Link::<OutputState>::open()?;
    let mut state = OutputState::default();
    let registry = link.registry(&mut state, deadline)?;
    let globals = state.globals.clone();
    let outputs = read_outputs(&mut link, &mut state, &registry, &globals, deadline)?;
    Ok(outputs.into_iter().map(|(_, output)| output).collect())
}
