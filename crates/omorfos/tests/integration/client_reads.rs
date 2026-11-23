//! The tests' own Wayland client, which says when it cannot read from the
//! compositor rather than taking that for nothing having been sent.

use std::os::unix::net::UnixStream;

use crate::common::Client;

#[test]
fn a_client_whose_compositor_has_gone_is_told_it_cannot_read() {
    let (stream, compositor) = UnixStream::pair().unwrap();
    let mut client = Client::new(stream);
    drop(compositor);

    assert!(client.receive().is_err());
}

#[test]
#[should_panic(expected = "Wayland client read failed")]
fn a_compositor_gone_without_an_error_is_not_taken_for_one_that_sent_none() {
    let (stream, compositor) = UnixStream::pair().unwrap();
    let mut client = Client::new(stream);
    drop(compositor);

    let _ = client.error();
}
