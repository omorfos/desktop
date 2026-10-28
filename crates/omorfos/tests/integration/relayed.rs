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

use omorfos::{Input, Roles};
use omorfos_config::Action;
use omorfos_ipc::hold::{Told, Word};
use omorfos_ipc::relay::{self, Hand, Program, Started};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session, TEXT, key};
use crate::cursors::{color_at, pointer_at, theme};
use crate::hold::{Holder, hold_with};
use crate::holding::{nudge, pointed};
use crate::taskbar::{entry, listing};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

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
    /// What the relay has said, of programs let go.
    said: Rc<RefCell<Vec<u8>>>,
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
            said: Rc::default(),
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
        let (session, holder, _) = self.compositor_and_end(prepared);
        (session, holder)
    }

    /// Starts a compositor as [`Relayed::compositor_with`] does, and keeps
    /// a copy of its end of its connection to the relay, for a test to hand
    /// the relay what the compositor would. It is to be dropped before the
    /// compositor ends, for the relay to see it go.
    fn compositor_and_end(
        &self,
        prepared: impl FnOnce(&mut Session),
    ) -> (Session, Holder, OwnedFd) {
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
        (session, holder, compositor)
    }

    /// What steps the relay once, without waiting.
    fn stepping(&self) -> Box<dyn FnMut()> {
        let (relay, said) = (Rc::clone(&self.relay), Rc::clone(&self.said));
        Box::new(move || {
            relay
                .borrow_mut()
                .step(Some(Duration::ZERO), &mut *said.borrow_mut());
        })
    }

    /// Steps the relay alone, as it is with no compositor.
    fn step(&self) {
        self.relay
            .borrow_mut()
            .step(Some(Duration::ZERO), &mut *self.said.borrow_mut());
    }

    /// Checks that the one program connected was kept, and if not, says
    /// what the relay said of it.
    fn kept(&self) {
        let said = String::from_utf8_lossy(&self.said.borrow()).into_owned();
        assert_eq!(
            self.relay.borrow().programs(),
            1,
            "the program kept: {said}"
        );
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
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);

    // The program goes on as it did, none the wiser: the keyboard and the
    // pointer, back where they were, are not said to have left.
    second.roundtrip(&mut client);
    let told = client.take_events();
    let focus = |event: &Event| {
        matches!(
            event,
            Event::KeyboardLeave
                | Event::KeyboardEnter
                | Event::PointerLeave
                | Event::PointerEnter { .. }
        )
    };
    assert!(!told.iter().any(focus), "{told:?}");
    client.fill(&window.surface, (200, 100), GREEN);
    second.roundtrip(&mut client);
    second.roundtrip(&mut client);
    assert_eq!(second.draw().at(10, 10), GREEN);
}

#[test]
fn the_pointers_shape_is_named_under_the_enter_the_program_knows() {
    let relayed = Relayed::new();
    // The pointer over where the window is, in each compositor.
    let hovering = |session: &mut Session| {
        theme(session);
        let over = Input::Motion { x: 50.0, y: 50.0 };
        session.compositor_mut().input(over, 0);
    };
    let (mut first, first_holder) = relayed.compositor_with(hovering);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    first.roundtrip(&mut client);
    client.take_events();
    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor_with(hovering);
    second.eventually(|session| session.draw().at(100, 80) == RED);
    second.roundtrip(&mut client);
    let pointer = |event: &Event| matches!(event, Event::PointerLeave | Event::PointerEnter { .. });
    let told = client.take_events();
    assert!(
        !told.iter().any(pointer),
        "it keeps the enter it knows: {told:?}"
    );

    client.set_cursor_shape(Shape::Text);
    second.roundtrip(&mut client);
    assert_eq!(color_at(&mut second, 50, 50), BLUE);
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

#[test]
fn a_decorated_window_drawn_through_a_viewport_is_shown_again_as_it_was() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open_decorated(&mut client, "app", (200, 100), RED);
    client.watch_scale(&window.surface);
    client.fill_plain(&window.surface, (200, 100), GREEN);
    first.roundtrip(&mut client);
    first.roundtrip(&mut client);
    let (titlebar, inside) = ((20, 5), (20, 60));
    let before = first.draw();
    let framed = before.at(titlebar.0, titlebar.1);
    assert_eq!(before.at(inside.0, inside.1), GREEN);
    assert_ne!(framed, GREEN, "a titlebar");
    assert!(client.state.told.scale.is_some(), "told its scale");
    client.take_events();
    client.state.told.scale = None;

    drop((first, holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(inside.0, inside.1) == GREEN);
    assert_eq!(
        second.draw().at(titlebar.0, titlebar.1),
        framed,
        "framed as before"
    );
    second.roundtrip(&mut client);
    // Neither is told again what has not changed.
    let told = client.take_events();
    let decoration = told
        .iter()
        .any(|event| matches!(event, Event::Decoration { .. }));
    assert!(!decoration, "{told:?}");
    assert_eq!(client.state.told.scale, None);
}

#[test]
fn a_window_drawn_in_parts_is_shown_again_with_each_where_and_over_what_it_was() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    // A part drawn on its own, one drawn with it, shown only once it is
    // committed, and one drawn with the window and put under the first.
    let over = client.subsurface(&window.surface, (10, 10), (40, 40), GREEN);
    let inner = client.surface();
    client.part_of(&inner, &over).set_position(5, 5);
    client.fill(&inner, (10, 10), BLUE);
    over.commit();
    let under = client.surface();
    let below = client.part_of(&under, &window.surface);
    below.set_position(30, 30);
    below.place_below(&over);
    client.fill(&under, (40, 40), BLUE);
    window.surface.commit();
    first.roundtrip(&mut client);
    let parts = [
        (12, 12, GREEN),
        (18, 18, BLUE),
        (35, 35, GREEN),
        (60, 45, BLUE),
    ];
    let shown = |session: &mut Session| {
        let drawn = session.draw();
        parts.iter().all(|(x, y, color)| drawn.at(*x, *y) == *color) && drawn.at(150, 50) == RED
    };
    assert!(shown(&mut first), "drawn in parts");
    drop((first, holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(shown);
}

#[test]
fn a_program_that_named_the_pointers_shape_names_it_again_once_the_pointer_is_back() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor_with(theme);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    assert_eq!(pointer_at(&mut first, 50, 50), GREEN, "the ordinary one");
    first.roundtrip(&mut client);
    client.set_cursor_shape(Shape::Text);
    first.roundtrip(&mut client);
    assert_eq!(color_at(&mut first, 50, 50), BLUE);
    drop((first, holder));
    relayed.step();
    relayed.step();
    relayed.kept();

    // Told the pointer has come back, it says again what it is to look
    // like, as it would to any compositor.
    let (mut second, _holder) = relayed.compositor_with(theme);
    second.eventually(|session| session.draw().at(150, 90) == RED);
    pointer_at(&mut second, 50, 50);
    second.roundtrip(&mut client);
    let told = client.take_events();
    let back = |event: &Event| matches!(event, Event::PointerEnter { .. });
    assert!(told.iter().any(back), "{told:?}");
    client.set_cursor_shape(Shape::Text);
    second.roundtrip(&mut client);
    assert_eq!(color_at(&mut second, 50, 50), BLUE);
}

#[test]
fn a_program_watching_how_large_a_screen_is_is_not_told_again_what_has_not_changed() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    client.watch_screen_size();
    first.roundtrip(&mut client);
    let sized = |event: &Event| matches!(event, Event::ScreenSize(..));
    assert!(client.take_events().iter().any(sized), "told its size");
    first.open(&mut client, "app", (200, 100), RED);
    drop((first, holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    let told = client.take_events();
    assert!(!told.iter().any(sized), "{told:?}");
}

#[test]
fn a_field_is_told_what_is_typed_into_it_by_its_own_count_of_what_it_said() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    // A field is entered only while there is an input method to type in it.
    let mut typist = first.connect_as(Roles::INPUT);
    let _method = typist.input_method();
    first.roundtrip(&mut typist);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let field = client.text_input();
    first.open(&mut client, "app", (200, 100), RED);
    assert!(client.take_events().contains(&Event::TextEnter));
    // Enabled, then moved: two commits.
    field.enable();
    field.set_cursor_rectangle(10, 10, 2, 16);
    field.commit();
    field.set_cursor_rectangle(20, 10, 2, 16);
    field.commit();
    first.roundtrip(&mut client);
    drop((first, holder, typist));
    relayed.step();
    relayed.step();
    relayed.kept();

    let (mut second, _holder) = relayed.compositor();
    let mut typist = second.connect_as(Roles::INPUT);
    let method = typist.input_method();
    second.roundtrip(&mut typist);
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    let told = client.take_events();
    let left = told.iter().position(|event| *event == Event::TextLeave);
    let entered = told.iter().position(|event| *event == Event::TextEnter);
    assert!(
        left.zip(entered)
            .is_some_and(|(left, entered)| left < entered),
        "{told:?}"
    );
    // Back in the field, the program enables it again, as on any enter.
    field.enable();
    field.set_cursor_rectangle(20, 10, 2, 16);
    field.commit();
    second.roundtrip(&mut client);
    second.roundtrip(&mut typist);
    typist.type_text(&method, "日本");
    second.roundtrip(&mut typist);
    second.roundtrip(&mut client);
    let typed = client.take_events();
    assert!(
        typed.contains(&Event::TextCommitted("日本".to_owned())),
        "{typed:?}"
    );
    assert!(
        typed.contains(&Event::TextDone(3)),
        "three commits: {typed:?}"
    );
}

#[test]
fn a_window_that_said_where_its_edges_are_is_shown_again_by_them() {
    let relayed = Relayed::new();
    let (mut first, holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    // The window proper is the middle of a larger surface, as it is for an
    // application that draws a shadow around itself, as `foot` does.
    let window = client.toplevel("app", "app");
    first.roundtrip(&mut client);
    window.xdg_surface.set_window_geometry(30, 20, 200, 100);
    client.fill(&window.surface, (260, 160), RED);
    first.roundtrip(&mut client);
    first.roundtrip(&mut client);
    let placed = |session: &mut Session| {
        let drawn = session.draw();
        drawn.at(229, 139) == RED && drawn.at(231, 141) == BACKGROUND
    };
    assert!(placed(&mut first), "its shadow off the screen");
    drop((first, holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(placed);
    second.roundtrip(&mut client);
    relayed.kept();
}

#[test]
fn a_program_with_a_menu_open_is_kept_and_told_the_menu_is_done() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    first.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    first.roundtrip(&mut client);
    client.take_events();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    assert!(
        client.take_events().contains(&Event::PopupDone),
        "the menu is over, as the compositor before closed it"
    );

    // Put away and shown again in the same surface, as a toolkit does, it
    // is a menu of the next compositor's.
    surface.attach(None, 0, 0);
    surface.commit();
    popup.destroy();
    let (_popup, _positioner) = client.popup_on(&surface, &window, (10, 20), (50, 30));
    second.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .iter()
            .any(|event| matches!(event, Event::PopupConfigure { .. })),
        "configured as a menu"
    );
    client.fill(&surface, (50, 30), BLUE);
    second.roundtrip(&mut client);
    assert_eq!(client.error(), None);
    second.eventually(|session| session.draw().at(15, 25) == BLUE);
}

#[test]
fn a_program_told_of_a_swipe_under_way_is_kept_and_told_it_was_cancelled() {
    use omorfos::{Gesture, Input};
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    let _gestures = client.gestures();
    first
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    first.roundtrip(&mut client);
    let begun = Gesture::SwipeBegin { fingers: 4 };
    first.compositor_mut().input(Input::Gesture(begun), 0);
    first.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Gesture(begun)));

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    let told = client.take_events();
    assert!(
        told.contains(&Event::Gesture(Gesture::SwipeEnd { cancelled: true })),
        "{told:?}"
    );

    // Gestures go on being told.
    second
        .compositor_mut()
        .input(Input::Motion { x: 60.0, y: 50.0 }, 0);
    let pinch = Gesture::PinchBegin { fingers: 2 };
    second.compositor_mut().input(Input::Gesture(pinch), 0);
    second.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Gesture(pinch)));
}

/// Puts the pointer over where a window opens first.
fn hovering(session: &mut Session) {
    let over = Input::Motion { x: 50.0, y: 50.0 };
    session.compositor_mut().input(over, 0);
}

#[test]
fn a_program_holding_the_pointer_holds_it_again_once_the_next_compositor_is_up() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor_with(hovering);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let game = first.open(&mut client, "game", (200, 100), RED);
    let _relative = client.relative_pointer();
    let all = client.region((0, 0, 200, 100));
    let _lock = client.lock_pointer(&game.surface, Some(&all));
    all.destroy();
    first.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::PointerLocked));

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor_with(hovering);
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    let told = client.take_events();
    let held = |event: &Event| matches!(event, Event::PointerLocked | Event::PointerUnlocked);
    let held: Vec<&Event> = told.iter().filter(|event| held(event)).collect();
    assert_eq!(
        held,
        [&Event::PointerUnlocked, &Event::PointerLocked],
        "let go of as the compositor went, and held again: {told:?}"
    );

    // Held, the pointer stays, and how far it moved is told.
    let told = nudge(&mut second, &mut client, 10.0, 10.0);
    let moved = |event: &Event| {
        matches!(
            event,
            Event::RelativeMotion {
                by: (10.0, 10.0),
                ..
            }
        )
    };
    assert!(told.iter().any(moved), "{told:?}");
    assert_eq!(pointed(&told), None);
    assert_eq!(client.error(), None);
}

#[test]
fn a_program_that_held_the_pointer_once_is_not_held_again_and_may_ask_anew() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor_with(hovering);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let game = first.open(&mut client, "game", (200, 100), RED);
    let lock = client.lock_pointer_once(&game.surface);
    first.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::PointerLocked));

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor_with(hovering);
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    let told = client.take_events();
    assert!(told.contains(&Event::PointerUnlocked), "{told:?}");
    assert!(!told.contains(&Event::PointerLocked), "{told:?}");
    let told = nudge(&mut second, &mut client, 10.0, 10.0);
    assert_eq!(pointed(&told), Some((60.0, 60.0)), "the user has it");

    // Over, it is let go of, and the program asks again.
    lock.destroy();
    let _again = client.lock_pointer_once(&game.surface);
    second.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::PointerLocked));
    assert_eq!(client.error(), None);
}

#[test]
fn a_program_told_the_user_is_away_is_told_they_are_back_and_away_again_by_the_next() {
    use crate::idle::{AWAY, SOON, idles_within};
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    first.roundtrip(&mut client);
    client.watch_idle(AWAY);
    assert!(idles_within(&mut first, &mut client, SOON));

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    // The next compositor counts from when it started, so the program is
    // not left thinking the user away while they may be back.
    let told = client.take_events();
    let back = told.iter().position(|event| *event == Event::Back);
    assert!(back.is_some(), "{told:?}");
    let after = &told[back.unwrap()..];
    // Said already, on a machine slow to draw.
    let again = after.contains(&Event::Idle) || idles_within(&mut second, &mut client, SOON);
    assert!(again, "away again");
    assert_eq!(client.error(), None);
}

#[test]
fn a_program_typing_with_a_keyboard_of_its_own_types_with_it_through_the_next() {
    let relayed = Relayed::new();
    let (mut first, first_holder, towards) = relayed.compositor_and_end(|_| ());
    // A program with the role to type, as the compositor starts it.
    let (part, ours) = UnixStream::pair().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: Roles::INPUT.bits(),
    };
    relay::send(towards.as_fd(), &Hand::Started(started), &[part.as_fd()]).unwrap();
    drop((part, towards));
    let mut typist = first.adopt(ours);
    let keyboard = typist.virtual_keyboard();
    let mut app = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut app, "app", (200, 100), RED);
    first.roundtrip(&mut typist);
    assert_eq!(typist.error(), None);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    assert_eq!(relayed.relay.borrow().programs(), 2, "both kept");
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut app);
    app.take_events();

    // Its keymap is the next compositor's too.
    let a = key::A.raw() - 8;
    keyboard.key(0, a, 1);
    keyboard.key(0, a, 0);
    second.roundtrip(&mut typist);
    second.roundtrip(&mut app);
    assert_eq!(typist.error(), None);
    let told = app.take_events();
    let typed = Event::Key {
        code: a,
        pressed: true,
    };
    assert!(told.contains(&typed), "{told:?}");
}

#[test]
fn a_taskbar_kept_hears_of_a_window_kept_once_and_that_one_not_kept_has_closed() {
    let relayed = Relayed::new();
    let (mut first, first_holder, towards) = relayed.compositor_and_end(|_| ());
    // A taskbar, as the compositor starts it, with the role to list windows.
    let (part, ours) = UnixStream::pair().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: Roles::PANEL.bits(),
    };
    relay::send(towards.as_fd(), &Hand::Started(started), &[part.as_fd()]).unwrap();
    drop((part, towards));
    let mut taskbar = first.adopt(ours);
    let mut app = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut app, "kept", (200, 100), RED);
    // One not through the relay, which goes with the compositor.
    let mut alone = first.connect();
    first.open(&mut alone, "alone", (100, 100), GREEN);
    first.roundtrip(&mut taskbar);
    let both = [entry("kept", "kept window"), entry("alone", "alone window")];
    assert_eq!(listing(&taskbar), both);

    drop((first, first_holder, alone));
    relayed.step();
    relayed.step();
    assert_eq!(relayed.relay.borrow().programs(), 2, "both kept");
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut taskbar);
    assert_eq!(listing(&taskbar), [entry("kept", "kept window")]);
    assert_eq!(taskbar.state.taskbar.len(), 2, "told of each once");

    // What the window says is told under the handle the taskbar has, and a
    // window new to it is new.
    window.role.set_title("renamed".to_owned());
    second.roundtrip(&mut app);
    let mut more = second.connect();
    second.open(&mut more, "new", (100, 100), BLUE);
    second.roundtrip(&mut taskbar);
    let now = [entry("kept", "renamed"), entry("new", "new window")];
    assert_eq!(listing(&taskbar), now);
    window.role.destroy();
    second.roundtrip(&mut app);
    second.roundtrip(&mut taskbar);
    assert_eq!(listing(&taskbar), [entry("new", "new window")]);
    assert_eq!(taskbar.error(), None);
}

#[test]
fn a_program_waiting_to_hear_a_picture_was_shown_is_kept_and_told_it_was_not() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    let _feedback = client.feedback(&window.surface);
    client.fill(&window.surface, (200, 100), GREEN);
    first.roundtrip(&mut client);
    client.take_events();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == GREEN);
    second.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Discarded));
    assert_eq!(client.error(), None);
}

#[test]
fn a_program_holding_a_token_to_hand_focus_on_is_kept() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    let token = client.request_token(false);
    first.roundtrip(&mut client);
    assert!(client.state.token.take().is_some());

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    // Let go of, it is the relay's to answer for.
    token.destroy();
    second.roundtrip(&mut client);
    let _next = client.request_token(false);
    second.roundtrip(&mut client);
    assert!(
        client.state.token.is_some(),
        "the next compositor gives one"
    );
    assert_eq!(client.error(), None);
}

#[test]
fn a_program_that_keeps_the_screen_awake_gives_its_window_an_icon_and_shares_it_is_kept() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    let _inhibitor = client.inhibit_idle(&window.surface);
    let (_icons, icon) = client.icon();
    icon.set_name("utilities-terminal".to_owned());
    let _exported = client.export(&window);
    first.roundtrip(&mut client);
    assert!(client.state.exported.take().is_some());

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    assert!(
        client.state.exported.is_some(),
        "told the next compositor's handle"
    );
    assert_eq!(client.error(), None);
}

#[test]
fn a_window_given_an_icon_drawn_in_buffers_made_after_it_is_kept() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut client, "app", (200, 100), RED);
    // As Qt gives one: the icon made, then what it is drawn in, and both
    // destroyed once it is given, the icon first.
    let (icons, icon) = client.icon();
    let buffer = client.buffer((32, 32), BLUE);
    icon.add_buffer(&buffer, 1);
    icons.set_icon(&window.role, Some(&icon));
    icon.destroy();
    buffer.destroy();
    window.surface.commit();
    first.roundtrip(&mut client);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    second.roundtrip(&mut client);
    relayed.kept();
    assert_eq!(client.error(), None);
}

#[test]
fn a_dialog_of_another_programs_window_belongs_to_it_again_once_both_are_kept() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut player = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = first.open(&mut player, "player", (200, 100), RED);
    first.compositor_mut().perform(&Action::Fullscreen);
    let _shared = player.export(&window);
    first.roundtrip(&mut player);
    let handle = player.state.exported.take().unwrap();
    // A portal, through the relay too, makes its dialog belong to the window,
    // and shows it only once the compositor has been started anew.
    let mut portal = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let dialog = portal.toplevel_uncommitted("portal", "choose a file");
    let _imported = portal.import_parent_of(&handle, &dialog);
    dialog.surface.commit();
    first.roundtrip(&mut portal);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    assert_eq!(relayed.relay.borrow().programs(), 2, "both kept");
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut player);
    assert!(
        player.state.exported.is_some(),
        "told the next compositor's handle"
    );
    second.roundtrip(&mut portal);
    portal.fill(&dialog.surface, (100, 50), BLUE);
    second.roundtrip(&mut portal);
    second.roundtrip(&mut portal);
    let desktop = second.compositor().desktop();
    let shown = (desktop.active_workspace(), desktop.frames().count());
    assert_eq!(shown, (1, 2), "with it, over its fullscreen window");
    assert!(!portal.take_events().contains(&Event::ImportGone));
    assert_eq!(portal.error(), None);
}

#[test]
fn a_window_imported_from_a_program_not_kept_is_gone_once_the_compositor_is_started_anew() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    // Shared by a program not through the relay, which goes with the
    // compositor.
    let mut alone = first.connect();
    let window = first.open(&mut alone, "alone", (200, 100), RED);
    let _shared = alone.export(&window);
    first.roundtrip(&mut alone);
    let handle = alone.state.exported.take().unwrap();
    let mut portal = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let dialog = first.open(&mut portal, "portal", (100, 50), BLUE);
    let _imported = portal.import_parent_of(&handle, &dialog);
    first.roundtrip(&mut portal);
    assert!(!portal.take_events().contains(&Event::ImportGone));

    drop((first, first_holder, alone));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.compositor().desktop().frames().count() == 1);
    second.roundtrip(&mut portal);
    assert!(portal.take_events().contains(&Event::ImportGone));
    assert_eq!(portal.error(), None);
}

#[test]
fn a_program_holding_a_buffer_in_the_memory_of_the_card_is_kept() {
    use smithay::backend::allocator::{Fourcc, Modifier};
    let relayed = Relayed::new();
    let offered = |session: &mut Session| {
        session
            .compositor_mut()
            .offer_gpu_buffers(0, crate::gpu::drawable())
            .unwrap();
    };
    let (mut first, first_holder) = relayed.compositor_with(offered);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "app", (200, 100), RED);
    let memory = tempfile::tempfile().unwrap();
    memory.set_len(16 * 16 * 4).unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(Modifier::Linear));
    let buffer = client.gpu_buffer(memory.as_fd(), ((16, 16), 64), format);
    first.roundtrip(&mut client);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor_with(offered);
    second.eventually(|session| session.draw().at(10, 10) == RED);
    second.roundtrip(&mut client);
    assert!(!client.take_events().contains(&Event::GpuBufferFailed));
    buffer.destroy();
    second.roundtrip(&mut client);
    assert_eq!(client.error(), None);
}

#[test]
#[ignore = "needs a graphics card this user may use: run with --include-ignored"]
fn a_window_drawn_in_the_memory_of_the_card_is_shown_again_by_the_next_compositor() {
    use smithay::backend::allocator::Fourcc;
    use smithay::reexports::gbm;
    let (card, number) = crate::common::graphics_card();
    let relayed = Relayed::new();
    let offered = |session: &mut Session| {
        session
            .compositor_mut()
            .offer_gpu_buffers(number, crate::gpu::drawable())
            .unwrap();
    };
    let (mut first, first_holder) = relayed.compositor_with(offered);
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let window = client.toplevel("drawn", "drawn on the card");
    first.roundtrip(&mut client);
    let device = gbm::Device::new(card).unwrap();
    let flags = gbm::BufferObjectFlags::LINEAR | gbm::BufferObjectFlags::RENDERING;
    let mut picture = device
        .create_buffer_object::<()>(64, 32, gbm::Format::Xrgb8888, flags)
        .unwrap();
    picture
        .map_mut(0, 0, 64, 32, |mapped| {
            let row = mapped.stride() as usize;
            for line in mapped.buffer_mut().chunks_mut(row) {
                for pixel in line[..64 * 4].as_chunks_mut::<4>().0 {
                    *pixel = GREEN.to_le_bytes();
                }
            }
        })
        .unwrap();
    let file = picture.fd().unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(picture.modifier()));
    let buffer = client.gpu_buffer(file.as_fd(), ((64, 32), picture.stride()), format);
    window.surface.attach(Some(&buffer), 0, 0);
    window.surface.damage_buffer(0, 0, 64, 32);
    window.surface.commit();
    first.roundtrip(&mut client);
    first.roundtrip(&mut client);
    assert_eq!(first.draw().at(10, 10), GREEN);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    relayed.kept();
    let (mut second, _holder) = relayed.compositor_with(offered);
    second.eventually(|session| session.draw().at(10, 10) == GREEN);
}

#[test]
fn a_part_handed_back_by_the_relay_has_the_roles_its_compositor_gave_it() {
    let relayed = Relayed::new();
    let (mut session, _holder, towards) = relayed.compositor_and_end(|_| ());
    // What the relay does not know, from the compositor, is passed over.
    relay::send(towards.as_fd(), &Hand::Kept, &[]).unwrap();
    let (part, ours) = UnixStream::pair().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: Roles::PANEL.bits(),
    };
    relay::send(towards.as_fd(), &Hand::Started(started), &[part.as_fd()]).unwrap();
    drop((part, towards));
    let mut panel = session.adopt(ours);
    let strip = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    let _held = panel.layer(&strip);
    session.roundtrip(&mut panel);
    session.roundtrip(&mut panel);
    let told = panel.take_events();
    let shown = told
        .iter()
        .any(|event| matches!(event, Event::LayerConfigure { .. }));
    assert!(shown && !told.contains(&Event::LayerClosed), "{told:?}");
}

#[test]
fn what_was_copied_is_there_again_from_the_keeper_kept_through_a_restart() {
    let relayed = Relayed::new();
    let (mut first, first_holder, towards) = relayed.compositor_and_end(|_| ());
    // The keeper, connected as the compositor starts it: its connection
    // handed to the relay, with the role of keeping the clipboard.
    let directory = tempfile::TempDir::new().unwrap();
    let listener = UnixListener::bind(directory.path().join("wayland-keeper")).unwrap();
    let runtime = directory.path().to_owned();
    let keeper = std::thread::spawn(move || {
        let variable = |variable: &str| match variable {
            "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
            "WAYLAND_DISPLAY" => Some("wayland-keeper".into()),
            _ => None,
        };
        let limits = omorfos_clipboard::Limits::default();
        crate::keeper::said(omorfos_clipboard::start_with(&variable, limits))
    });
    let (part, _) = listener.accept().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: Roles::CLIPBOARD.bits(),
    };
    relay::send(towards.as_fd(), &Hand::Started(started), &[part.as_fd()]).unwrap();
    drop((part, towards));

    // A program copies, the keeper takes it, and the program closes.
    let mut editor = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut editor, "editor", (200, 100), RED);
    editor.copy_as("kept", &[TEXT]);
    first.eventually(|session| {
        session.roundtrip(&mut editor);
        editor.state.replaced
    });
    drop(editor);

    // The compositor ends, and the next has it on the clipboard at once.
    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    let pasted = |session: &mut Session| {
        let mut terminal = session.connect();
        session.open(&mut terminal, "terminal", (200, 100), BLUE);
        let pipe = terminal.paste();
        session.roundtrip(&mut terminal);
        pipe.map(|mut pipe| {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    };
    second.eventually(|session| pasted(session).as_deref() == Some("kept"));
    drop(second);
    assert!(!keeper.is_finished() || keeper.join().is_ok());
}

/// The stand-in for a program that runs for the whole session as the keeper
/// of `session`, which is started once the compositor has been joined to the
/// relay, as the backend does.
fn keeping(session: &mut Session) {
    let component =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/component");
    session.compositor_mut().set_keeper(component);
}

#[test]
fn the_keeper_is_started_through_the_relay_and_not_again_by_the_next_compositor() {
    let relayed = Relayed::new();
    // The stand-in leaves its marks beside the socket for scripts, which a
    // compositor joined to the relay serves beside the relay's.
    let beside = relayed.path.parent().unwrap().to_owned();
    std::fs::write(beside.join("component-does"), "stay").unwrap();
    let started = || std::fs::read_to_string(beside.join("component-started")).unwrap_or_default();
    let (mut first, first_holder) = relayed.compositor_with(keeping);
    first.compositor_mut().autostart();
    // Once the relay has said it kept nothing, through the relay.
    // Made, then written to: read only once it says the process.
    let process = || {
        let said = std::fs::read_to_string(beside.join("component-process")).ok();
        said.filter(|said| !said.is_empty())
    };
    first.eventually(|_| {
        started() == "x" && process().is_some() && relayed.relay.borrow().programs() == 1
    });
    let process = process().unwrap();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor_with(keeping);
    second.compositor_mut().autostart();
    for _ in 0..50 {
        second.step();
    }
    assert_eq!(started(), "x", "the one kept is the keeper");
    assert_eq!(relayed.relay.borrow().programs(), 1);
    let _ = Command::new("kill").arg(process).status();
}

/// What the compositor of `session` says the session is, over its socket
/// for scripts.
fn state(session: &mut Session) -> omorfos_ipc::State {
    let environment = session.compositor().session_environment();
    let mut socket = environment.into_iter();
    let (_, ipc) = socket.find(|(name, _)| *name == "OMORFOS_SOCKET").unwrap();
    let mut peer = omorfos_ipc::Client::connect(std::path::Path::new(&ipc)).unwrap();
    match session.ask(&mut peer, &omorfos_ipc::Request::State) {
        omorfos_ipc::Reply::State(state) => state,
        other => unreachable!("{other:?}"),
    }
}

#[test]
fn a_window_comes_back_where_and_as_it_was_and_the_session_as_it_was() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "kept", (200, 100), RED);
    // On the second desktop, shown, and kept above the rest.
    let second_desktop = omorfos_config::WorkspaceNumber::clamped(2);
    for action in [
        Action::MoveToWorkspace(second_desktop),
        Action::Workspace(second_desktop),
        Action::KeepAbove,
    ] {
        first.compositor_mut().perform(&action);
    }
    first.step();
    let before = state(&mut first);
    let window = |state: &omorfos_ipc::State| {
        let window = state.windows.first();
        window.map(|window| (window.workspace, window.standing, window.focused))
    };
    let shown = |state: &omorfos_ipc::State| {
        let shown = state.workspaces.iter().filter(|workspace| workspace.active);
        shown
            .map(|workspace| workspace.number)
            .collect::<Vec<u32>>()
    };
    assert_eq!(shown(&before), [2]);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| !state(session).windows.is_empty());
    let after = state(&mut second);
    assert_eq!(window(&after), window(&before));
    assert_eq!(shown(&after), [2], "the desktop it showed");
}

#[test]
fn windows_that_shared_a_workspace_share_it_again_as_it_was_cut() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open(&mut client, "left", (200, 100), RED);
    first
        .compositor_mut()
        .perform(&Action::Tile(omorfos_config::Half::Left));
    first.open(&mut client, "right", (200, 100), BLUE);
    first.step();
    let parts = |session: &Session| {
        let frames = session.compositor().desktop().frames();
        let part = |frame: omorfos_core::Frame<'_>| (frame.window().app_id.clone(), frame.geometry);
        let mut parts: Vec<_> = frames.map(part).collect();
        parts.sort_by_key(|(app, _)| app.clone());
        parts
    };
    let before = parts(&first);
    assert_eq!(before.len(), 2, "both shown, sharing the workspace");
    let lines = first.compositor().desktop().lines(0);
    assert_eq!(lines.len(), 1);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| parts(session).len() == 2);
    assert_eq!(parts(&second), before, "each in the part it had");
    assert_eq!(
        second.compositor().desktop().lines(0),
        lines,
        "cut as it was"
    );
}

#[test]
fn windows_that_shared_a_frame_share_it_again_in_their_order() {
    let relayed = Relayed::new();
    let (mut first, first_holder) = relayed.compositor();
    let mut client = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    first.open_decorated(&mut client, "editor", (200, 100), RED);
    first.open_decorated(&mut client, "terminal", (200, 100), BLUE);
    // The terminal's tab carried to the editor's titlebar, and the editor
    // shown again.
    crate::dragging::drag(&mut first, crate::dragging::MIDDLE, (100, 40), (100, 10));
    first.roundtrip(&mut client);
    first.compositor_mut().perform(&Action::PreviousTab);
    first.roundtrip(&mut client);
    let tabs = |session: &Session| {
        let frames = session.compositor().desktop().frames();
        let tabs = |frame: omorfos_core::Frame<'_>| {
            let apps = frame.tabs.iter().map(|tab| tab.app_id.clone());
            (apps.collect::<Vec<_>>(), frame.window().app_id.clone())
        };
        frames.map(tabs).collect::<Vec<_>>()
    };
    let before = tabs(&first);
    assert_eq!(before.len(), 1, "one frame");
    assert_eq!(before[0].0.len(), 2, "with both");
    assert_eq!(before[0].1, "editor");

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| {
        tabs(session)
            .first()
            .is_some_and(|(apps, _)| apps.len() == 2)
    });
    assert_eq!(tabs(&second), before, "in their order, the one shown shown");
    assert_eq!(second.draw().at(100, 60), RED);
}

#[test]
fn a_program_in_a_sandbox_is_kept_and_stays_in_it() {
    let relayed = Relayed::new();
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("sandbox");
    let (mut first, first_holder) = relayed.compositor();
    // Flatpak makes the sandbox through the relay, and goes.
    let mut flatpak = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    let open = flatpak.sandbox(&path);
    first.roundtrip(&mut flatpak);
    drop(flatpak);
    let shown = |client: &Client, name: &str| client.state.offered.iter().any(|it| it == name);
    let inside = |session: &mut Session| {
        let client = session.adopt(UnixStream::connect(&path).unwrap());
        let withheld = crate::roles::WITHHELD
            .iter()
            .all(|name| !shown(&client, name));
        assert!(withheld, "in the sandbox");
        assert!(shown(&client, "xdg_wm_base"));
        client
    };
    let mut app = inside(&mut first);
    first.open(&mut app, "app", (200, 100), GREEN);
    assert_eq!(first.draw().at(10, 10), GREEN);

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(10, 10) == GREEN);
    // Still in it, as the next compositor offers it nothing more, and what
    // comes in that way still is.
    second.roundtrip(&mut app);
    let withheld = crate::roles::WITHHELD.iter().all(|name| !shown(&app, name));
    assert!(withheld, "still in the sandbox");
    let _other = inside(&mut second);

    // When the sandbox ends, nothing more can come in that way.
    drop(open);
    second.eventually(|_| UnixStream::connect(&path).is_err());
}

#[test]
fn past_as_many_sandboxes_as_are_listened_on_one_made_is_let_go() {
    let relayed = Relayed::new();
    let directory = tempfile::TempDir::new().unwrap();
    let listener = UnixListener::bind(directory.path().join("sandbox")).unwrap();
    let (_open, closing) = UnixStream::pair().unwrap();
    let (mut first, _holder) = relayed.compositor();
    let mut flatpak = first.adopt(UnixStream::connect(&relayed.path).unwrap());
    for _ in 0..=64 {
        flatpak.sandbox_of(&listener, &closing);
    }
    first.roundtrip(&mut flatpak);
    first.step();
    let said = String::from_utf8_lossy(&relayed.said.borrow()).into_owned();
    let let_go = said.matches("a sandbox was let go, as 64 are listened on already");
    assert_eq!(let_go.count(), 1, "{said}");
}

#[test]
fn a_panel_kept_by_the_relay_is_on_its_edge_again_after_a_restart() {
    let relayed = Relayed::new();
    let (mut first, first_holder, towards) = relayed.compositor_and_end(|_| ());
    let (part, ours) = UnixStream::pair().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: Roles::PANEL.bits(),
    };
    relay::send(towards.as_fd(), &Hand::Started(started), &[part.as_fd()]).unwrap();
    drop((part, towards));
    let mut panel = first.adopt(ours);
    let strip = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let _held = first.open_layer(&mut panel, &strip, (crate::common::WIDTH, 30), BLUE);
    assert_eq!(first.draw().at(5, 5), BLUE);
    first.roundtrip(&mut panel);
    panel.take_events();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor();
    second.eventually(|session| session.draw().at(5, 5) == BLUE);
    // Windows keep clear of it as before, and it is told nothing new.
    let mut client = second.connect();
    second.open(&mut client, "app", (200, 100), RED);
    assert_eq!(second.draw().at(5, 35), RED);
    second.roundtrip(&mut panel);
    let told = panel.take_events();
    let configured = told
        .iter()
        .any(|event| matches!(event, Event::LayerConfigure { .. } | Event::LayerClosed));
    assert!(!configured, "{told:?}");
}

#[test]
fn a_program_started_with_a_role_is_kept_and_not_taken_for_the_keeper() {
    let relayed = Relayed::new();
    let beside = relayed.path.parent().unwrap().to_owned();
    std::fs::write(beside.join("component-does"), "stay").unwrap();
    let started = || std::fs::read_to_string(beside.join("component-started")).unwrap_or_default();
    // Made, then written to: read only once it says the process.
    let process = || {
        let said = std::fs::read_to_string(beside.join("component-process")).ok();
        said.filter(|said| !said.is_empty())
    };
    // The stand-in, started by the configuration with the role the keeper
    // has.
    let component =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/component");
    let config = format!(
        "autostart:\n  - {{ run: ['{}'], may: [clipboard] }}\n",
        component.display()
    );
    let (mut first, first_holder) = relayed.compositor_with(|session| {
        session.write_config("config.yaml", &config);
        let reload = Action::ReloadConfig;
        session.compositor_mut().perform(&reload);
    });
    first.compositor_mut().autostart();
    first.eventually(|_| process().is_some() && relayed.relay.borrow().programs() == 1);
    let users = process().unwrap();
    std::fs::remove_file(beside.join("component-process")).unwrap();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    assert_eq!(relayed.relay.borrow().programs(), 1, "kept");
    let (mut second, _holder) = relayed.compositor_with(keeping);
    second.compositor_mut().autostart();
    // The keeper is started, as the one kept is not it.
    second.eventually(|_| started() == "xx" && process().is_some());
    let keeper = process().unwrap();
    for process in [users, keeper] {
        let _ = Command::new("kill").arg(process).status();
    }
}

/// The stand-in for a program that runs for the whole session as the panel
/// of `session`.
fn panelled(session: &mut Session) {
    let component =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/component");
    session.compositor_mut().set_panel(component);
}

#[test]
fn a_component_kept_is_known_again_and_started_again_if_it_stops() {
    let relayed = Relayed::new();
    let beside = relayed.path.parent().unwrap().to_owned();
    std::fs::write(beside.join("component-does"), "stay").unwrap();
    let started = || std::fs::read_to_string(beside.join("component-started")).unwrap_or_default();
    // Made, then written to: read only once it says the process.
    let process = || {
        let said = std::fs::read_to_string(beside.join("component-process")).ok();
        said.filter(|said| !said.is_empty())
    };
    let (mut first, first_holder) = relayed.compositor_with(panelled);
    first.compositor_mut().autostart();
    first.eventually(|_| process().is_some() && relayed.relay.borrow().programs() == 1);
    let kept = process().unwrap();
    // Said to the relay as the panel, once there is something to say.
    first.step();

    drop((first, first_holder));
    relayed.step();
    relayed.step();
    let (mut second, _holder) = relayed.compositor_with(panelled);
    second.compositor_mut().autostart();
    for _ in 0..50 {
        second.step();
    }
    assert_eq!(started(), "x", "known again, and not started again");

    // It stops, as one that crashed does: it is started again, as one the
    // compositor started itself would be.
    std::fs::remove_file(beside.join("component-process")).unwrap();
    let _ = Command::new("kill").arg(&kept).status();
    second.eventually(|_| started() == "xx" && process().is_some());
    let _ = Command::new("kill").arg(process().unwrap()).status();
}
