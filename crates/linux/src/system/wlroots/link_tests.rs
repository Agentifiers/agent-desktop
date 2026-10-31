use std::time::Duration;

use super::*;

#[test]
fn a_poll_timeout_keeps_seconds_and_nanoseconds() {
    let spec = timespec(Duration::from_millis(2_500));
    assert_eq!(spec.tv_sec, 2);
    assert_eq!(spec.tv_nsec, 500_000_000);
}

#[test]
fn connection_and_protocol_failures_carry_the_compositor_detail() {
    let connect = connect_failed("no socket");
    let protocol = protocol_failed("broken pipe");
    assert_eq!(connect.code, ErrorCode::ActionFailed);
    assert_eq!(connect.platform_detail.as_deref(), Some("no socket"));
    assert_eq!(protocol.platform_detail.as_deref(), Some("broken pipe"));
}
