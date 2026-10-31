//! The helper's connection to the compositor and the markers drawn on it.
//! A marker lives on the output that holds its point and is moved there by
//! margins, so a move costs one commit and damages two marker-sized boxes.

use std::collections::BTreeMap;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use agent_desktop_core::{AdapterError, CursorOverlayStyle, Deadline, ErrorCode, Point};
use wayland_client::Proxy;
use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::WlRegistry;

use super::bound::Bound;
use super::canvas::{AgentKey, Canvas};
use super::marker_surface::MarkerSurface;
use super::painted::Painted;
use super::stage_state::StageState;
use crate::system::wlroots::link::{Link, protocol_failed};
use crate::system::wlroots::output::Output;
use crate::system::wlroots::outputs::read_outputs;

const STEP_MS: u64 = 2_000;

pub(crate) struct Stage {
    link: Link<StageState>,
    state: StageState,
    registry: WlRegistry,
    bound: Bound,
    outputs: Vec<(u32, WlOutput, Output)>,
    markers: BTreeMap<AgentKey, MarkerSurface>,
}

fn step() -> Result<Deadline, AdapterError> {
    Deadline::detached_after(STEP_MS)
}

fn output_globals(state: &StageState) -> Vec<u32> {
    state
        .globals
        .iter()
        .filter(|global| global.interface == "wl_output")
        .map(|global| global.name)
        .collect()
}

/// The index of the output whose layout rectangle holds `point`, and the
/// point's offset from that output's top-left corner.
pub(crate) fn locate(outputs: &[Output], point: &Point) -> Option<(usize, (i32, i32))> {
    let (x, y) = (point.x.round(), point.y.round());
    outputs.iter().enumerate().find_map(|(index, output)| {
        let rect = output.rect();
        let inside =
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height;
        inside.then_some((index, ((x - rect.x) as i32, (y - rect.y) as i32)))
    })
}

impl Stage {
    /// Connects to the compositor and binds what a marker needs.
    pub(crate) fn open() -> Result<Self, AdapterError> {
        let deadline = step()?;
        let mut link = Link::<StageState>::open()?;
        let mut state = StageState::default();
        let registry = link.registry(&mut state, deadline)?;
        let globals = state.globals.clone();
        let bound = Bound::bind(&registry, &globals, &link.handle).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::PlatformNotSupported,
                "The compositor offers no layer shell to draw the cursor overlay on",
            )
        })?;
        let mut stage = Self {
            link,
            state,
            registry,
            bound,
            outputs: Vec::new(),
            markers: BTreeMap::new(),
        };
        stage.read_outputs(deadline)?;
        Ok(stage)
    }

    fn read_outputs(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        for (_, handle, _) in self.outputs.drain(..) {
            if handle.version() >= 3 {
                handle.release();
            }
        }
        self.state.outputs.clear();
        let globals = self.state.globals.clone();
        let names = output_globals(&self.state);
        let read = read_outputs(
            &mut self.link,
            &mut self.state,
            &self.registry,
            &globals,
            deadline,
        )?;
        self.outputs = names
            .into_iter()
            .zip(read)
            .map(|(name, (handle, output))| (name, handle, output))
            .collect();
        let known: Vec<String> = self.outputs.iter().map(|o| o.2.name.clone()).collect();
        self.markers
            .retain(|_, marker| known.contains(&marker.output));
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), AdapterError> {
        let closed = std::mem::take(&mut self.state.closed);
        self.markers
            .retain(|_, marker| !closed.contains(&marker.id));
        let current: Vec<u32> = self.outputs.iter().map(|o| o.0).collect();
        if output_globals(&self.state) != current {
            self.read_outputs(step()?)?;
        }
        Ok(())
    }

    fn wait_configured(&mut self, id: u32) -> Result<(), AdapterError> {
        self.link
            .dispatch_until(&mut self.state, step()?, |state| {
                state.configured.contains(&id) || state.closed.contains(&id)
            })?;
        if self.state.closed.contains(&id) {
            return Err(protocol_failed("the compositor closed the cursor marker"));
        }
        Ok(())
    }

    /// Sends queued requests and dispatches queued events without waiting.
    pub(crate) fn pump(&mut self) -> Result<(), AdapterError> {
        self.link
            .queue
            .dispatch_pending(&mut self.state)
            .map_err(protocol_failed)?;
        match self.link.queue.flush() {
            Ok(()) | Err(WaylandError::Io(_)) => Ok(()),
            Err(error) => Err(protocol_failed(error)),
        }
    }

    /// Waits up to `timeout` for compositor events or for one of `others` to
    /// become readable, reads the events, and answers which of `others` are
    /// ready. A compositor that hung up is an error.
    pub(crate) fn wait(
        &mut self,
        others: &[BorrowedFd<'_>],
        timeout: Duration,
    ) -> Result<Vec<bool>, AdapterError> {
        self.pump()?;
        let Some(guard) = self.link.queue.prepare_read() else {
            return Ok(vec![false; others.len()]);
        };
        let ready = super::poll::readable(guard.connection_fd().as_fd(), others, timeout)?;
        if ready.0 {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(protocol_failed(error)),
            }
        }
        self.pump()?;
        Ok(ready.1)
    }
}

impl Canvas for Stage {
    fn show(
        &mut self,
        agent: &AgentKey,
        point: &Point,
        style: &CursorOverlayStyle,
    ) -> Result<(), AdapterError> {
        self.refresh()?;
        let outputs: Vec<Output> = self.outputs.iter().map(|o| o.2.clone()).collect();
        let Some((index, margin)) = locate(&outputs, point) else {
            return self.hide(agent);
        };
        let (_, handle, output) = &self.outputs[index];
        if self
            .markers
            .get(agent)
            .is_some_and(|marker| marker.output != output.name)
        {
            self.markers.remove(agent);
        }
        if !self.markers.contains_key(agent) {
            self.state.next_id = self.state.next_id.wrapping_add(1);
            let marker = MarkerSurface::create(
                self.state.next_id,
                &self.bound,
                (handle, &output.name),
                &self.link.handle,
            );
            self.markers.insert(agent.clone(), marker);
        }
        let Some(marker) = self.markers.get_mut(agent) else {
            return Ok(());
        };
        let repainted = marker
            .painted
            .as_ref()
            .is_none_or(|painted| painted.style != *style);
        let retired = if repainted {
            marker
                .painted
                .replace(Painted::draw(&self.bound.shm, style, &self.link.handle)?)
        } else {
            None
        };
        if marker.is_mapped() {
            marker.update(margin, repainted);
            drop(retired);
            return Ok(());
        }
        drop(retired);
        let id = marker.id;
        self.state.configured.retain(|configured| *configured != id);
        marker.request_map(&self.bound, margin, &self.link.handle);
        self.wait_configured(id)?;
        if let Some(marker) = self.markers.get_mut(agent) {
            marker.finish_map();
        }
        Ok(())
    }

    fn hide(&mut self, agent: &AgentKey) -> Result<(), AdapterError> {
        if let Some(marker) = self.markers.get_mut(agent) {
            marker.unmap();
        }
        Ok(())
    }

    fn settle(&mut self) -> Result<(), AdapterError> {
        self.link.roundtrip(&mut self.state, step()?)
    }
}

#[cfg(test)]
#[path = "stage_tests.rs"]
mod tests;
