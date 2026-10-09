//! Server-side decorations: negotiation and how windows sit in their frames.

use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode;
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use crate::common::{Event, HEIGHT, Session, WIDTH};
use crate::windows::{BACKGROUND, RED};

/// The default theme's frame: a 1 pixel border and a 28 pixel titlebar.
const BORDER: i32 = 1;
const TITLEBAR: i32 = 28;

#[test]
fn a_window_that_asks_is_told_the_compositor_decorates_it() {
    let mut session = Session::new();
    let mut client = session.connect();

    session.open_decorated(&mut client, "foot", (200, 100), RED);

    let events = client.take_events();
    assert!(
        events.contains(&Event::Decoration { server_side: true }),
        "{events:?}"
    );
    let frame = session.compositor.desktop().frames().next().unwrap();
    assert!(frame.decoration.is_some());
    assert_eq!(frame.geometry.size.width, 200 + 2 * BORDER.unsigned_abs());
    assert_eq!(
        frame.geometry.size.height,
        100 + (TITLEBAR + BORDER).unsigned_abs()
    );
}

#[test]
fn a_window_that_does_not_ask_is_left_undecorated() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "gtk", (200, 100), RED);
    let frame = session.compositor.desktop().frames().next().unwrap();
    assert_eq!(frame.decoration, None);
    assert_eq!(session.draw().at(0, 0), RED);
}

#[test]
fn a_decorated_window_is_drawn_inside_its_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "foot", (200, 100), RED);

    let picture = session.draw();
    assert_eq!(
        picture.at(BORDER, TITLEBAR),
        RED,
        "the window starts below the titlebar"
    );
    assert_eq!(picture.at(BORDER + 199, TITLEBAR + 99), RED);
    assert_eq!(picture.at(BORDER + 210, TITLEBAR + 50), BACKGROUND);
}

#[test]
fn the_compositor_insists_on_decorating_whatever_mode_is_requested() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel_uncommitted("picky", "");
    let decoration = client.decoration(&window);
    decoration.set_mode(Mode::ClientSide);
    window.surface.commit();
    session.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .contains(&Event::Decoration { server_side: true })
    );

    client.fill(&window.surface, (200, 100), RED);
    session.roundtrip(&mut client);
    client.take_events();

    // Asking again once the window is up, or withdrawing the preference,
    // changes nothing: the compositor never hands decoration to the client.
    decoration.set_mode(Mode::ClientSide);
    decoration.unset_mode();
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        !events.contains(&Event::Decoration { server_side: false }),
        "{events:?}"
    );
    assert!(
        session
            .compositor
            .desktop()
            .frames()
            .next()
            .unwrap()
            .decoration
            .is_some()
    );
}

#[test]
fn a_maximized_decorated_window_is_given_the_space_inside_the_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "foot", (200, 100), RED);
    client.take_events();

    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let inside = (WIDTH - 2 * BORDER, HEIGHT - TITLEBAR - BORDER);
    assert_eq!(
        client.last_configure(),
        (inside, vec![State::Maximized, State::Activated])
    );
}

#[test]
fn a_window_that_resizes_itself_keeps_that_size_when_restored_from_maximized() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "terminal", (200, 100), RED);

    client.fill(&window.surface, (300, 150), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    client.take_events();

    window.role.unset_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((300, 150), vec![State::Activated])
    );
}

#[test]
fn a_stale_size_still_in_flight_is_not_mistaken_for_a_resize() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    client.take_events();

    // The client asks to be restored and, before it hears back, commits
    // another frame at the maximized size.
    window.role.unset_maximized();
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    assert_eq!(
        client.last_configure(),
        ((200, 100), vec![State::Activated])
    );
}
