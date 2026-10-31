//! One wait over the compositor connection and the helper's sockets.

use std::os::fd::BorrowedFd;
use std::time::Duration;

use agent_desktop_core::AdapterError;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::system::wlroots::link::protocol_failed;

fn timespec(timeout: Duration) -> Timespec {
    Timespec {
        tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(timeout.subsec_nanos()),
    }
}

fn ready(fd: &PollFd<'_>) -> bool {
    fd.revents()
        .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL)
}

/// Waits up to `timeout` until `first` or one of `others` is readable or hung
/// up, and answers which are. An interrupted wait answers that none is.
pub(crate) fn readable(
    first: BorrowedFd<'_>,
    others: &[BorrowedFd<'_>],
    timeout: Duration,
) -> Result<(bool, Vec<bool>), AdapterError> {
    let mut fds: Vec<PollFd<'_>> = std::iter::once(first)
        .chain(others.iter().copied())
        .map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN))
        .collect();
    match poll(&mut fds, Some(&timespec(timeout))) {
        Ok(_) => {}
        Err(rustix::io::Errno::INTR) => return Ok((false, vec![false; others.len()])),
        Err(error) => return Err(protocol_failed(error)),
    }
    let mut answers = fds.iter().map(ready);
    let first = answers.next().unwrap_or(false);
    Ok((first, answers.collect()))
}

#[cfg(test)]
#[path = "poll_tests.rs"]
mod tests;
