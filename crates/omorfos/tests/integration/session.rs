//! The session: the event loop around the compositor, as a backend uses it.

use std::os::unix::net::UnixStream;

use omorfos_config::Action;
use smithay::reexports::calloop::channel;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::Session;
use crate::windows::RED;

#[test]
fn clients_that_connect_to_a_served_socket_are_taken_on() {
    let mut session = Session::new();
    let path = session.config.path().join("wayland-test");
    let socket = ListeningSocket::bind_absolute(path.clone()).unwrap();
    session.inner().serve(socket).unwrap();

    let mut first = session.adopt(UnixStream::connect(&path).unwrap());
    let mut second = session.adopt(UnixStream::connect(&path).unwrap());
    session.open(&mut first, "first", (200, 100), RED);
    session.open(&mut second, "second", (200, 100), RED);

    assert_eq!(session.compositor().desktop().windows().count(), 2);
}

#[test]
fn a_backend_can_add_its_own_source_of_events() {
    let mut session = Session::new();
    let (sender, receiver) = channel::channel::<Action>();
    session
        .inner()
        .handle()
        .insert_source(receiver, |event, (), compositor| {
            if let channel::Event::Msg(action) = event {
                compositor.perform(&action);
            }
        })
        .unwrap();

    sender.send(Action::NextWorkspace).unwrap();
    session.step();
    assert_eq!(session.compositor().desktop().active_workspace(), 1);

    // The other thing a channel reports is its sender going away.
    drop(sender);
    session.step();
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
}

#[test]
fn a_session_takes_turns_until_it_is_asked_to_end() {
    let mut session = Session::new();
    let mut turns = 0;
    let outcome = session.inner().run(|compositor| {
        turns += 1;
        if turns == 2 {
            compositor.perform(&Action::Quit);
        }
        Ok(())
    });
    assert!(outcome.is_ok());
    assert_eq!(turns, 2);
}

#[test]
fn a_turn_that_fails_ends_the_session_with_its_error() {
    let mut session = Session::new();
    let outcome = session.inner().run(|_| Err("the screen went away".into()));
    assert_eq!(outcome.unwrap_err().to_string(), "the screen went away");
    assert!(session.compositor().is_running());
}
