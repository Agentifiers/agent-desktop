use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;

use agent_desktop_core::Deadline;

use super::{command, start};

#[test]
fn the_helper_gets_its_marker_and_only_allowlisted_variables() {
    let parent = |name: &str| match name {
        "HOME" | "WAYLAND_DISPLAY" | "AGENT_DESKTOP_INTERACTION_LEASE_FD" | "DISPLAY" => {
            Some(format!("{name}-value"))
        }
        _ => None,
    };
    let command = command(
        Path::new("/usr/bin/agent-desktop"),
        "run-1",
        Path::new("/run/user/1/s.sock"),
        &parent,
    );
    let envs: BTreeMap<&OsStr, Option<&OsStr>> = command.get_envs().collect();
    let names: Vec<&str> = envs.keys().filter_map(|key| key.to_str()).collect();
    assert_eq!(
        names,
        [
            "AGENT_DESKTOP_WLR_CURSOR_OVERLAY",
            "AGENT_DESKTOP_WLR_CURSOR_SESSION",
            "AGENT_DESKTOP_WLR_CURSOR_SOCKET",
            "HOME",
            "WAYLAND_DISPLAY",
        ]
    );
    assert_eq!(
        envs[OsStr::new("AGENT_DESKTOP_WLR_CURSOR_SESSION")],
        Some(OsStr::new("run-1"))
    );
    assert_eq!(command.get_program(), OsStr::new("/usr/bin/agent-desktop"));
}

#[test]
fn a_process_that_does_not_answer_the_marker_never_starts_itself() {
    let deadline = Deadline::detached_after(500).expect("deadline");
    let error = start("run-1", Path::new("/nonexistent.sock"), deadline)
        .expect_err("a library host refuses");
    assert_eq!(
        error.code,
        agent_desktop_core::ErrorCode::PlatformNotSupported
    );
    assert_eq!(error.details.expect("details")["cause"], "no_overlay_host");
}
