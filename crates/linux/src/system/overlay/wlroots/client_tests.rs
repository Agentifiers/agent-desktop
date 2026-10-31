use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use agent_desktop_core::{CursorOverlayControl, Deadline};

use super::{exchange, release};
use crate::system::overlay::wlroots::request::{ACKNOWLEDGED, Request};

fn socket(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ad-wc-{name}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn deadline() -> Deadline {
    Deadline::detached_after(2_000).expect("deadline")
}

fn disable() -> Request {
    Request::Control(Box::new(CursorOverlayControl::disable("run-1".into())))
}

#[test]
fn no_listener_means_no_helper() {
    let path = socket("absent");
    assert!(
        exchange(&path, &disable(), deadline())
            .expect("answers")
            .is_none()
    );
    std::fs::write(&path, b"").expect("stale file");
    let stale = exchange(&path, &disable(), deadline());
    std::fs::remove_file(&path).expect("cleanup");
    assert!(stale.expect("answers").is_none());
}

#[test]
fn an_acknowledged_request_keeps_its_connection_until_released() {
    let path = socket("ack");
    let listener = UnixListener::bind(&path).expect("bind");
    let helper = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = Request::read(&stream).expect("one request");
        stream.write_all(&[ACKNOWLEDGED]).expect("ack");
        let mut rest = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut rest).expect("eof");
        stream.write_all(&[ACKNOWLEDGED]).expect("revealed");
        request
    });
    let held = exchange(&path, &Request::Conceal, deadline())
        .expect("answers")
        .expect("a helper");
    release(held);
    assert_eq!(helper.join().expect("helper"), Request::Conceal);
    std::fs::remove_file(&path).expect("cleanup");
}

#[test]
fn a_helper_that_hangs_up_without_answering_is_an_error() {
    let path = socket("hangup");
    let listener = UnixListener::bind(&path).expect("bind");
    let helper = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let _ = Request::read(&stream);
    });
    assert!(exchange(&path, &disable(), deadline()).is_err());
    helper.join().expect("helper");
    std::fs::remove_file(&path).expect("cleanup");
}

#[test]
fn a_silent_helper_times_out_at_the_deadline() {
    let path = socket("silent");
    let _listener = UnixListener::bind(&path).expect("bind");
    let short = Deadline::detached_after(150).expect("deadline");
    let error = exchange(&path, &disable(), short).expect_err("times out");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
    std::fs::remove_file(&path).expect("cleanup");
}
