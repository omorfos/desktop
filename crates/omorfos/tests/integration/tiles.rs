//! Windows that share a workspace, and the line between them.

use omorfos::Input;
use omorfos_config::{Action, Half};

use crate::common::{Client, Session};
use crate::windows::{BACKGROUND, BLUE, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
/// The colour of a window's border in the built-in theme, which the line
/// is drawn in.
const LINE: u32 = 0xff2c_303a;
const YELLOW: u32 = 0xffff_ff00;
const MAGENTA: u32 = 0xffff_00ff;
const GREEN: u32 = 0xff00_ff00;

/// A session with two windows sharing a workspace, the first in the half
/// named. Each goes on drawing itself 200 by 100, so the rest of the
/// screen says what else is there. The pointer is drawn from a theme of
/// coloured squares: green as it ordinarily is, yellow where a line is
/// dragged from side to side, magenta where one is dragged up and down.
fn shared(half: Half) -> (Session, Client) {
    let mut session = Session::new();
    let shapes = [
        ("default", 8, GREEN),
        ("col-resize", 8, YELLOW),
        ("row-resize", 8, MAGENTA),
    ];
    let themes = session.install_cursors("default", &shapes);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::Tile(half));
    session.open(&mut client, "second", (200, 100), BLUE);
    session.step();
    (session, client)
}

fn point(session: &mut Session, x: i32, y: i32) {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    session.step();
}

fn button(session: &mut Session, button: u32, pressed: bool, time: u32) {
    let event = Input::Button { button, pressed };
    session.compositor_mut().input(event, time);
    session.step();
}

/// How wide the first window's part is, and where the second's begins.
fn parts(session: &Session) -> Vec<(i32, i32, u32, u32)> {
    let frames = session.compositor().desktop().frames();
    let part = |frame: omorfos_core::Frame<'_>| {
        let at = frame.geometry;
        (at.origin.x, at.origin.y, at.size.width, at.size.height)
    };
    frames.map(part).collect()
}

#[test]
fn a_line_is_drawn_between_two_windows_that_share_a_workspace() {
    let (mut session, _client) = shared(Half::Left);
    let picture = session.draw();
    assert_eq!(picture.at(320, 300), LINE);
    assert_eq!(picture.at(319, 300), BACKGROUND, "a hair wide");
    assert_eq!(picture.at(321, 300), BACKGROUND);
    assert_eq!(picture.at(10, 10), RED);
    assert_eq!(picture.at(330, 10), BLUE);

    session.step();
    assert!(!session.draw().damaged, "and not drawn again as it is");

    // It takes the colour the theme gives a window's border.
    session.write_config("theme.css", "window { border: 1px solid #ff00ff; }\n");
    session.eventually(|session| session.draw().at(320, 300) == MAGENTA);
}

#[test]
fn the_pointer_shows_that_the_line_can_be_dragged() {
    let (mut session, mut client) = shared(Half::Left);
    let pointer = |session: &mut Session, x: i32, y: i32| {
        point(session, x, y);
        session.draw().at(x + 3, y + 3)
    };
    assert_eq!(pointer(&mut session, 400, 300), GREEN);
    assert_eq!(
        pointer(&mut session, 323, 300),
        YELLOW,
        "a little to one side"
    );
    assert_eq!(pointer(&mut session, 317, 300), YELLOW, "or the other");
    assert_eq!(pointer(&mut session, 330, 300), GREEN);

    // On the line the pointer is on neither window.
    client.take_events();
    point(&mut session, 100, 50);
    point(&mut session, 320, 50);
    session.roundtrip(&mut client);
    let events = client.take_events();
    let left = |event: &crate::common::Event| matches!(event, crate::common::Event::PointerLeave);
    assert!(events.iter().any(left), "{events:?}");
}

#[test]
fn dragging_the_line_gives_one_window_more_room_and_the_other_less() {
    use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

    let (mut session, mut client) = shared(Half::Left);
    assert_eq!(parts(&session), [(0, 0, 320, 480), (320, 0, 320, 480)]);
    client.take_events();

    point(&mut session, 321, 300);
    button(&mut session, LEFT, true, 0);
    point(&mut session, 400, 100);
    assert_eq!(parts(&session), [(0, 0, 400, 480), (400, 0, 240, 480)]);
    let picture = session.draw();
    assert_eq!(picture.at(400, 300), LINE);
    assert_eq!(picture.at(320, 300), BACKGROUND);
    assert_eq!(picture.at(403, 103), YELLOW, "the pointer still says so");

    // Each window is told its new size as the line moves.
    session.roundtrip(&mut client);
    let (size, states) = client.last_configure();
    assert!(size == (400, 480) || size == (240, 480), "{size:?}");
    assert!(states.contains(&State::TiledLeft));

    // Let go, the line stays where it was left and the pointer is free.
    button(&mut session, LEFT, false, 0);
    point(&mut session, 500, 100);
    assert_eq!(parts(&session), [(0, 0, 400, 480), (400, 0, 240, 480)]);

    // It stops before either window has less than a sixth of the screen.
    point(&mut session, 400, 300);
    button(&mut session, LEFT, true, 1000);
    point(&mut session, 630, 300);
    button(&mut session, LEFT, false, 1000);
    assert_eq!(parts(&session), [(0, 0, 534, 480), (534, 0, 106, 480)]);
}

#[test]
fn pressing_the_line_twice_puts_it_back_in_the_middle() {
    let (mut session, _client) = shared(Half::Left);
    point(&mut session, 320, 300);
    button(&mut session, LEFT, true, 0);
    point(&mut session, 200, 300);
    button(&mut session, LEFT, false, 0);
    assert_eq!(parts(&session)[0], (0, 0, 200, 480));

    // Slowly, each press is the start of a drag that goes nowhere.
    button(&mut session, LEFT, true, 5000);
    button(&mut session, LEFT, false, 5000);
    button(&mut session, LEFT, true, 9000);
    button(&mut session, LEFT, false, 9000);
    assert_eq!(parts(&session)[0], (0, 0, 200, 480));

    button(&mut session, LEFT, true, 9200);
    button(&mut session, LEFT, false, 9200);
    assert_eq!(parts(&session), [(0, 0, 320, 480), (320, 0, 320, 480)]);
}

#[test]
fn only_the_left_button_takes_hold_of_the_line() {
    let (mut session, mut client) = shared(Half::Left);
    client.take_events();
    point(&mut session, 320, 300);
    button(&mut session, RIGHT, true, 0);
    point(&mut session, 200, 300);
    button(&mut session, RIGHT, false, 0);
    assert_eq!(parts(&session)[0], (0, 0, 320, 480), "nothing moved");

    // And no window was told of a press on it.
    session.roundtrip(&mut client);
    let events = client.take_events();
    let pressed =
        |event: &crate::common::Event| matches!(event, crate::common::Event::PointerButton { .. });
    assert!(!events.iter().any(pressed), "{events:?}");
}

#[test]
fn a_line_between_one_window_and_another_below_it_is_dragged_up_and_down() {
    let (mut session, _client) = shared(Half::Top);
    assert_eq!(parts(&session), [(0, 0, 640, 240), (0, 240, 640, 240)]);
    assert_eq!(session.draw().at(500, 240), LINE);

    point(&mut session, 500, 242);
    assert_eq!(session.draw().at(503, 245), MAGENTA);
    button(&mut session, LEFT, true, 0);
    point(&mut session, 100, 300);
    button(&mut session, LEFT, false, 0);
    assert_eq!(parts(&session), [(0, 0, 640, 300), (0, 300, 640, 180)]);
    assert_eq!(session.draw().at(500, 300), LINE);
}

#[test]
fn there_is_no_line_once_a_window_is_alone() {
    let (mut session, _client) = shared(Half::Left);
    session.compositor_mut().perform(&Action::Close);
    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    assert_eq!(session.draw().at(320, 300), BACKGROUND);
}
