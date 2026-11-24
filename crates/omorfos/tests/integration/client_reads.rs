//! The tests' own Wayland client, which says when it cannot read from the
//! compositor rather than taking that for nothing having been sent.
//!
//! The compositor's end is gone once the last copy of it is closed, which
//! another test starting a program meanwhile may hold for a moment: so it
//! is read until it is seen to be gone, as `Session::eventually` waits.

use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::common::Client;

/// How many times, a millisecond apart, the client reads before the
/// compositor's end must have been seen to go.
const TRIES: usize = 5000;

#[test]
fn a_client_whose_compositor_has_gone_is_told_it_cannot_read() {
    let (stream, compositor) = UnixStream::pair().unwrap();
    let mut client = Client::new(stream);
    drop(compositor);

    let told = (0..TRIES).any(|_| {
        std::thread::sleep(Duration::from_millis(1));
        client.receive().is_err()
    });
    assert!(told, "the compositor's end was never seen to go");
}

#[test]
#[should_panic(expected = "Wayland client read failed")]
fn a_compositor_gone_without_an_error_is_not_taken_for_one_that_sent_none() {
    let (stream, compositor) = UnixStream::pair().unwrap();
    let mut client = Client::new(stream);
    drop(compositor);

    // Reading a connection that is gone fails the test, here as wanted;
    // until it is seen to be gone there is no error, and it is read again.
    for _ in 0..TRIES {
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(client.error(), None);
    }
}
