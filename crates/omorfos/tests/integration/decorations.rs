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
    let frame = session.compositor().desktop().frames().next().unwrap();
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
    let frame = session.compositor().desktop().frames().next().unwrap();
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
    // Its bottom corners are rounded with the frame's; see `corners`.
    assert_eq!(picture.at(BORDER + 199, TITLEBAR + 80), RED, "its far edge");
    assert_eq!(picture.at(BORDER + 100, TITLEBAR + 99), RED, "its last row");
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
            .compositor()
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

/// Colours of the default theme, as `0xAARRGGBB`.
const ACCENT: u32 = 0xff5b_9cf5;
const FOCUSED_TITLEBAR: u32 = 0xff23_262e;
const UNFOCUSED_TITLEBAR: u32 = 0xff1b_1d23;
const UNFOCUSED_BORDER: u32 = 0xff2c_303a;

#[test]
fn a_decorated_frame_is_drawn_with_a_titlebar_and_borders() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "foot", (200, 100), RED);

    let picture = session.draw();
    // The frame is 202 by 129: a 28 pixel titlebar, then the window, with a
    // 1 pixel border on the left, right and bottom.
    assert_eq!(
        picture.at(190, 14),
        FOCUSED_TITLEBAR,
        "titlebar, clear of the title"
    );
    assert_eq!(
        picture.at(100, 0),
        ACCENT,
        "the focused frame's outline is the accent colour"
    );
    assert_eq!(picture.at(0, 60), ACCENT, "left border");
    assert_eq!(picture.at(201, 60), ACCENT, "right border");
    assert_eq!(picture.at(100, 128), ACCENT, "bottom border");
    assert_eq!(picture.at(0, 0), BACKGROUND, "the top corners are rounded");
    assert_eq!(picture.at(202, 60), BACKGROUND, "nothing beyond the frame");
    assert_eq!(picture.at(100, 129), BACKGROUND);
}

#[test]
fn the_title_is_written_on_the_titlebar() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "foot", (300, 100), RED);

    let picture = session.draw();
    let title_area = (11..150).flat_map(|x| (4..24).map(move |y| (x, y)));
    let inked = title_area
        .filter(|(x, y)| picture.at(*x, *y) != FOCUSED_TITLEBAR)
        .count();
    assert!(
        inked > 30,
        "expected glyphs on the titlebar, found {inked} changed pixels"
    );
}

#[test]
fn a_frame_that_loses_focus_is_redrawn_in_the_unfocused_colours() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    session.open(&mut client, "second", (50, 50), RED);
    session
        .compositor_mut()
        .input(omorfos::Input::Motion { x: 600.0, y: 400.0 }, 0);

    let picture = session.draw();
    assert_eq!(picture.at(190, 14), UNFOCUSED_TITLEBAR);
    assert_eq!(picture.at(0, 100), UNFOCUSED_BORDER);
}

#[test]
fn decorations_are_repainted_only_when_something_about_them_changes() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "editor", (200, 100), RED);
    assert!(session.draw().damaged);

    session.step();
    assert!(!session.draw().damaged, "an unchanged frame is not redrawn");

    window.role.set_title("a different title".to_owned());
    session.roundtrip(&mut client);
    assert!(session.draw().damaged, "a new title repaints the titlebar");
}

#[test]
fn a_theme_change_restyles_existing_frames() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "foot", (200, 100), RED);

    session.write_config("theme.yaml", "colors: { accent: '#00ff00', focused: { border: '#00ff00', titlebar: '#0000ff', text: '#ffffff' } }\nborder: { width: 4, radius: 0 }\n");
    session.compositor_mut().reload_config();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let picture = session.draw();
    assert_eq!(
        picture.at(0, 0),
        0xff00_ff00,
        "square corners in the new border colour"
    );
    assert_eq!(picture.at(2, 60), 0xff00_ff00, "a wider border");
    assert_eq!(picture.at(190, 14), 0xff00_00ff);
}
