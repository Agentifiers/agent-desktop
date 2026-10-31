use super::*;
use crate::system::wlroots::toplevel_flags::ToplevelFlags;

/// How the fake compositor answers a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    /// Applies every request as a wlroots compositor does.
    Obey,
    /// Leaves every toplevel as it was.
    Ignore,
    /// Closes the toplevel a request names, whatever the request.
    Vanish,
}

/// A compositor with fixed toplevels that records every request.
pub(crate) struct FakeToplevels {
    pub(crate) toplevels: Vec<ToplevelState>,
    pub(crate) answer: Answer,
    pub(crate) sent: Vec<(u32, ToplevelRequest)>,
    pub(crate) send_error: Option<AdapterError>,
    pub(crate) settles: usize,
    pub(crate) keyboard: bool,
}

pub(crate) fn toplevel(key: u32, title: &str, app_id: &str) -> ToplevelState {
    ToplevelState {
        key,
        title: title.to_owned(),
        app_id: app_id.to_owned(),
        flags: ToplevelFlags::default(),
        outputs: vec!["HEADLESS-1".to_owned()],
        has_parent: false,
    }
}

impl FakeToplevels {
    pub(crate) fn new(toplevels: Vec<ToplevelState>, answer: Answer) -> Self {
        Self {
            toplevels,
            answer,
            sent: Vec::new(),
            send_error: None,
            settles: 0,
            keyboard: true,
        }
    }

    fn apply(&mut self, key: u32, request: ToplevelRequest) {
        if request == ToplevelRequest::Close {
            self.toplevels.retain(|toplevel| toplevel.key != key);
            return;
        }
        for toplevel in &mut self.toplevels {
            let flags = &mut toplevel.flags;
            let target = toplevel.key == key;
            match request {
                ToplevelRequest::Activate => {
                    flags.activated = target;
                    if target {
                        flags.minimized = false;
                    }
                }
                ToplevelRequest::SetMinimized(on) if target => {
                    flags.minimized = on;
                    flags.activated &= !on;
                }
                ToplevelRequest::SetMaximized(on) if target => flags.maximized = on,
                _ => {}
            }
        }
    }
}

impl ToplevelSession for FakeToplevels {
    fn toplevels(&self) -> Vec<ToplevelState> {
        self.toplevels.clone()
    }

    fn send(&mut self, key: u32, request: ToplevelRequest) -> Result<(), AdapterError> {
        if let Some(error) = self.send_error.clone() {
            return Err(error);
        }
        self.sent.push((key, request));
        match self.answer {
            Answer::Obey => self.apply(key, request),
            Answer::Vanish => self.apply(key, ToplevelRequest::Close),
            Answer::Ignore => {}
        }
        Ok(())
    }

    fn settle(
        &mut self,
        key: u32,
        _wanted: &dyn Fn(&ToplevelState) -> bool,
        _deadline: Deadline,
    ) -> Result<Option<ToplevelState>, AdapterError> {
        self.settles += 1;
        Ok(self
            .toplevels
            .iter()
            .find(|toplevel| toplevel.key == key)
            .cloned())
    }

    fn refresh(&mut self, _deadline: Deadline) -> Result<(), AdapterError> {
        Ok(())
    }

    fn seat_has_keyboard(&self) -> bool {
        self.keyboard
    }
}
