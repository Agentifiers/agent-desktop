use agent_desktop_core::{CursorOverlayControl, CursorOverlayStyle};

use super::Request;

#[test]
fn every_request_survives_its_line() {
    let requests = [
        Request::Conceal,
        Request::Control(Box::new(CursorOverlayControl::disable("run-1".into()))),
        Request::Control(Box::new(
            CursorOverlayControl::enable("run-1".into(), CursorOverlayStyle::default())
                .with_agent_id(Some("agent-a".into())),
        )),
    ];
    for request in requests {
        let line = request.encode().expect("encodes");
        assert_eq!(line.last(), Some(&b'\n'));
        assert_eq!(
            line.iter().filter(|byte| **byte == b'\n').count(),
            1,
            "one line"
        );
        assert_eq!(Request::read(line.as_slice()).expect("decodes"), request);
    }
}

#[test]
fn malformed_unknown_and_unterminated_requests_are_refused() {
    for line in [
        &b"{\"request\":\"conceal\"}"[..],
        b"{\"request\":\"explode\"}\n",
        b"{\"request\":\"conceal\",\"extra\":1}\n",
        b"[1]\n",
        b"{\"request\":\"control\",\"control\":{\"action\":\"disable\",\"session_id\":\"../x\"}}\n",
    ] {
        assert!(
            Request::read(line).is_err(),
            "{}",
            String::from_utf8_lossy(line)
        );
    }
}

#[test]
fn an_oversized_line_is_refused_without_reading_past_the_limit() {
    let mut line = vec![b' '; 64 * 1024];
    line.push(b'\n');
    assert!(Request::read(line.as_slice()).is_err());
}
