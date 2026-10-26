//! The compositor behind the relay (ADR 61): it serves the programs the
//! relay hands it, knows each by the process the relay says it is, and
//! steps down when the relay goes.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::rc::Rc;
use std::time::Duration;

use omorfos_ipc::hold::{Told, Word};
use omorfos_ipc::relay::{self, Hand, Program};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session};
use crate::hold::{Holder, hold_with};
use crate::windows::{GREEN, RED};

/// Puts `session` under a holder that has it served by a relay; returns the
/// holder, the relay's end of its connection to the compositor, where the
/// socket is, and the socket.
fn relayed(session: &mut Session) -> (Holder, OwnedFd, PathBuf, UnixListener) {
    let path = session.config.path().join("wayland-relayed");
    let socket = UnixListener::bind(&path).unwrap();
    let (relays, compositor) = relay::pair().unwrap();
    let told = Told {
        relayed: true,
        ..Told::default()
    };
    let holder = hold_with(session, told, &[compositor.as_fd(), socket.as_fd()]);
    // The socket is the relay's to serve, and kept open as it would be.
    (holder, relays, path, socket)
}

/// Hands the compositor, as the relay would, a program it says is
/// `process`; returns the program's end.
fn hand(session: &mut Session, relays: &OwnedFd, process: u32) -> Client {
    let (program, theirs) = UnixStream::pair().unwrap();
    let who = Hand::Program(Program {
        pid: process,
        uid: 0,
        gid: 0,
        number: None,
    });
    relay::send(relays.as_fd(), &who, &[theirs.as_fd()]).unwrap();
    session.step();
    session.adopt(program)
}

/// A program of a name its own, running until it is dropped.
struct Sleeper(Child);

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_program_the_relay_hands_over_is_served() {
    let mut session = Session::new();
    let (_holder, relays, _, _socket) = relayed(&mut session);
    let mut client = hand(&mut session, &relays, std::process::id());
    session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(session.draw().at(10, 10), RED);
}

#[test]
fn a_program_is_known_by_the_process_the_relay_says_it_is() {
    let mut session = Session::new();
    let (_holder, relays, _, _socket) = relayed(&mut session);
    let sleeper = Sleeper(Command::new("sleep").arg("30").spawn().unwrap());
    // Started, a process is named for its program only a moment later: the
    // kernel lets the one that started it go on before it renames it.
    let comm = format!("/proc/{}/comm", sleeper.0.id());
    session.eventually(|_| std::fs::read_to_string(&comm).unwrap_or_default() == "sleep\n");
    let mut client = hand(&mut session, &relays, sleeper.0.id());

    // Refused, the program is named for the user by its process, which is
    // the one the relay said and not the relay's, nor this one's.
    let strip = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    let _held = client.layer(&strip);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::LayerClosed));
    let said: Vec<&str> = session.compositor().events().collect();
    assert!(
        said.iter().any(|said| said.starts_with("`sleep` asked")),
        "{said:?}"
    );
}

#[test]
fn what_the_relay_hands_that_is_not_a_program_and_its_connection_is_passed_over() {
    let mut session = Session::new();
    let (_holder, relays, _, _socket) = relayed(&mut session);
    let (_ours, theirs) = UnixStream::pair().unwrap();
    relay::send(relays.as_fd(), &Hand::Socket, &[theirs.as_fd()]).unwrap();
    let alone = Hand::Program(Program {
        pid: 1,
        uid: 0,
        gid: 0,
        number: None,
    });
    relay::send(relays.as_fd(), &alone, &[]).unwrap();
    session.step();

    let mut client = hand(&mut session, &relays, std::process::id());
    session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(session.draw().at(10, 10), RED);
}

#[test]
fn the_compositor_does_not_serve_the_socket_the_relay_serves() {
    let mut session = Session::new();
    let (_holder, _relays, path, _socket) = relayed(&mut session);
    let mut stray = UnixStream::connect(&path).unwrap();
    // wl_display.sync, which a compositor serving the socket would answer.
    let mut sync = 1_u32.to_ne_bytes().to_vec();
    sync.extend((12_u32 << 16).to_ne_bytes());
    sync.extend(2_u32.to_ne_bytes());
    stray.write_all(&sync).unwrap();
    session.step();
    session.step();
    stray.set_nonblocking(true).unwrap();
    let mut answer = [0; 64];
    assert!(stray.read(&mut answer).is_err(), "nothing is said to it");
}

#[test]
fn when_the_relay_goes_the_compositor_steps_down_to_be_started_anew() {
    let mut session = Session::new();
    let (mut holder, relays, _, _socket) = relayed(&mut session);
    session.step();
    holder.said();
    drop(relays);
    // Heard once its connection closes, which a program another test is
    // starting may hold open for a moment.
    session.eventually(|session| session.compositor().stepping_down());
    session.step();
    assert!(holder.said().contains(&Word::SteppingDown));
}

#[test]
fn relayed_with_no_socket_beside_it_is_not_held() {
    let mut session = Session::new();
    let (_relays, compositor) = relay::pair().unwrap();
    let (holder, theirs) = UnixStream::pair().unwrap();
    let told = Told {
        relayed: true,
        ..Told::default()
    };
    omorfos_ipc::hold::give(&holder, &told.written(), &[compositor.as_fd()]).unwrap();
    assert!(session.inner().hold(theirs).is_err());
}

/// A relay, as the tests step it, and the holder's end of its connection.
struct Relayed {
    relay: Rc<RefCell<omorfos_relay::Relay>>,
    holder: OwnedFd,
    socket: UnixListener,
    path: PathBuf,
    _directory: tempfile::TempDir,
}

impl Relayed {
    /// A relay handed a socket of its own.
    fn new() -> Self {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join("wayland-kept");
        let socket = UnixListener::bind(&path).unwrap();
        let (holder, relays) = relay::pair().unwrap();
        relay::send(holder.as_fd(), &Hand::Socket, &[socket.as_fd()]).unwrap();
        Self {
            relay: Rc::new(RefCell::new(omorfos_relay::Relay::new(relays))),
            holder,
            socket,
            path,
            _directory: directory,
        }
    }

    /// Starts a compositor joined to the relay, which steps it with itself.
    fn compositor(&self) -> (Session, Holder) {
        self.compositor_with(|_| ())
    }

    /// Starts a compositor as [`Relayed::compositor`] does, with `prepared`
    /// done to it before it is joined.
    fn compositor_with(&self, prepared: impl FnOnce(&mut Session)) -> (Session, Holder) {
        let mut session = Session::new();
        prepared(&mut session);
        let (relays, compositor) = relay::pair().unwrap();
        let told = Told {
            relayed: true,
            ..Told::default()
        };
        let holder = hold_with(
            &mut session,
            told,
            &[compositor.as_fd(), self.socket.as_fd()],
        );
        relay::send(self.holder.as_fd(), &Hand::Compositor, &[relays.as_fd()]).unwrap();
        session.stepped.push(self.stepping());
        (session, holder)
    }

    /// What steps the relay once, without waiting.
    fn stepping(&self) -> Box<dyn FnMut()> {
        let relay = Rc::clone(&self.relay);
        Box::new(move || {
            relay
                .borrow_mut()
                .step(Some(Duration::ZERO), &mut std::io::sink());
        })
    }

    /// Steps the relay alone, as it is with no compositor.
    fn step(&self) {
        self.relay
            .borrow_mut()
            .step(Some(Duration::ZERO), &mut std::io::sink());
    }
}

#[test]
fn a_window_kept_by_the_relay_is_shown_by_the_next_compositor_as_it_was() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    assert_eq!(first.draw().at(10, 10), RED);

    client.take_events();

    // The compositor ends, and another is started in its place.
    drop((first, first_holder));
    relayed.step();
    relayed.step();
    assert_eq!(relayed.relay.borrow().programs(), 1, "the program kept");
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);

    // The program goes on as it did, none the wiser.
    second.roundtrip(&mut client);
    let told = client.take_events();
    assert_eq!(
        told[..2],
        [Event::KeyboardLeave, Event::PointerLeave],
        "told only that what it had left it, and came back: {told:?}"
    );
    assert!(told.contains(&Event::KeyboardEnter), "{told:?}");
    client.fill(&window.surface, (200, 100), GREEN);
    second.roundtrip(&mut client);
    second.roundtrip(&mut client);
    assert_eq!(second.draw().at(10, 10), GREEN);
}

/// A program with a window, kept through the end of `first`, and the
/// compositor joined after it, prepared by `next`, showing the window again.
fn restarted(
    relayed: &Relayed,
    first: impl FnOnce(&mut Session),
    next: impl FnOnce(&mut Session),
) -> (Session, Holder, Client) {
    let (mut session, holder) = relayed.compositor_with(first);
    let mut client = session.adopt(UnixStream::connect(&relayed.path).unwrap());
    session.open(&mut client, "app", (200, 100), RED);
    drop((session, holder));
    relayed.step();
    relayed.step();
    let (mut session, holder) = relayed.compositor_with(next);
    session.eventually(|session| session.draw().at(10, 10) == RED);
    session.roundtrip(&mut client);
    (session, holder, client)
}

#[test]
fn a_screen_new_to_the_program_is_offered_by_a_name_it_can_bind() {
    let relayed = Relayed::new();
    let (mut session, _holder, mut client) = restarted(
        &relayed,
        |_| (),
        |session| {
            session.add_output((640, 480));
        },
    );
    session.roundtrip(&mut client);
    assert_eq!(
        client.state.screens.len(),
        2,
        "told of the new one, and bound it"
    );

    // Live again, one plugged in and taken out is as the program knows it.
    let plugged = session.add_output((320, 200));
    session.roundtrip(&mut client);
    // Bound as it was offered, the binding is sent the next time.
    session.roundtrip(&mut client);
    assert_eq!(client.state.screens.len(), 3);
    session.compositor_mut().remove_output(&plugged);
    session.roundtrip(&mut client);
    assert!(!client.state.outputs_left.is_empty(), "told it is gone");
}

#[test]
fn a_program_bound_to_what_the_next_compositor_no_longer_offers_is_let_go() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor_with(|session| {
        session.add_output((640, 480));
    });
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.roundtrip(&mut client);
    assert_eq!(client.state.screens.len(), 2);
    drop((first, holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|_| relayed.relay.borrow().programs() == 0);
}

#[test]
fn what_the_compositor_before_offered_goes_nowhere_and_the_next_ones_offers_are_the_programs() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    client.copy("before");
    first.roundtrip(&mut client);
    first.roundtrip(&mut client);
    let stale = client.state.selection.clone().unwrap();
    drop((first, holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);

    // Asked of an offer of the compositor before, nothing comes; and it is
    // destroyed as the program likes.
    let (mut nothing, writer) = std::io::pipe().unwrap();
    stale.receive("text/plain;charset=utf-8".to_owned(), writer.as_fd());
    drop(writer);
    stale.destroy();
    second.roundtrip(&mut client);
    let mut said = String::new();
    nothing.read_to_string(&mut said).unwrap();
    assert_eq!(said, "");

    // What is copied now is offered, and pasted, as before.
    client.copy("after");
    second.roundtrip(&mut client);
    second.roundtrip(&mut client);
    let mut pasted = client.paste().unwrap();
    second.roundtrip(&mut client);
    second.roundtrip(&mut client);
    let mut text = String::new();
    pasted.read_to_string(&mut text).unwrap();
    assert_eq!(text, "after");
}
