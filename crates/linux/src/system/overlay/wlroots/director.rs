//! The cursors one session's helper draws, driven the way the Windows
//! renderer drives its one: a travel glides along the shared hand path and is
//! answered only once the cursor has arrived, an effect plays its ripple and
//! highlight after it was answered, a new label eases in once the cursor
//! lands, and after a quiet spell everything fades out and rests until the
//! next control. Controls reach agents the way the X11 overlay routes them.

use std::collections::BTreeMap;
use std::time::Duration;

use agent_desktop_core::{
    AdapterError, CURSOR_IDLE_REST_MS, CURSOR_REST_FADE_MS, CursorMotion, CursorOverlayControl,
    CursorPhase, Point, Rect,
};

use super::canvas::{AgentKey, Canvas};
use super::clock::Clock;
use super::effect::Effect;
use super::look::{label, reveal::Reveal, schedule};
use super::paint::{Light, frame};
use super::presentation::Presentation;
use super::rest::Rest;

const REST_FADE_STEPS: u32 = 13;
const REFRESH_HZ: u32 = 60;
const NO_SCREEN: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1280.0,
    height: 720.0,
};
const UNKNOWN_START: (f64, f64) = (-180.0, 108.0);

pub(crate) struct Director {
    cursors: BTreeMap<AgentKey, Presentation>,
    concealers: usize,
    quiet_since_ms: u64,
    rest: Rest,
    font: Option<fontdue::Font>,
}

fn frame_interval() -> Duration {
    schedule::frame_interval(REFRESH_HZ)
}

impl Director {
    pub(crate) fn new(font: Option<fontdue::Font>) -> Self {
        Self {
            cursors: BTreeMap::new(),
            concealers: 0,
            quiet_since_ms: 0,
            rest: Rest::Awake,
            font,
        }
    }

    #[cfg(test)]
    pub(crate) fn cursor(&self, agent: &AgentKey) -> Option<&Presentation> {
        self.cursors.get(agent)
    }

    /// Applies `control` and draws its result; a travel returns once the
    /// cursor has arrived. Answers `false` once the overlay is disabled.
    pub(crate) fn apply(
        &mut self,
        control: &CursorOverlayControl,
        canvas: &mut impl Canvas,
        clock: &mut impl Clock,
    ) -> Result<bool, AdapterError> {
        self.rest = Rest::Awake;
        self.quiet_since_ms = clock.now_ms();
        let agent = control.agent_id().map(str::to_owned);
        match control {
            CursorOverlayControl::Disable { .. } => {
                self.cursors
                    .values_mut()
                    .for_each(|cursor| cursor.hidden = true);
                self.draw(canvas, clock.now_ms(), 1.0)?;
                canvas.settle()?;
                return Ok(false);
            }
            CursorOverlayControl::Hide { .. } | CursorOverlayControl::Show { .. } => {
                for (key, cursor) in &mut self.cursors {
                    if agent.is_none() || *key == agent {
                        cursor.hidden = control.is_hide();
                        if !cursor.hidden {
                            cursor.reveal.begin();
                        }
                    }
                }
            }
            CursorOverlayControl::Enable { style, motion, .. } => {
                let cursor = self.cursors.entry(agent).or_default();
                cursor.style = style.clone();
                cursor.gait = (motion.clone(), 0);
                adopt_label(cursor, control.label());
                cursor.hidden = false;
                cursor.reveal.begin();
            }
            CursorOverlayControl::Present {
                instruction,
                style,
                motion,
                ..
            } => {
                let cursor = self.cursors.entry(agent.clone()).or_default();
                cursor.style = style.clone();
                cursor.gait.0 = motion.clone();
                adopt_label(cursor, instruction.label());
                cursor.hidden = false;
                let target = instruction.target().copied();
                match instruction.phase() {
                    CursorPhase::Travel | CursorPhase::Drag => {
                        let destination = match instruction.phase() {
                            CursorPhase::Drag => instruction.drag_from(),
                            _ => None,
                        }
                        .unwrap_or_else(|| instruction.destination())
                        .clone();
                        self.glide(&agent, destination, canvas, clock)?;
                        if instruction.phase() == CursorPhase::Travel
                            && let Some(cursor) = self.cursors.get_mut(&agent)
                        {
                            cursor.gait.1 = cursor.gait.1.wrapping_add(1);
                        }
                    }
                    CursorPhase::Effect => {
                        let point = instruction.destination();
                        let mut effect = Effect::at(
                            point,
                            instruction.is_click(),
                            style.ripple(),
                            clock.now_ms(),
                        );
                        effect.target = target;
                        cursor.pose = Some(point.clone());
                        cursor.effect = Some(effect);
                        cursor.reveal.begin();
                    }
                }
            }
        }
        self.draw(canvas, clock.now_ms(), 1.0)?;
        canvas.settle()?;
        Ok(true)
    }

    fn glide(
        &mut self,
        agent: &AgentKey,
        destination: Point,
        canvas: &mut impl Canvas,
        clock: &mut impl Clock,
    ) -> Result<(), AdapterError> {
        let screen = canvas.screen_at(&destination).unwrap_or(NO_SCREEN);
        let Some(cursor) = self.cursors.get_mut(agent) else {
            return Ok(());
        };
        cursor.effect = None;
        let start = cursor.pose.clone().unwrap_or_else(|| Point {
            x: (destination.x + UNKNOWN_START.0).clamp(screen.x, screen.x + screen.width - 1.0),
            y: (destination.y + UNKNOWN_START.1).clamp(screen.y, screen.y + screen.height - 1.0),
        });
        let motion =
            CursorMotion::shaped(start, destination.clone(), &cursor.gait.0, cursor.gait.1);
        let arrival = motion.duration_ms() + motion.dwell_ms();
        let started = clock.now_ms();
        loop {
            let elapsed = clock.now_ms().saturating_sub(started);
            if let Some(cursor) = self.cursors.get_mut(agent) {
                cursor.pose = Some(motion.sample(elapsed));
            }
            self.draw_one(agent, canvas, clock.now_ms(), 1.0)?;
            if schedule::has_arrived(elapsed, arrival) {
                break;
            }
            canvas.settle()?;
            clock.sleep(frame_interval());
        }
        if let Some(cursor) = self.cursors.get_mut(agent) {
            cursor.pose = Some(destination);
            cursor.reveal.begin();
        }
        self.draw_one(agent, canvas, clock.now_ms(), 1.0)
    }

    /// Keeps every cursor off the screen until each conceal is revealed.
    pub(crate) fn conceal(&mut self) {
        self.concealers += 1;
    }

    pub(crate) fn reveal(&mut self) {
        self.concealers = self.concealers.saturating_sub(1);
    }

    /// Draws what is animating at `now_ms` and answers when to come back:
    /// a frame from now while anything moves, the rest deadline while a
    /// cursor shows still, and never while nothing shows.
    pub(crate) fn tick(
        &mut self,
        canvas: &mut impl Canvas,
        now_ms: u64,
    ) -> Result<Option<Duration>, AdapterError> {
        let shown = self
            .cursors
            .values()
            .any(|cursor| !cursor.hidden && cursor.pose.is_some());
        let step_ms = (CURSOR_REST_FADE_MS / u64::from(REST_FADE_STEPS)).max(1);
        match self.rest {
            Rest::Resting => return Ok(None),
            Rest::Fading(started) => {
                let step = (now_ms.saturating_sub(started) / step_ms) as u32 + 1;
                if step > REST_FADE_STEPS {
                    self.rest = Rest::Resting;
                    self.draw(canvas, now_ms, 0.0)?;
                    return Ok(None);
                }
                self.draw(canvas, now_ms, schedule::rest_fade(step, REST_FADE_STEPS))?;
                return Ok(Some(Duration::from_millis(step_ms)));
            }
            Rest::Awake => {}
        }
        let mut animating = false;
        for cursor in self.cursors.values_mut() {
            if cursor
                .effect
                .as_ref()
                .is_some_and(|effect| effect.sample(now_ms).is_none())
            {
                cursor.effect = None;
                animating = true;
            }
            let showing = !cursor.hidden && cursor.pose.is_some();
            animating |= showing && (cursor.effect.is_some() || cursor.reveal.is_playing());
        }
        if animating {
            self.quiet_since_ms = now_ms;
            self.draw(canvas, now_ms, 1.0)?;
            return Ok(Some(frame_interval()));
        }
        if !shown {
            return Ok(None);
        }
        let quiet = now_ms.saturating_sub(self.quiet_since_ms);
        if quiet >= CURSOR_IDLE_REST_MS {
            self.rest = Rest::Fading(now_ms);
            return Ok(Some(Duration::ZERO));
        }
        Ok(Some(Duration::from_millis(CURSOR_IDLE_REST_MS - quiet)))
    }

    /// Brings `canvas` to every cursor's state at `now_ms`.
    pub(crate) fn draw(
        &mut self,
        canvas: &mut impl Canvas,
        now_ms: u64,
        overlay_opacity: f64,
    ) -> Result<(), AdapterError> {
        let agents: Vec<AgentKey> = self.cursors.keys().cloned().collect();
        for agent in agents {
            self.draw_one(&agent, canvas, now_ms, overlay_opacity)?;
        }
        Ok(())
    }

    fn draw_one(
        &self,
        agent: &AgentKey,
        canvas: &mut impl Canvas,
        now_ms: u64,
        overlay_opacity: f64,
    ) -> Result<(), AdapterError> {
        let Some(cursor) = self.cursors.get(agent) else {
            return Ok(());
        };
        let visible = self.concealers == 0
            && !cursor.hidden
            && overlay_opacity > 0.0
            && self.rest != Rest::Resting;
        let tip = cursor.pose.clone().filter(|_| visible);
        let Some(tip) = tip else {
            return canvas.hide(agent);
        };
        let screen = canvas.screen_at(&tip).unwrap_or(NO_SCREEN);
        let light = Light {
            now_ms,
            overlay_opacity,
            font: self.font.as_ref(),
        };
        match frame(cursor, screen, &light) {
            Some((origin, surface)) => canvas.present(agent, &tip, (&origin, &surface)),
            None => canvas.hide(agent),
        }
    }
}

fn adopt_label(cursor: &mut Presentation, label: Option<&str>) {
    let next = label.and_then(label::clamp);
    if next != cursor.label {
        cursor.reveal = Reveal::for_label(next.as_deref());
    }
    cursor.label = next;
}

#[cfg(test)]
#[path = "director_tests.rs"]
mod tests;
