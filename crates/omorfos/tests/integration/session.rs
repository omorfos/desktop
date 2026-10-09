//! The session: the event loop around the compositor, as a backend uses it.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use omorfos_config::Action;
use serde_json::json;
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
fn a_session_takes_a_turn_each_time_it_is_woken_until_asked_to_end() {
    let mut session = Session::new();
    let wake = session.waker();
    wake.send(()).unwrap();
    let mut turns = 0;
    let outcome = session.inner().run(|compositor| {
        turns += 1;
        if turns == 3 {
            compositor.perform(&Action::Quit);
        }
        wake.send(()).unwrap();
        Ok(())
    });
    assert!(outcome.is_ok());
    assert_eq!(turns, 3);
}

#[test]
fn a_turn_that_fails_ends_the_session_with_its_error() {
    let mut session = Session::new();
    session.waker().send(()).unwrap();
    let outcome = session.inner().run(|_| Err("the screen went away".into()));
    assert_eq!(outcome.unwrap_err().to_string(), "the screen went away");
    assert!(session.compositor().is_running());
}

#[test]
fn a_sleeping_session_is_woken_by_a_saved_configuration_and_reloads_it() {
    let mut session = Session::new();
    session.write_config("config.yaml", "workspaces: [a, b, c]\n");
    session.run_until(|compositor| compositor.desktop().workspace_count() == 3);
}

#[test]
fn a_sleeping_session_is_woken_by_an_ipc_peer_and_answers_it() {
    let mut session = Session::new();
    let path = session.listen();
    let mut peer = UnixStream::connect(&path).unwrap();
    peer.write_all(b"\"version\"\n").unwrap();
    peer.set_nonblocking(true).unwrap();

    let mut answer = [0; 256];
    session.run_until(|_| peer.read(&mut answer).is_ok());
    assert!(answer.starts_with(b"{\"version\":"));
}

#[test]
fn a_sleeping_session_is_woken_by_a_peer_that_writes_later() {
    let mut session = Session::new();
    let path = session.listen();
    let mut peer = UnixStream::connect(&path).unwrap();
    peer.set_nonblocking(true).unwrap();
    // Taken on while it has nothing to say.
    session.step();
    session.step();

    peer.write_all(b"\"version\"\n").unwrap();
    let mut answer = [0; 256];
    session.run_until(|_| peer.read(&mut answer).is_ok());
    assert!(answer.starts_with(b"{\"version\":"));
}

#[test]
fn a_sleeping_session_is_woken_when_a_program_it_started_exits() {
    let mut session = Session::new();
    let command: Action = serde_json::from_value(json!({ "spawn": ["sleep", "0.3"] })).unwrap();
    session.compositor_mut().perform(&command);
    assert_eq!(session.compositor().problems(), [""; 0]);

    // Nothing else is going to happen, so a turn taken once the program is
    // over can only be the one its exit caused.
    let started = Instant::now();
    session.run_until(|_| started.elapsed() >= Duration::from_millis(250));
}
