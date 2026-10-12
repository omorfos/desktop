//! Server-side decorations: negotiation and how windows sit in their frames.

use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode;
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use std::time::Duration;

use crate::common::{Event, HEIGHT, Session, WIDTH, between, half_way};
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
fn a_frame_gets_to_the_colours_of_its_focus_over_the_time_the_theme_gives() {
    let theme = "window, titlebar { transition: 100ms linear; }\n\
                 window { border-color: #ff0000; }\n\
                 window:focus { border-color: #0000ff; }\n\
                 titlebar { background: #ff0000; }\n\
                 titlebar:focus { background: #0000ff; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    let (red, blue) = (0xffff_0000, 0xff00_00ff);
    let frame = |session: &mut Session| {
        let picture = session.draw();
        (picture.at(190, 14), picture.at(0, 100))
    };
    session.open_decorated(&mut client, "first", (200, 100), RED);
    // A window comes in over the same time, which is another test's.
    session.advance(Duration::from_millis(100));
    assert_eq!(frame(&mut session), (blue, blue), "in the colours of focus");
    assert_eq!(session.compositor_mut().deadline(), None);

    let start = session.now;
    session.open(&mut client, "second", (50, 50), RED);
    session
        .compositor_mut()
        .input(omorfos::Input::Motion { x: 600.0, y: 400.0 }, 0);
    session.step();
    assert_eq!(frame(&mut session), (blue, blue), "setting out");
    let again = session.compositor_mut().deadline();
    assert_eq!(again, Some(start + Duration::from_millis(16)));

    session.now = start + Duration::from_millis(50);
    session.step();
    let (titlebar, border) = frame(&mut session);
    assert!(half_way(titlebar), "{titlebar:#x}");
    assert!(half_way(border), "{border:#x}");

    session.now = start + Duration::from_millis(100);
    session.step();
    assert_eq!(frame(&mut session), (red, red), "arrived");
    assert_eq!(session.compositor_mut().deadline(), None, "and at rest");
    assert!(!session.draw().damaged, "and not drawn again");
}

#[test]
fn a_window_and_its_frame_come_onto_the_screen_over_the_time_the_theme_gives() {
    let theme = "window { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    // The titlebar, the border down the left and the window itself.
    let seen = |session: &mut Session| {
        let picture = session.draw();
        [picture.at(190, 14), picture.at(0, 100), picture.at(100, 80)]
    };
    session.open_decorated(&mut client, "first", (200, 100), RED);
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "not there yet");
    let start = session.now;
    let again = session.compositor_mut().deadline();
    assert_eq!(again, Some(start + Duration::from_millis(16)));

    session.advance(Duration::from_millis(50));
    let [titlebar, border, window] = seen(&mut session);
    assert!(
        between(titlebar, BACKGROUND, FOCUSED_TITLEBAR),
        "{titlebar:#x}"
    );
    assert!(between(border, BACKGROUND, ACCENT), "{border:#x}");
    assert!(between(window, BACKGROUND, RED), "{window:#x}");

    session.advance(Duration::from_millis(50));
    assert_eq!(seen(&mut session), [FOCUSED_TITLEBAR, ACCENT, RED]);
    assert_eq!(session.compositor_mut().deadline(), None, "and at rest");
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");
}

#[test]
fn a_window_and_its_frame_go_from_the_screen_over_the_time_the_theme_gives() {
    use omorfos_config::Action;

    let theme = "window { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    // The titlebar, the border down the left and the window itself.
    let seen = |session: &mut Session| {
        let picture = session.draw();
        [picture.at(190, 14), picture.at(0, 100), picture.at(100, 80)]
    };
    let whole = [FOCUSED_TITLEBAR, ACCENT, RED];
    session.open_decorated(&mut client, "first", (200, 100), RED);
    session.advance(Duration::from_millis(100));
    assert_eq!(seen(&mut session), whole);

    // Its workspace is left, which takes it from the screen.
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();
    assert_eq!(seen(&mut session), whole, "still all there");
    let start = session.now;
    let again = session.compositor_mut().deadline();
    assert_eq!(again, Some(start + Duration::from_millis(16)));

    session.advance(Duration::from_millis(50));
    let [titlebar, border, window] = seen(&mut session);
    assert!(
        between(titlebar, BACKGROUND, FOCUSED_TITLEBAR),
        "{titlebar:#x}"
    );
    assert!(between(border, BACKGROUND, ACCENT), "{border:#x}");
    assert!(between(window, BACKGROUND, RED), "{window:#x}");

    session.advance(Duration::from_millis(50));
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "gone");
    assert_eq!(session.compositor_mut().deadline(), None, "and at rest");
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");

    // Shown again, it comes in afresh.
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.step();
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "coming back");
    session.advance(Duration::from_millis(100));
    assert_eq!(seen(&mut session), whole);
}

#[test]
fn a_window_that_closes_fades_out_as_it_last_was() {
    let theme = "window { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "first", (200, 100), RED);
    session.advance(Duration::from_millis(100));
    assert_eq!(session.draw().at(100, 80), RED);

    // The program takes its window away, and keeps what it drew it from.
    window.role.destroy();
    window.xdg_surface.destroy();
    window.surface.destroy();
    session.roundtrip(&mut client);
    assert_eq!(session.draw().at(100, 80), RED, "still all there");

    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    let (titlebar, inside) = (picture.at(190, 14), picture.at(100, 80));
    assert!(
        between(titlebar, BACKGROUND, UNFOCUSED_TITLEBAR)
            || between(titlebar, BACKGROUND, FOCUSED_TITLEBAR),
        "{titlebar:#x}"
    );
    assert!(between(inside, BACKGROUND, RED), "{inside:#x}");

    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    assert_eq!(
        (picture.at(190, 14), picture.at(100, 80)),
        (BACKGROUND, BACKGROUND)
    );
}

#[test]
fn a_window_whose_program_has_gone_does_not_stop_the_screen_being_drawn() {
    let theme = "window { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    session.advance(Duration::from_millis(100));
    assert_eq!(session.draw().at(100, 80), RED);

    // Everything the program had goes with it, the memory its window was
    // drawn from included. Its frame is the compositor's and still fades.
    drop(client);
    session.eventually(|session| session.compositor().desktop().frames().count() == 0);
    let start = session.now;
    session.now = start + Duration::from_millis(50);
    session.step();
    let titlebar = session.draw().at(190, 14);
    assert_ne!(titlebar, BACKGROUND, "{titlebar:#x}");

    session.now = start + Duration::from_millis(200);
    session.step();
    assert_eq!(session.draw().at(190, 14), BACKGROUND);
}

#[test]
fn a_window_of_one_colour_fades_out_too() {
    let theme = "window { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    let window = client.toplevel("plain", "plain window");
    session.roundtrip(&mut client);
    client.fill_plain(&window.surface, (200, 100), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    session.advance(Duration::from_millis(100));
    let inside = |session: &mut Session| session.draw().at(20, 20);
    assert_eq!(inside(&mut session), RED);

    session
        .compositor_mut()
        .perform(&omorfos_config::Action::NextWorkspace);
    session.step();
    session.advance(Duration::from_millis(50));
    let half = inside(&mut session);
    assert!(between(half, BACKGROUND, RED), "{half:#x}");
    session.advance(Duration::from_millis(50));
    assert_eq!(inside(&mut session), BACKGROUND);
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

    session.write_config(
        "theme.css",
        "window { border-width: 4px; border-radius: 0; }\n\
         window:focus { border-color: #00ff00; }\n\
         titlebar:focus { background: #0000ff; color: #ffffff; }\n",
    );
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
