use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::readable;

#[test]
fn readiness_is_reported_per_descriptor() {
    let (first, mut first_peer) = UnixStream::pair().expect("pair");
    let (second, second_peer) = UnixStream::pair().expect("pair");
    let (third, _third_peer) = UnixStream::pair().expect("pair");
    let quiet = readable(
        first.as_fd(),
        &[second.as_fd(), third.as_fd()],
        Duration::from_millis(10),
    )
    .expect("waits");
    assert_eq!(quiet, (false, vec![false, false]));
    first_peer.write_all(b"x").expect("write");
    drop(second_peer);
    let ready = readable(
        first.as_fd(),
        &[second.as_fd(), third.as_fd()],
        Duration::from_secs(1),
    )
    .expect("waits");
    assert_eq!(ready, (true, vec![true, false]));
}
