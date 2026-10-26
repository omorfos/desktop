//! The compositor behind the relay (ADR 61): it serves the programs the
//! relay hands it, knows each by the process the relay says it is, and
//! steps down when the relay goes.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command};

use omorfos_ipc::hold::{Told, Word};
use omorfos_ipc::relay::{self, Hand, Program};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session};
use crate::hold::{Holder, hold_with};
use crate::windows::RED;

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
