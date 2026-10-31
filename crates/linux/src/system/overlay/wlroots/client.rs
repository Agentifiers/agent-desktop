//! A command's side of a helper connection: send one request and wait,
//! within the deadline, for the byte that says the compositor applied it.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use super::request::{ACKNOWLEDGED, Request};

const RELEASE_WAIT: Duration = Duration::from_millis(1_500);
const SHORTEST_WAIT: Duration = Duration::from_millis(1);

fn unanswered(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::internal("The cursor overlay helper did not acknowledge the request")
        .with_platform_detail(detail.to_string())
}

fn remaining(deadline: Deadline) -> Result<Duration, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        return Err(deadline.timeout_error());
    }
    Ok(remaining.max(SHORTEST_WAIT))
}

/// Sends `request` to the helper listening on `socket`. Answers `None` when
/// no helper listens there, and the acknowledged connection otherwise.
pub(crate) fn exchange(
    socket: &Path,
    request: &Request,
    deadline: Deadline,
) -> Result<Option<UnixStream>, AdapterError> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(unanswered(error)),
    };
    let line = request.encode()?;
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(unanswered)?;
    stream.write_all(&line).map_err(unanswered)?;
    stream
        .set_read_timeout(Some(remaining(deadline)?))
        .map_err(unanswered)?;
    let mut answer = [0_u8; 1];
    match stream.read_exact(&mut answer) {
        Ok(()) if answer[0] == ACKNOWLEDGED => Ok(Some(stream)),
        Ok(()) => Err(unanswered("unexpected acknowledgement byte")),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Err(deadline
                .timeout_error()
                .with_platform_detail(error.to_string()))
        }
        Err(error) => Err(unanswered(error)),
    }
}

/// Ends a conceal and waits briefly for the helper to show its markers again.
pub(crate) fn release(stream: UnixStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.set_read_timeout(Some(RELEASE_WAIT));
    let mut answer = [0_u8; 1];
    let _ = (&stream).read(&mut answer);
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
