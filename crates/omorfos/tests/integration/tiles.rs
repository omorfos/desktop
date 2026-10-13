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

/// The app of the window with focus.
fn focused(session: &Session) -> String {
    let desktop = session.compositor().desktop();
    let window = desktop.focused().and_then(|id| desktop.window(id));
    window
        .map(|window| window.app_id.clone())
        .unwrap_or_default()
}

#[test]
fn a_third_window_has_half_of_a_side_and_a_line_of_its_own() {
    let (mut session, mut client) = shared(Half::Left);
    // The second, on the right, takes the bottom of the right; the third
    // to open has the top.
    session
        .compositor_mut()
        .perform(&Action::Tile(Half::Bottom));
    session.open(&mut client, "third", (200, 100), GREEN);
    session.step();
    assert_eq!(
        parts(&session),
        [(0, 0, 320, 480), (320, 240, 320, 240), (320, 0, 320, 240)]
    );
    let picture = session.draw();
    assert_eq!(picture.at(320, 400), LINE, "the line down the screen");
    assert_eq!(picture.at(500, 240), LINE, "and the one across the right");
    assert_eq!(picture.at(100, 240), BACKGROUND, "which stops at the first");

    // The line across is dragged by itself.
    point(&mut session, 500, 241);
    assert_eq!(session.draw().at(503, 244), MAGENTA);
    button(&mut session, LEFT, true, 0);
    point(&mut session, 500, 300);
    button(&mut session, LEFT, false, 0);
    assert_eq!(
        parts(&session),
        [(0, 0, 320, 480), (320, 300, 320, 180), (320, 0, 320, 300)]
    );
}

#[test]
fn focus_and_windows_go_from_part_to_part_by_direction() {
    let (mut session, _client) = shared(Half::Left);
    assert_eq!(focused(&session), "second");

    session.compositor_mut().perform(&Action::Focus(Half::Left));
    assert_eq!(focused(&session), "first");
    session.compositor_mut().perform(&Action::Focus(Half::Left));
    assert_eq!(focused(&session), "first", "nothing further that way");

    // The first changes places with the second, and keeps focus.
    session.compositor_mut().perform(&Action::Swap(Half::Right));
    session.step();
    assert_eq!(focused(&session), "first");
    let picture = session.draw();
    assert_eq!(picture.at(330, 10), RED, "the first is on the right now");
    assert_eq!(picture.at(10, 10), BLUE);
}

/// A session with one window, 200 by 100 with a titlebar, held by its
/// titlebar and about to be dragged.
fn held() -> (Session, Client) {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    point(&mut session, 120, 14);
    button(&mut session, LEFT, true, 0);
    (session, client)
}

/// Where the first frame is, and the workspace being shown.
fn placed(session: &Session) -> (Option<(i32, i32, u32, u32)>, usize) {
    let desktop = session.compositor().desktop();
    (parts(session).first().copied(), desktop.active_workspace())
}

#[test]
fn a_window_dragged_to_an_edge_is_shown_the_half_it_would_take() {
    let (mut session, _client) = held();
    let before = parts(&session);

    // Against the left of the screen, the left half is washed over.
    point(&mut session, 0, 240);
    let picture = session.draw();
    assert_ne!(picture.at(250, 400), BACKGROUND, "the half it would have");
    assert_eq!(picture.at(400, 400), BACKGROUND, "and not the other");
    session.step();
    assert!(
        !session.draw().damaged,
        "not drawn again while nothing moves"
    );

    // Away from the edge again, nothing is offered and nothing happens.
    point(&mut session, 300, 240);
    assert_eq!(session.draw().at(250, 400), BACKGROUND);
    button(&mut session, LEFT, false, 0);
    assert_eq!(placed(&session).1, 0);
    assert_ne!(parts(&session), before, "it was only moved");
}

#[test]
fn let_go_against_an_edge_a_window_is_put_in_that_half_of_a_workspace() {
    let edges = [
        ((0, 240), (0, 0, 320, 480)),
        ((639, 240), (320, 0, 320, 480)),
        ((320, 0), (0, 0, 640, 240)),
        ((320, 479), (0, 240, 640, 240)),
    ];
    for (edge, half) in edges {
        let (mut session, _client) = held();
        let before = parts(&session);
        point(&mut session, edge.0, edge.1);
        button(&mut session, LEFT, false, 0);
        assert_eq!(placed(&session), (Some(half), 1), "{edge:?}");

        // Back on its desktop, it is where it was before it was dragged
        // to the edge, and not half off the screen.
        session.compositor_mut().perform(&Action::Fullscreen);
        session.step();
        assert_eq!(parts(&session), before);
    }
}

#[test]
fn near_a_corner_a_window_is_offered_a_quarter() {
    let corners = [
        ((0, 20), (0, 0, 320, 240)),
        ((30, 0), (0, 0, 320, 240)),
        ((0, 470), (0, 240, 320, 240)),
        ((639, 10), (320, 0, 320, 240)),
        ((600, 479), (320, 240, 320, 240)),
    ];
    for (corner, quarter) in corners {
        let (mut session, _client) = held();
        point(&mut session, corner.0, corner.1);
        let middle = (quarter.0 + 160, quarter.1 + 200);
        let picture = session.draw();
        assert_ne!(picture.at(middle.0, middle.1), BACKGROUND, "{corner:?}");
        button(&mut session, LEFT, false, 0);
        assert_eq!(placed(&session), (Some(quarter), 1), "{corner:?}");
    }
}

#[test]
fn an_edge_with_another_screen_beyond_it_offers_nothing() {
    let (mut session, _client) = held();
    session.add_output((640, 480));
    session.step();

    // The right of the first screen is the way to the second.
    point(&mut session, 639, 240);
    assert_eq!(session.draw().at(400, 400), BACKGROUND);
    // The far right of the second is an edge like any other.
    point(&mut session, 1279, 240);
    button(&mut session, LEFT, false, 0);
    let part = parts(&session).first().copied();
    assert_eq!(part, Some((960, 0, 320, 480)));

    // Back on a desktop, it is in the middle of the screen it was
    // dragged to: 200 by 100 and its titlebar and borders, in 640 by 480.
    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    let (x, y, width, height) = parts(&session).first().copied().unwrap_or_default();
    let (right, below) = (
        1280 - x - width.cast_signed(),
        480 - y - height.cast_signed(),
    );
    assert!((x - 640 - right).abs() <= 1, "as far from each side");
    assert!((y - below).abs() <= 1, "and from top and bottom");
}

#[test]
fn a_window_held_when_its_screen_goes_is_offered_nothing() {
    let (mut session, _client) = held();
    point(&mut session, 0, 240);
    let only = session.output();
    session.compositor_mut().remove_output(&only);
    button(&mut session, LEFT, false, 0);
    assert_eq!(session.compositor().desktop().workspaces().count(), 1);
}

#[test]
fn a_window_carried_by_its_titlebar_to_another_part_changes_places() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    session.open_decorated(&mut client, "second", (200, 100), BLUE);
    session.step();
    let apps = |session: &Session| -> Vec<(String, i32)> {
        let frames = session.compositor().desktop().frames();
        let mut apps: Vec<(String, i32)> = frames
            .map(|frame| (frame.window().app_id.clone(), frame.geometry.origin.x))
            .collect();
        apps.sort();
        apps
    };
    assert_eq!(
        apps(&session),
        [("first".to_owned(), 0), ("second".to_owned(), 320)]
    );

    // Its titlebar is brought back at the top of the screen, and taken
    // hold of.
    point(&mut session, 150, 0);
    session.advance(std::time::Duration::from_millis(300));
    point(&mut session, 150, 14);
    button(&mut session, LEFT, true, 0);

    // Over its own part nothing is offered; over the other, that part is
    // washed over.
    point(&mut session, 150, 300);
    assert_eq!(session.draw().at(500, 400), BACKGROUND);
    point(&mut session, 500, 300);
    assert_ne!(session.draw().at(500, 400), BACKGROUND);
    assert_eq!(session.draw().at(100, 400), BACKGROUND);

    button(&mut session, LEFT, false, 0);
    assert_eq!(
        apps(&session),
        [("first".to_owned(), 320), ("second".to_owned(), 0)]
    );
    assert_eq!(
        session.draw().at(500, 400),
        BACKGROUND,
        "nothing is offered now"
    );
}
