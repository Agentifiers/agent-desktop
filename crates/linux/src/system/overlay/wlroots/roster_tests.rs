use agent_desktop_core::{
    AdapterError, CursorOverlayConfig, CursorOverlayControl, CursorOverlayInstruction,
    CursorOverlayStyle, Point,
};

use super::Roster;
use crate::system::overlay::wlroots::canvas::{AgentKey, Canvas};

#[derive(Debug, Default)]
struct Recorder {
    calls: Vec<String>,
}

impl Canvas for Recorder {
    fn show(
        &mut self,
        agent: &AgentKey,
        point: &Point,
        style: &CursorOverlayStyle,
    ) -> Result<(), AdapterError> {
        self.calls.push(format!(
            "show {} {},{} x{}",
            agent.as_deref().unwrap_or("-"),
            point.x,
            point.y,
            style.size()
        ));
        Ok(())
    }

    fn hide(&mut self, agent: &AgentKey) -> Result<(), AdapterError> {
        self.calls
            .push(format!("hide {}", agent.as_deref().unwrap_or("-")));
        Ok(())
    }

    fn settle(&mut self) -> Result<(), AdapterError> {
        self.calls.push("settle".into());
        Ok(())
    }
}

fn present(x: f64, y: f64, agent: Option<&str>) -> CursorOverlayControl {
    let config = CursorOverlayConfig::enabled(None, 6).expect("config");
    let instruction =
        CursorOverlayInstruction::new(Point { x, y }, &config, false).expect("instruction");
    CursorOverlayControl::present("run-1".into(), instruction).with_agent_id(agent.map(Into::into))
}

fn rendered(roster: &Roster) -> Vec<String> {
    let mut canvas = Recorder::default();
    roster.render(&mut canvas).expect("render");
    canvas.calls
}

#[test]
fn enable_creates_a_marker_that_waits_for_a_point() {
    let mut roster = Roster::default();
    let enable = CursorOverlayControl::enable("run-1".into(), CursorOverlayStyle::default());
    assert!(roster.apply(&enable));
    assert_eq!(rendered(&roster), ["hide -", "settle"]);
}

#[test]
fn a_present_moves_the_tip_to_the_destination_and_follows_the_next() {
    let mut roster = Roster::default();
    roster.apply(&CursorOverlayControl::enable(
        "run-1".into(),
        CursorOverlayStyle::default(),
    ));
    roster.apply(&present(100.0, 200.0, None));
    assert_eq!(rendered(&roster), ["show - 100,200 x1", "settle"]);
    roster.apply(&present(640.0, 360.0, None));
    assert_eq!(rendered(&roster), ["show - 640,360 x1", "settle"]);
}

#[test]
fn hide_and_show_without_an_agent_reach_every_marker() {
    let mut roster = Roster::default();
    roster.apply(&present(1.0, 2.0, Some("a")));
    roster.apply(&present(3.0, 4.0, Some("b")));
    roster.apply(&CursorOverlayControl::hide("run-1".into()));
    assert_eq!(rendered(&roster), ["hide a", "hide b", "settle"]);
    roster.apply(&CursorOverlayControl::show("run-1".into()).with_agent_id(Some("b".into())));
    assert_eq!(rendered(&roster), ["hide a", "show b 3,4 x1", "settle"]);
    roster.apply(&CursorOverlayControl::show("run-1".into()));
    assert_eq!(
        rendered(&roster),
        ["show a 1,2 x1", "show b 3,4 x1", "settle"]
    );
}

#[test]
fn a_hide_names_no_marker_into_being() {
    let mut roster = Roster::default();
    roster.apply(&CursorOverlayControl::hide("run-1".into()).with_agent_id(Some("a".into())));
    roster.apply(&CursorOverlayControl::show("run-1".into()));
    assert!(roster.marker(&Some("a".into())).is_none());
    assert_eq!(rendered(&roster), ["settle"]);
}

#[test]
fn a_present_shows_a_hidden_marker_again() {
    let mut roster = Roster::default();
    roster.apply(&present(5.0, 6.0, None));
    roster.apply(&CursorOverlayControl::hide("run-1".into()));
    roster.apply(&present(7.0, 8.0, None));
    assert_eq!(rendered(&roster), ["show - 7,8 x1", "settle"]);
}

#[test]
fn concealing_keeps_markers_off_until_every_conceal_is_revealed() {
    let mut roster = Roster::default();
    roster.apply(&present(5.0, 6.0, None));
    roster.conceal();
    roster.conceal();
    roster.apply(&present(9.0, 9.0, None));
    assert_eq!(rendered(&roster), ["hide -", "settle"]);
    roster.reveal();
    assert_eq!(rendered(&roster), ["hide -", "settle"]);
    roster.reveal();
    assert_eq!(rendered(&roster), ["show - 9,9 x1", "settle"]);
}

#[test]
fn disable_takes_every_marker_off_the_screen() {
    let mut roster = Roster::default();
    roster.apply(&present(5.0, 6.0, Some("a")));
    roster.apply(&present(7.0, 8.0, None));
    assert!(!roster.apply(&CursorOverlayControl::disable("run-1".into())));
    assert_eq!(rendered(&roster), ["hide -", "hide a", "settle"]);
}

#[test]
fn an_enable_keeps_the_point_and_takes_the_new_style() {
    let mut roster = Roster::default();
    roster.apply(&present(5.0, 6.0, None));
    let mut style = CursorOverlayStyle::default();
    style.set_size(2.0);
    roster.apply(&CursorOverlayControl::enable("run-1".into(), style));
    assert_eq!(rendered(&roster), ["show - 5,6 x2", "settle"]);
}
