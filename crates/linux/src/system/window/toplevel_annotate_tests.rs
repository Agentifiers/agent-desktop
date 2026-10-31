use agent_desktop_core::{ProcessId, WindowState};

use super::*;
use crate::system::wlroots::toplevel_session::fake::toplevel;

fn window(title: &str, pid: u32) -> WindowInfo {
    WindowInfo {
        id: format!("w-{pid}"),
        title: title.to_owned(),
        app: "app".to_owned(),
        pid: ProcessId::new(pid),
        process_instance: Some("boot:1".to_owned()),
        bounds: None,
        state: WindowState {
            minimized: Some(false),
            visible: Some(true),
            ..WindowState::default()
        },
    }
}

fn names(pid: u32) -> Vec<String> {
    match pid {
        1 => vec!["gnome-calculator".to_owned()],
        _ => vec!["gnome-text-editor".to_owned()],
    }
}

fn run(windows: &mut [WindowInfo], toplevels: &[ToplevelState]) {
    let mut infos: Vec<&mut WindowInfo> = windows.iter_mut().collect();
    annotate(&mut infos, toplevels, names);
}

#[test]
fn a_minimized_toplevel_marks_its_window_minimized_and_unseen() {
    let mut calculator = toplevel(3, "Calculator", "org.gnome.Calculator");
    calculator.flags.minimized = true;
    let mut windows = [window("Calculator", 1), window("Notes", 2)];
    run(&mut windows, &[calculator, toplevel(4, "Notes", "x")]);
    assert_eq!(windows[0].state.minimized, Some(true));
    assert_eq!(windows[0].state.visible, Some(false));
    assert_eq!(windows[1].state.minimized, Some(false));
    assert_eq!(windows[1].state.visible, Some(true));
}

#[test]
fn the_activated_toplevel_is_focused_when_no_frame_reported_focus() {
    let mut notes = toplevel(4, "Notes", "x");
    notes.flags.activated = true;
    let mut windows = [window("Calculator", 1), window("Notes", 2)];
    run(&mut windows, &[toplevel(3, "Calculator", "c"), notes]);
    assert!(!windows[0].state.is_focused);
    assert!(windows[1].state.is_focused);
}

#[test]
fn a_focus_the_frames_already_agreed_on_is_kept() {
    let mut notes = toplevel(4, "Notes", "x");
    notes.flags.activated = true;
    let mut windows = [window("Calculator", 1), window("Notes", 2)];
    windows[0].state.is_focused = true;
    run(&mut windows, &[toplevel(3, "Calculator", "c"), notes]);
    assert!(windows[0].state.is_focused);
    assert!(!windows[1].state.is_focused);
}

#[test]
fn twin_toplevels_leave_the_window_as_the_toolkit_listed_it() {
    let mut first = toplevel(3, "Calculator", "org.gnome.Calculator");
    first.flags.minimized = true;
    first.flags.activated = true;
    let second = toplevel(5, "Calculator", "org.gnome.Calculator");
    let mut windows = [window("Calculator", 1)];
    run(&mut windows, &[first, second]);
    assert_eq!(windows[0].state.minimized, Some(false));
    assert!(!windows[0].state.is_focused);
    assert!(windows[0].bounds.is_none());
}
