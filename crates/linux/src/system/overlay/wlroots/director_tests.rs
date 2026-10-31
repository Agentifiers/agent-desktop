use std::time::Duration;

use agent_desktop_core::{
    AdapterError, CursorMotionProfile, CursorOverlayConfig, CursorOverlayControl,
    CursorOverlayInstruction, CursorOverlayStyle, CursorPhase, Point, Rect,
};

use super::Director;
use crate::system::overlay::wlroots::canvas::{AgentKey, Canvas};
use crate::system::overlay::wlroots::clock::Clock;
use crate::system::overlay::wlroots::look::raster::Surface;

#[derive(Debug, Default)]
struct Recorder {
    shown: Vec<(String, Point, i32, i32, usize)>,
    hidden: Vec<String>,
    settled: usize,
}

fn name(agent: &AgentKey) -> String {
    agent.clone().unwrap_or_else(|| "-".into())
}

impl Canvas for Recorder {
    fn screen_at(&self, _: &Point) -> Option<Rect> {
        Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        })
    }

    fn present(
        &mut self,
        agent: &AgentKey,
        tip: &Point,
        (_, surface): (&Point, &Surface),
    ) -> Result<(), AdapterError> {
        let accent = surface
            .pixels
            .iter()
            .filter(|pixel| (**pixel & 0x00FF_FFFF) == 0x0042_99FF || is_ripple(**pixel))
            .count();
        self.shown.push((
            name(agent),
            tip.clone(),
            surface.width,
            surface.height,
            accent,
        ));
        Ok(())
    }

    fn hide(&mut self, agent: &AgentKey) -> Result<(), AdapterError> {
        self.hidden.push(name(agent));
        Ok(())
    }

    fn settle(&mut self) -> Result<(), AdapterError> {
        self.settled += 1;
        Ok(())
    }
}

fn is_ripple(pixel: u32) -> bool {
    let (alpha, red, blue) = (pixel >> 24, (pixel >> 16) & 0xFF, pixel & 0xFF);
    alpha > 0 && blue > red + 40
}

struct Stepped(u64);

impl Clock for Stepped {
    fn now_ms(&self) -> u64 {
        self.0
    }

    fn sleep(&mut self, duration: Duration) {
        self.0 += duration.as_millis() as u64;
    }
}

fn present(phase: CursorPhase, x: f64, y: f64, click: bool) -> CursorOverlayControl {
    let config = CursorOverlayConfig::enabled(Some("Clicking eight".into()), 6).expect("config");
    let instruction = CursorOverlayInstruction::new(Point { x, y }, &config, click)
        .expect("instruction")
        .with_phase(phase);
    CursorOverlayControl::present_with_style(
        "run-1".into(),
        instruction,
        CursorOverlayStyle::default(),
    )
    .with_motion(CursorMotionProfile::default())
}

fn enabled(director: &mut Director, canvas: &mut Recorder, clock: &mut Stepped) {
    let enable = CursorOverlayControl::enable("run-1".into(), CursorOverlayStyle::default());
    assert!(director.apply(&enable, canvas, clock).expect("enable"));
}

#[test]
fn enable_draws_nothing_until_a_point_is_known() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    enabled(&mut director, &mut canvas, &mut clock);
    assert!(canvas.shown.is_empty());
    assert_eq!(canvas.hidden, ["-"]);
    assert_eq!(director.tick(&mut canvas, 10).expect("tick"), None);
}

#[test]
fn a_travel_glides_frame_by_frame_and_answers_on_arrival() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    enabled(&mut director, &mut canvas, &mut clock);
    director
        .apply(
            &present(CursorPhase::Travel, 100.0, 100.0, false),
            &mut canvas,
            &mut clock,
        )
        .expect("first");
    canvas.shown.clear();
    let before = clock.0;
    director
        .apply(
            &present(CursorPhase::Travel, 900.0, 500.0, false),
            &mut canvas,
            &mut clock,
        )
        .expect("travel");
    let tips: Vec<&Point> = canvas.shown.iter().map(|shown| &shown.1).collect();
    assert!(tips.len() >= 6, "{} frames", tips.len());
    assert!(
        clock.0 - before >= 90,
        "answered after {} ms",
        clock.0 - before
    );
    let last = tips.last().expect("frames");
    assert_eq!((last.x, last.y), (900.0, 500.0));
    assert!(
        tips.iter().any(|tip| tip.x > 200.0 && tip.x < 800.0),
        "no frame mid-glide"
    );
    assert!(
        canvas.shown.iter().all(|shown| shown.4 == 0),
        "a travel plays no ripple"
    );
}

#[test]
fn a_click_effect_plays_the_ripple_after_the_answer_then_ends() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    enabled(&mut director, &mut canvas, &mut clock);
    director
        .apply(
            &present(CursorPhase::Travel, 400.0, 300.0, true),
            &mut canvas,
            &mut clock,
        )
        .expect("travel");
    let answered = clock.0;
    director
        .apply(
            &present(CursorPhase::Effect, 400.0, 300.0, true),
            &mut canvas,
            &mut clock,
        )
        .expect("effect");
    assert_eq!(clock.0, answered, "an effect is answered before it plays");
    canvas.shown.clear();
    let mut now = clock.0;
    while let Some(wait) = director.tick(&mut canvas, now).expect("tick") {
        if wait > Duration::from_millis(100) {
            break;
        }
        now += wait.as_millis().max(1) as u64;
    }
    assert!(
        canvas.shown.iter().any(|shown| shown.4 > 50),
        "no ripple frame"
    );
    assert_eq!(
        canvas.shown.last().map(|shown| shown.4),
        Some(0),
        "the ripple did not end"
    );
}

#[test]
fn a_quiet_overlay_fades_out_and_rests_until_the_next_control() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    enabled(&mut director, &mut canvas, &mut clock);
    director
        .apply(
            &present(CursorPhase::Travel, 400.0, 300.0, false),
            &mut canvas,
            &mut clock,
        )
        .expect("travel");
    let mut now = clock.0;
    let mut guard = 0;
    while let Some(wait) = director.tick(&mut canvas, now).expect("tick") {
        now += wait.as_millis().max(1) as u64;
        guard += 1;
        assert!(guard < 500);
    }
    assert!(now - clock.0 >= 6_000, "rested after {} ms", now - clock.0);
    assert_eq!(canvas.hidden.last().map(String::as_str), Some("-"));
    canvas.shown.clear();
    director
        .apply(
            &CursorOverlayControl::show("run-1".into()),
            &mut canvas,
            &mut clock,
        )
        .expect("show");
    assert_eq!(canvas.shown.len(), 1, "a control wakes the overlay");
}

#[test]
fn hide_conceal_and_disable_take_cursors_off_and_route_by_agent() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    for agent in ["a", "b"] {
        let control =
            present(CursorPhase::Travel, 50.0, 60.0, false).with_agent_id(Some(agent.into()));
        director
            .apply(&control, &mut canvas, &mut clock)
            .expect("present");
    }
    let hide = CursorOverlayControl::hide("run-1".into()).with_agent_id(Some("a".into()));
    director
        .apply(&hide, &mut canvas, &mut clock)
        .expect("hide");
    assert!(
        director
            .cursor(&Some("a".into()))
            .is_some_and(|cursor| cursor.hidden)
    );
    assert!(
        director
            .cursor(&Some("b".into()))
            .is_some_and(|cursor| !cursor.hidden)
    );
    director.conceal();
    canvas.hidden.clear();
    director.draw(&mut canvas, clock.0, 1.0).expect("draw");
    assert_eq!(canvas.hidden, ["a", "b"]);
    director.reveal();
    let disable = CursorOverlayControl::disable("run-1".into());
    assert!(
        !director
            .apply(&disable, &mut canvas, &mut clock)
            .expect("disable")
    );
    assert!(canvas.hidden.ends_with(&["a".to_owned(), "b".to_owned()]));
}

#[test]
fn a_new_label_waits_for_the_arrival_before_it_eases_in() {
    let (mut director, mut canvas, mut clock) =
        (Director::new(None), Recorder::default(), Stepped(0));
    director
        .apply(
            &present(CursorPhase::Travel, 400.0, 300.0, false),
            &mut canvas,
            &mut clock,
        )
        .expect("travel");
    let cursor = director.cursor(&None).expect("cursor");
    assert_eq!(cursor.label.as_deref(), Some("Clicking eight"));
    assert!(
        cursor.reveal.is_playing(),
        "the card eases in once the cursor lands"
    );
}
