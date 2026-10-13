//! Server-side decorations: negotiation and how windows sit in their frames.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode;
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use std::time::Duration;

use crate::common::{Event, FOUR_WORKSPACES, HEIGHT, Session, WIDTH, between, half_way};
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
    let mut session =
        Session::with_config(&[("theme.css", theme), ("config.yaml", FOUR_WORKSPACES)]);
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
    let mut session =
        Session::with_config(&[("theme.css", theme), ("config.yaml", FOUR_WORKSPACES)]);
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

/// The colours of a titlebar's buttons under the default theme: close,
/// maximize and minimize when their frame has focus, and all of them when
/// it has not.
const BUTTONS: [u32; 3] = [0xffe0_6c75, 0xff98_c379, 0xffe5_c07b];
const PLAIN_BUTTON: u32 = 0xff2c_303a;

/// The middle of each button of a frame whose corner is at the screen's.
const CLOSE: (i32, i32) = (16, 14);
const MAXIMIZE: (i32, i32) = (36, 14);
const MINIMIZE: (i32, i32) = (56, 14);

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;

fn point(session: &mut Session, (x, y): (i32, i32)) {
    let (x, y) = (f64::from(x), f64::from(y));
    session
        .compositor_mut()
        .input(omorfos::Input::Motion { x, y }, 0);
}

/// Presses or lets go of a mouse button, at a time in thousandths of a
/// second.
fn button(session: &mut Session, button: u32, pressed: bool, time: u32) {
    let event = omorfos::Input::Button { button, pressed };
    session.compositor_mut().input(event, time);
}

fn click(session: &mut Session, at: (i32, i32)) {
    point(session, at);
    button(session, LEFT, true, 0);
    button(session, LEFT, false, 0);
    session.step();
}

/// Whether the first frame is maximized, and how many frames are showing.
fn frames(session: &Session) -> (bool, usize) {
    let mut frames = session.compositor().desktop().frames();
    let maximized = frames.next().is_some_and(|frame| frame.maximized);
    (maximized, session.compositor().desktop().frames().count())
}

#[test]
fn a_titlebar_has_buttons_that_close_make_fullscreen_and_minimize() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    client.take_events();
    let picture = session.draw();
    let shown = [CLOSE, MAXIMIZE, MINIMIZE].map(|(x, y)| picture.at(x, y));
    assert_eq!(shown, BUTTONS);

    // The second is for fullscreen: the window goes to a workspace of
    // its own, and comes back from it where it was.
    let places = |session: &Session| session.compositor().desktop().workspaces().count();
    click(&mut session, MAXIMIZE);
    assert!(first_frame(&session).fullscreen);
    assert_eq!(first_frame(&session).home, omorfos_core::Home::Solo);
    assert_eq!(places(&session), 2);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Fullscreen);
    session.step();
    assert!(!first_frame(&session).fullscreen, "and back again");
    assert_eq!(places(&session), 1);
    assert_eq!(frames(&session), (false, 1));

    click(&mut session, CLOSE);
    session.roundtrip(&mut client);
    assert!(
        client.take_events().contains(&Event::Close),
        "it is asked to"
    );
    assert_eq!(frames(&session).1, 1, "and is there until it does");

    click(&mut session, MINIMIZE);
    assert_eq!(frames(&session).1, 0, "put away");
}

#[test]
fn a_button_does_nothing_unless_the_pointer_is_let_go_on_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    client.take_events();
    let before = session
        .compositor()
        .desktop()
        .frames()
        .next()
        .unwrap()
        .geometry;

    // Pressed on one and let go on another, or off them altogether.
    for elsewhere in [MAXIMIZE, (120, 14), (100, 80)] {
        point(&mut session, CLOSE);
        button(&mut session, LEFT, true, 0);
        point(&mut session, elsewhere);
        button(&mut session, LEFT, false, 0);
        session.step();
    }
    // Nor with another button of the mouse.
    point(&mut session, CLOSE);
    button(&mut session, RIGHT, true, 0);
    button(&mut session, RIGHT, false, 0);
    // Nor by being let go on one that was not pressed.
    point(&mut session, (100, 80));
    button(&mut session, LEFT, true, 0);
    point(&mut session, MINIMIZE);
    button(&mut session, LEFT, false, 0);
    session.roundtrip(&mut client);

    assert!(!client.take_events().contains(&Event::Close));
    assert_eq!(frames(&session), (false, 1));
    let after = session
        .compositor()
        .desktop()
        .frames()
        .next()
        .unwrap()
        .geometry;
    assert_eq!(after, before, "and the frame was not dragged about");
}

#[test]
fn a_button_is_coloured_under_the_pointer_whether_or_not_its_frame_has_focus() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    // Another window takes the focus, clear of the first one's titlebar.
    session.open(&mut client, "second", (50, 50), RED);
    point(&mut session, (600, 400));
    session.step();
    let buttons = |session: &mut Session| {
        let picture = session.draw();
        [(11, 14), (31, 14), (51, 14)].map(|(x, y)| picture.at(x, y))
    };
    assert_eq!(buttons(&mut session), [PLAIN_BUTTON; 3]);

    point(&mut session, MAXIMIZE);
    session.step();
    assert_eq!(
        buttons(&mut session),
        [PLAIN_BUTTON, BUTTONS[1], PLAIN_BUTTON]
    );
    assert_ne!(
        session.draw().at(MAXIMIZE.0, MAXIMIZE.1),
        BUTTONS[1],
        "with a sign on it of what it does"
    );

    point(&mut session, (120, 14));
    session.step();
    assert_eq!(
        buttons(&mut session),
        [PLAIN_BUTTON; 3],
        "and not once left"
    );
}

#[test]
fn a_double_click_on_a_titlebar_maximizes_the_frame_and_another_puts_it_back() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    let title = (120, 14);
    let press = |session: &mut Session, at: (i32, i32), time: u32| {
        point(session, at);
        button(session, LEFT, true, time);
        button(session, LEFT, false, time);
        session.step();
    };

    press(&mut session, title, 1000);
    assert_eq!(frames(&session), (false, 1), "one click is not two");
    press(&mut session, title, 1300);
    assert_eq!(frames(&session), (true, 1));

    // Two clicks far apart in time are two clicks.
    press(&mut session, title, 5000);
    assert_eq!(frames(&session), (true, 1));
    // As are two that are far apart on the titlebar.
    press(&mut session, (300, 14), 5100);
    assert_eq!(frames(&session), (true, 1));
    // And two on different frames, if another is put in the way.
    press(&mut session, (300, 14), 5200);
    assert_eq!(frames(&session), (false, 1), "back as it was");

    let mut other = session.connect();
    session.open_decorated(&mut other, "second", (200, 100), RED);
    let second = session
        .compositor()
        .desktop()
        .frames()
        .last()
        .unwrap()
        .geometry;
    let on_second = (second.origin.x + 120, second.origin.y + 14);
    press(&mut session, on_second, 9000);
    press(&mut session, title, 9100);
    assert_eq!(
        frames(&session),
        (false, 2),
        "a click on each is not two on one"
    );
}

/// Where the pointer is put to open the menu of the first window: on its
/// title, clear of the buttons.
const TITLE: (i32, i32) = (120, 14);

/// Opens the menu of the window whose titlebar is at `at`, with a click of
/// the right button there.
fn open_window_menu(session: &mut Session, at: (i32, i32)) {
    point(session, at);
    button(session, RIGHT, true, 0);
    button(session, RIGHT, false, 0);
    session.step();
}

/// A point on a row of a menu opened at `at`. The rows are: close,
/// maximize, minimize, fullscreen; then shade, keep above, show on every
/// workspace; then send to a workspace. Each is 28 tall, inside 4 of
/// padding, and the lines between the three groups take 9 each.
fn row(at: (i32, i32), which: i32) -> (i32, i32) {
    let lines = (i32::from(which >= 4) + i32::from(which >= 7)) * 9;
    (at.0 + 60, at.1 + 4 + which * 28 + lines + 14)
}

/// Chooses a row of the menu of the first window.
fn choose(session: &mut Session, which: i32) {
    open_window_menu(session, TITLE);
    click(session, row(TITLE, which));
}

fn first_frame(session: &Session) -> omorfos_core::Frame<'_> {
    session.compositor().desktop().frames().next().unwrap()
}

#[test]
fn a_right_click_on_a_titlebar_opens_a_menu_of_what_can_be_done_with_the_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    client.take_events();

    open_window_menu(&mut session, TITLE);
    let menu = session.draw().at(TITLE.0 + 200, TITLE.1 + 100);
    assert_ne!(menu, BACKGROUND, "a menu, where the pointer is");
    // A click beside it puts it away and does nothing else.
    click(&mut session, (600, 400));
    assert_eq!(session.draw().at(TITLE.0 + 200, TITLE.1 + 100), BACKGROUND);

    choose(&mut session, 1);
    assert!(first_frame(&session).maximized);
    // Its titlebar is now along the top of the screen, and the same row
    // puts the window back.
    choose(&mut session, 1);
    assert!(!first_frame(&session).maximized);

    choose(&mut session, 0);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Close));

    choose(&mut session, 2);
    assert_eq!(frames(&session).1, 0, "minimized");
}

#[test]
fn a_window_can_be_rolled_up_into_its_titlebar_and_let_down_again() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    client.take_events();
    assert_eq!(first_frame(&session).geometry.size.height, 129);

    choose(&mut session, 4);
    assert!(first_frame(&session).standing.shaded);
    assert_eq!(first_frame(&session).geometry.size.height, 28);
    let picture = session.draw();
    assert_eq!(
        picture.at(190, 14),
        FOCUSED_TITLEBAR,
        "its titlebar is there"
    );
    assert_eq!(picture.at(100, 80), BACKGROUND, "and none of the window");

    // Nothing of the window is there to be clicked on, and it is not told
    // to be any other size.
    click(&mut session, (100, 80));
    session.roundtrip(&mut client);
    let told = client.take_events();
    let clicked = |event: &Event| matches!(event, Event::PointerButton { .. });
    assert!(!told.iter().any(clicked), "{told:?}");
    let resized =
        |event: &Event| matches!(event, Event::Configure { size, .. } if *size != (200, 100));
    assert!(!told.iter().any(resized), "{told:?}");

    // The same row lets it down.
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Shade);
    session.step();
    assert!(!first_frame(&session).standing.shaded);
    assert_eq!(session.draw().at(100, 80), RED);
}

#[test]
fn a_window_kept_above_stays_over_the_one_that_is_clicked() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    choose(&mut session, 5);
    assert!(first_frame(&session).standing.above);

    // A second window opens over part of the first, and has focus.
    let mut other = session.connect();
    session.open_decorated(&mut other, "second", (200, 100), 0xff00_ff00);
    let frames: Vec<_> = session.compositor().desktop().frames().collect();
    let on_top = frames.last().unwrap().window().app_id.clone();
    assert_eq!(on_top, "first", "kept above the newer one");
    let second = frames[0].geometry;
    drop(frames);
    // Where the two overlap, the first is what shows.
    let shared = (second.origin.x + 60, second.origin.y + 60);
    assert_eq!(session.draw().at(shared.0, shared.1), RED);

    // Asked again, it is where any other window would be.
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::FocusNext);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::KeepAbove);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::FocusNext);
    session.step();
    assert_eq!(session.draw().at(shared.0, shared.1), 0xff00_ff00);
}

#[test]
fn a_window_can_be_shown_on_every_workspace_or_sent_to_one() {
    use omorfos_config::Action;

    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);

    choose(&mut session, 6);
    assert!(first_frame(&session).standing.sticky);
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 80), RED, "there too");
    session.compositor_mut().perform(&Action::Stick);
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 80), BACKGROUND, "left where it was");
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();

    // The last row opens a list of the workspaces beside it, and the
    // third of those is where the window is sent.
    open_window_menu(&mut session, TITLE);
    let send = row(TITLE, 7);
    point(&mut session, send);
    session.step();
    click(&mut session, (send.0 + 220, send.1 + 2 * 28));
    assert_eq!(session.draw().at(100, 80), BACKGROUND, "gone from this one");
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 80), RED, "and on the third");
}

#[test]
fn a_window_is_made_fullscreen_from_the_menu_of_its_titlebar() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);

    choose(&mut session, 3);
    assert!(first_frame(&session).fullscreen);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
}

#[test]
fn a_window_can_be_sent_to_a_new_workspace_from_its_titlebar() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    assert_eq!(session.compositor().desktop().workspace_count(), 1);

    // Beside the last row: the one workspace there is, a line, and a new
    // one.
    open_window_menu(&mut session, TITLE);
    let send = row(TITLE, 7);
    point(&mut session, send);
    session.step();
    click(&mut session, (send.0 + 220, send.1 + 28 + 9));

    assert_eq!(session.draw().at(100, 80), BACKGROUND, "gone from this one");
    assert_eq!(session.compositor().desktop().workspace_count(), 2);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::NextWorkspace);
    session.step();
    assert_eq!(
        session.draw().at(100, 80),
        RED,
        "and on the one made for it"
    );
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
