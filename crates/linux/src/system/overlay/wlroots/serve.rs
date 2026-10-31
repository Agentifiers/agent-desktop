//! The session's overlay helper: the `agent-desktop` binary started again
//! with a marker variable, holding the compositor connection the markers
//! live on between commands. It serves one session's socket and ends on
//! disable, when the session ends or its overlay is turned off, and when
//! the compositor hangs up.

use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, ErrorCode};

use super::endpoint::{PROTOCOL, ensure_private};
use super::request::{ACKNOWLEDGED, Request};
use super::roster::Roster;
use super::stage::Stage;

pub(crate) const MARKER_ENV: &str = "AGENT_DESKTOP_WLR_CURSOR_OVERLAY";
pub(crate) const SESSION_ENV: &str = "AGENT_DESKTOP_WLR_CURSOR_SESSION";
pub(crate) const SOCKET_ENV: &str = "AGENT_DESKTOP_WLR_CURSOR_SOCKET";
const SESSION_CHECK: Duration = Duration::from_secs(1);
const CLIENT_WAIT: Duration = Duration::from_secs(1);

static HOSTS_HELPER: AtomicBool = AtomicBool::new(false);

/// Whether this process's executable answers the helper marker, so it may
/// start itself as a helper. Only the command-line binary asks.
pub(crate) fn hosts_helper() -> bool {
    HOSTS_HELPER.load(Ordering::SeqCst)
}

/// What the binary's entry point calls first: `None` unless this process
/// was started as a helper.
pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    HOSTS_HELPER.store(true, Ordering::SeqCst);
    let marker = std::env::var_os(MARKER_ENV)?;
    if marker != PROTOCOL {
        return Some(Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Invalid cursor overlay helper protocol marker",
        )));
    }
    Some(serve_from_env())
}

struct SocketFile(PathBuf);

impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn io_failed(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::internal("The cursor overlay helper could not listen")
        .with_platform_detail(detail.to_string())
}

fn serve_from_env() -> Result<(), AdapterError> {
    let _ = rustix::process::setsid();
    let session = std::env::var(SESSION_ENV).map_err(io_failed)?;
    agent_desktop_core::context::validate_session_id(&session).map_err(io_failed)?;
    let socket = PathBuf::from(std::env::var_os(SOCKET_ENV).ok_or_else(|| io_failed("no socket"))?);
    if socket != super::socket_for(&session)? {
        return Err(io_failed("the socket does not belong to the session"));
    }
    let mut stage = Stage::open()?;
    let directory = socket.parent().unwrap_or(Path::new("/"));
    ensure_private(directory)?;
    match std::fs::remove_file(&socket) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_failed(error)),
    }
    let listener = UnixListener::bind(&socket).map_err(io_failed)?;
    let cleanup = SocketFile(socket);
    std::fs::set_permissions(&cleanup.0, std::fs::Permissions::from_mode(0o600))
        .map_err(io_failed)?;
    listener.set_nonblocking(true).map_err(io_failed)?;
    let mut ready = std::io::stdout();
    ready.write_all(&[ACKNOWLEDGED]).map_err(io_failed)?;
    ready.flush().map_err(io_failed)?;
    let disabled = run(&mut stage, &listener, &session)?;
    drop(cleanup);
    if let Some(mut requester) = disabled {
        let _ = requester.write_all(&[ACKNOWLEDGED]);
    }
    Ok(())
}

fn conceal_ended(mut stream: &UnixStream) -> bool {
    let mut byte = [0_u8; 1];
    !matches!(stream.read(&mut byte), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
}

/// Serves until the session's overlay ends. Answers the connection whose
/// disable ended it, acknowledged once the helper's socket is gone.
fn run(
    stage: &mut Stage,
    listener: &UnixListener,
    session: &str,
) -> Result<Option<UnixStream>, AdapterError> {
    let mut roster = Roster::default();
    let mut concealers: Vec<UnixStream> = Vec::new();
    let mut checked = Instant::now();
    loop {
        let ready = {
            let fds: Vec<BorrowedFd<'_>> = std::iter::once(listener.as_fd())
                .chain(concealers.iter().map(AsFd::as_fd))
                .collect();
            stage.wait(&fds, SESSION_CHECK)?
        };
        let mut ended = Vec::new();
        let mut kept = Vec::new();
        for (stream, readable) in concealers.drain(..).zip(ready.iter().skip(1)) {
            if *readable && conceal_ended(&stream) {
                roster.reveal();
                ended.push(stream);
            } else {
                kept.push(stream);
            }
        }
        concealers = kept;
        if !ended.is_empty() {
            roster.render(stage)?;
            for mut stream in ended {
                let _ = stream.write_all(&[ACKNOWLEDGED]);
            }
        }
        if ready.first().copied().unwrap_or(false) {
            loop {
                let stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(io_failed(error)),
                };
                if let Some(disabled) =
                    serve_one(stream, session, (&mut roster, stage), &mut concealers)?
                {
                    return Ok(Some(disabled));
                }
            }
        }
        if checked.elapsed() >= SESSION_CHECK {
            if !super::session_gate::serves(session) {
                roster.apply(&agent_desktop_core::CursorOverlayControl::disable(
                    session.to_owned(),
                ));
                roster.render(stage)?;
                return Ok(None);
            }
            checked = Instant::now();
        }
    }
}

fn serve_one(
    mut stream: UnixStream,
    session: &str,
    (roster, stage): (&mut Roster, &mut Stage),
    concealers: &mut Vec<UnixStream>,
) -> Result<Option<UnixStream>, AdapterError> {
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(CLIENT_WAIT)).is_err()
        || stream.set_write_timeout(Some(CLIENT_WAIT)).is_err()
    {
        return Ok(None);
    }
    let Ok(request) = Request::read(&stream) else {
        return Ok(None);
    };
    match request {
        Request::Conceal => {
            roster.conceal();
            roster.render(stage)?;
            if stream.write_all(&[ACKNOWLEDGED]).is_ok() && stream.set_nonblocking(true).is_ok() {
                concealers.push(stream);
            } else {
                roster.reveal();
                roster.render(stage)?;
            }
        }
        Request::Control(control) if control.session_id() == session => {
            let serving = roster.apply(&control);
            roster.render(stage)?;
            if !serving {
                return Ok(Some(stream));
            }
            let _ = stream.write_all(&[ACKNOWLEDGED]);
        }
        Request::Control(_) => {}
    }
    Ok(None)
}
