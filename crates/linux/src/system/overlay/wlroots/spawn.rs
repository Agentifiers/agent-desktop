//! Starts a session's overlay helper: this same command-line binary, with
//! null standard input and error, an allowlisted environment, and a pipe on
//! standard output that carries one byte once the helper listens. The
//! interaction lease a caller passed down is made close-on-exec first, so
//! the long-lived helper never holds it.

use std::io::Read;
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::endpoint::PROTOCOL;
use super::request::ACKNOWLEDGED;
use super::serve::{MARKER_ENV, SESSION_ENV, SOCKET_ENV, hosts_helper};
use crate::input::clipboard::owner::owner_spawn_spec::protect_lease_descriptor;

const INHERITED: [&str; 4] = [
    "HOME",
    "XDG_RUNTIME_DIR",
    "WAYLAND_DISPLAY",
    "AGENT_DESKTOP_HOME",
];

fn failed(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::internal("The cursor overlay helper did not start")
        .with_platform_detail(detail.to_string())
}

/// The helper command for `session_id`'s `socket`, reading the parent's
/// variables through `parent_env`.
pub(crate) fn command(
    executable: &Path,
    session_id: &str,
    socket: &Path,
    parent_env: &dyn Fn(&str) -> Option<String>,
) -> Command {
    let mut command = Command::new(executable);
    command
        .env_clear()
        .envs(
            INHERITED
                .into_iter()
                .filter_map(|name| parent_env(name).map(|value| (name, value))),
        )
        .env(MARKER_ENV, PROTOCOL)
        .env(SESSION_ENV, session_id)
        .env(SOCKET_ENV, socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Starts the helper and waits, within `deadline`, until it listens.
pub(crate) fn start(
    session_id: &str,
    socket: &Path,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if !hosts_helper() {
        return Err(AdapterError::not_supported(
            "cursor overlay outside the agent-desktop command-line binary",
        )
        .with_details(serde_json::json!({"rendered": false, "cause": "no_overlay_host"})));
    }
    let executable = std::env::current_exe().map_err(failed)?;
    let parent_env = |name: &str| std::env::var(name).ok();
    protect_lease_descriptor(&parent_env);
    let mut child = command(&executable, session_id, socket, &parent_env)
        .spawn()
        .map_err(failed)?;
    let Some(mut ready) = child.stdout.take() else {
        stop(child);
        return Err(failed("the helper has no readiness pipe"));
    };
    let mut answer = [0_u8; 1];
    let answered = super::poll::readable(ready.as_fd(), &[], deadline.remaining())
        .map(|(readable, _)| readable && ready.read(&mut answer).is_ok_and(|read| read == 1));
    match answered {
        Ok(true) if answer[0] == ACKNOWLEDGED => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Ok(_) if deadline.is_expired() => {
            stop(child);
            Err(deadline.timeout_error())
        }
        Ok(_) => {
            let status = child.wait().map_err(failed)?;
            Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The cursor overlay helper could not draw on this compositor",
            )
            .with_platform_detail(format!("the helper exited with {status}")))
        }
        Err(error) => {
            stop(child);
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
