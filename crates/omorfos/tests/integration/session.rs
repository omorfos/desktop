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
    session.inner().serve(socket, omorfos::Roles::NONE).unwrap();

    let mut first = session.adopt(UnixStream::connect(&path).unwrap());
    let mut second = session.adopt(UnixStream::connect(&path).unwrap());
    session.open(&mut first, "first", (200, 100), RED);
    session.open(&mut second, "second", (200, 100), RED);

    assert_eq!(session.compositor().desktop().windows().count(), 2);
}

#[test]
fn a_program_that_connects_when_no_descriptor_is_left_is_taken_on_once_one_is() {
    let name =
        "session::a_program_that_connects_when_no_descriptor_is_left_is_taken_on_once_one_is";
    if !crate::common::starved(name) {
        return;
    }
    let mut session = Session::new();
    let path = session.config.path().join("wayland-test");
    let socket = ListeningSocket::bind_absolute(path.clone()).unwrap();
    session.inner().serve(socket, omorfos::Roles::NONE).unwrap();

    // Every descriptor taken, as a program can take them, but the one the
    // program that connects next has for its end.
    let taken = crate::common::take_descriptors(1);
    let waiting = UnixStream::connect(&path).unwrap();

    // The session goes on, and does not spin on what it cannot take: a
    // turn with nothing it can do waits as long as it was told to.
    for _ in 0..3 {
        session.step();
    }
    let turns = crate::common::turns_within(&mut session, Duration::from_millis(300));
    assert!(turns < 20, "it spun, {turns} turns");

    // Once there are descriptors again, the program is taken on.
    drop(taken);
    for _ in 0..200 {
        session.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut program = session.adopt(waiting);
    session.open(&mut program, "late", (200, 100), RED);
    assert_eq!(session.compositor().desktop().windows().count(), 1);
}

#[test]
fn text_drawn_first_when_no_descriptor_is_left_is_drawn() {
    let name = "session::text_drawn_first_when_no_descriptor_is_left_is_drawn";
    if !crate::common::starved(name) {
        return;
    }
    // A session that has drawn no text yet, as one may not have when a
    // program takes every descriptor.
    let mut session = Session::new();
    let path = session.config.path().join("wayland-test");
    let socket = ListeningSocket::bind_absolute(path.clone()).unwrap();
    session.inner().serve(socket, omorfos::Roles::NONE).unwrap();
    let taken = crate::common::take_descriptors(1);
    // A program connecting then cannot be taken on, and the user is told
    // so, in words drawn for the first time with no descriptor to open a
    // font with.
    let _waiting = UnixStream::connect(&path).unwrap();
    for _ in 0..3 {
        session.step();
    }
    drop(session.draw());
    drop(taken);
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

    sender.send(Action::NewWorkspace).unwrap();
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

#[test]
fn what_falls_due_while_a_turn_waits_is_done_in_that_turn() {
    let mut session = Session::with_config(&[("config.yaml", "idle: { screen_off: 1 }\n")]);
    session.step();
    let due = session.compositor().deadline().unwrap();
    // A turn woken before it is due, by what the session has still to
    // take in, is not the one this is about: the one that ends after it is.
    while Instant::now() < due {
        let wait = due.saturating_duration_since(Instant::now()) + Duration::from_millis(20);
        session.inner().dispatch_now(Some(wait)).unwrap();
    }
    assert!(!session.compositor().screens_on(), "not the turn after");
}
