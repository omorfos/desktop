//! Windows that share a workspace, and the line between them.

use omorfos::Input;
use omorfos_config::{Action, Half};

use crate::common::{Client, Session, Toplevel};
use crate::windows::{BACKGROUND, BLUE, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
/// The colour of the line in the built-in theme, which is that of a
/// window's border.
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

    // It takes the colour the theme gives it.
    session.write_config("theme.css", "tile-line { color: #ff00ff; }\n");
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
fn the_line_stops_where_a_window_says_it_can_be_no_smaller() {
    let mut session = Session::new();
    let mut client = session.connect();
    let first = session.open(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    let second = session.open(&mut client, "second", (200, 100), BLUE);
    first.role.set_min_size(250, 0);
    first.surface.commit();
    // A window with a titlebar over it says how small it can be under that.
    second.role.set_min_size(300, 0);
    second.surface.commit();
    session.roundtrip(&mut client);

    point(&mut session, 320, 300);
    button(&mut session, LEFT, true, 0);
    point(&mut session, 100, 300);
    assert_eq!(parts(&session)[0], (0, 0, 250, 480), "the first's least");
    point(&mut session, 600, 300);
    assert_eq!(parts(&session)[0], (0, 0, 340, 480), "the second's least");
    button(&mut session, LEFT, false, 0);

    // The middle, too, is only as near as the second lets it be.
    second.role.set_min_size(400, 0);
    second.surface.commit();
    session.roundtrip(&mut client);
    point(&mut session, 340, 300);
    button(&mut session, LEFT, true, 1000);
    button(&mut session, LEFT, false, 1000);
    button(&mut session, LEFT, true, 1100);
    button(&mut session, LEFT, false, 1100);
    assert_eq!(parts(&session)[0], (0, 0, 240, 480));
}

#[test]
fn a_window_that_says_it_can_be_no_smaller_is_given_room_without_the_line_dragged() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    let second = session.open(&mut client, "second", (200, 100), BLUE);
    assert_eq!(parts(&session)[0], (0, 0, 320, 480));

    second.role.set_min_size(400, 0);
    second.surface.commit();
    session.roundtrip(&mut client);
    assert_eq!(parts(&session)[0], (0, 0, 240, 480), "as much as it says");
    let (size, _) = client.last_configure();
    assert_eq!(size.0, 400, "and it is told so");

    // Saying it no longer, it goes back to where the line was.
    second.role.set_min_size(0, 0);
    second.surface.commit();
    session.roundtrip(&mut client);
    assert_eq!(parts(&session)[0], (0, 0, 320, 480));
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

    // Its titlebar is there to be taken hold of.
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

/// A session with a red window on the desktop and a blue one tiled on the
/// left of a workspace, its right half with nothing in it.
/// Whether anything other than the background is drawn in `picture`
/// between `left` and `right` across and `top` and `bottom` down.
fn inked(
    picture: &crate::common::Picture,
    (left, top, right, bottom): (i32, i32, i32, i32),
) -> bool {
    (top..bottom).any(|y| (left..right).any(|x| picture.at(x, y) != BACKGROUND))
}

/// Where the title of the window `waiting` offers is written, under its
/// picture: across its box, inside its padding.
const TITLE: (i32, i32, i32, i32) = (352, 328, 608, 348);

fn waiting(config: &[(&str, &str)]) -> (Session, Client, Toplevel) {
    let mut session = Session::with_config(config);
    let mut client = session.connect();
    let waiting = session.open(&mut client, "waiting", (400, 300), RED);
    session.open(&mut client, "first", (200, 100), BLUE);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    session.step();
    (session, client, waiting)
}

#[test]
fn a_window_of_the_desktop_is_offered_small_in_a_part_with_nothing_in_it() {
    let (mut session, mut client, _waiting) = waiting(&[]);
    // Made as small as it has to be to fit the half with its title under
    // it, in a box with room round them.
    let picture = session.draw();
    assert_eq!(picture.at(360, 140), RED);
    assert_eq!(picture.at(600, 320), RED);
    assert_eq!(picture.at(348, 200), BACKGROUND, "room inside the box");
    assert_eq!(picture.at(360, 128), BACKGROUND, "and above");
    assert!(!inked(&picture, (352, 324, 608, 328)), "a gap under it");
    // The box has the border windows have, as the line beside it does.
    let border = picture.at(320, 200);
    assert_ne!(border, BACKGROUND);
    for (x, y) in [(344, 200), (615, 200), (480, 124), (480, 355)] {
        assert_eq!(picture.at(x, y), border, "at {x}, {y}");
    }
    assert_eq!(picture.at(343, 200), BACKGROUND, "and nothing outside it");
    // Its title is written under the picture, in the middle, and nothing
    // under that.
    assert!(inked(&picture, TITLE), "its title");
    let (left, top, right, bottom) = TITLE;
    assert!(
        !inked(&picture, (left, top, left + 10, bottom)),
        "in the middle"
    );
    assert!(!inked(&picture, (right - 10, top, right, bottom)));
    assert!(!inked(&picture, (left, bottom, right, 354)), "and below");

    // The pointer over it is the compositor's, and washes the whole box
    // over, title and all, which is brighter.
    session.roundtrip(&mut client);
    client.take_events();
    point(&mut session, 480, 240);
    session.roundtrip(&mut client);
    let entered =
        |event: &crate::common::Event| matches!(event, crate::common::Event::PointerEnter { .. });
    let events = client.take_events();
    assert!(!events.iter().any(entered), "{events:?}");
    let washed = session.draw();
    assert_ne!(washed.at(480, 240), RED);
    assert_ne!(washed.at(348, 200), BACKGROUND, "the box is washed");
    let lit = |picture: &crate::common::Picture| {
        let rows = TITLE.1..TITLE.3;
        rows.flat_map(|y| (TITLE.0..TITLE.2).map(move |x| (x, y)))
            .map(|(x, y)| picture.at(x, y))
            .collect::<Vec<_>>()
    };
    assert_ne!(lit(&washed), lit(&picture), "and its title");
    point(&mut session, 100, 240);
    assert_eq!(session.draw().at(480, 240), RED, "and not once it leaves");

    // Only the left button chooses it, on its picture or its title; that
    // puts it in the part.
    point(&mut session, 480, 338);
    button(&mut session, RIGHT, true, 0);
    button(&mut session, RIGHT, false, 0);
    assert_eq!(parts(&session).len(), 1);
    button(&mut session, LEFT, true, 0);
    button(&mut session, LEFT, false, 0);
    assert_eq!(parts(&session), [(0, 0, 320, 480), (320, 0, 320, 480)]);
}

#[test]
fn a_window_offered_keeps_drawing_and_a_private_one_is_blank() {
    let rules = "rules:\n  - match: { app_id: waiting }\n    private: true\n";
    let theme = "capture { background: #00ff00; }";
    let config = [("rules.yaml", rules), ("theme.css", theme)];
    let (mut session, mut client, waiting) = waiting(&config);
    let picture = session.draw();
    assert_eq!(picture.at(480, 240), GREEN);
    assert!(inked(&picture, TITLE), "told by its title");
    // Washed over under the pointer as any other is.
    point(&mut session, 480, 240);
    assert_ne!(session.draw().at(480, 240), GREEN);
    point(&mut session, 100, 240);

    // A title that changes is written anew: none, and nothing is.
    waiting.role.set_title(String::new());
    session.roundtrip(&mut client);
    assert!(!inked(&session.draw(), TITLE));

    // Told that what it drew has been shown, as on the screen.
    client.take_events();
    client.request_frame(&waiting.surface);
    session.roundtrip(&mut client);
    session.send_frames(std::time::Duration::from_millis(16));
    session.roundtrip(&mut client);
    let done = |event: &crate::common::Event| matches!(event, crate::common::Event::FrameDone);
    assert!(client.take_events().iter().any(done));
}

/// A window offered in the empty half, as [`waiting`] has it, with the
/// icons installed in `data` to be found.
fn waiting_with_icons(
    config: &[(&str, &str)],
    data: &std::path::Path,
) -> (Session, Client, Toplevel) {
    let mut session = Session::with_config(config);
    session
        .compositor_mut()
        .set_data_directories(vec![data.to_owned()]);
    let mut client = session.connect();
    let waiting = session.open(&mut client, "waiting", (400, 300), RED);
    session.open(&mut client, "first", (200, 100), BLUE);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    session.step();
    (session, client, waiting)
}

/// Whether `colour` is anywhere on the line a window offered has its title
/// on.
fn on_title_line(picture: &crate::common::Picture, colour: u32) -> bool {
    let middle = i32::midpoint(TITLE.1, TITLE.3);
    (TITLE.0..TITLE.2).any(|x| picture.at(x, middle) == colour)
}

#[test]
fn a_window_offered_has_its_icon_before_its_title_or_the_one_its_program_gave() {
    let data = crate::switcher::installed(&[("waiting", YELLOW)], &[]);
    let (mut session, mut client, waiting) = waiting_with_icons(&[], data.path());
    assert!(on_title_line(&session.draw(), YELLOW), "its application's");
    assert_eq!(session.draw().at(480, 226), RED, "and its picture");
    client.set_icon(&waiting, None, Some((32, MAGENTA)));
    session.roundtrip(&mut client);
    point(&mut session, 100, 240);
    let picture = session.draw();
    assert!(on_title_line(&picture, MAGENTA), "its program's own");
    assert!(!on_title_line(&picture, YELLOW));
}

#[test]
fn a_private_window_offered_is_shown_by_its_icon_in_its_blank() {
    let data = crate::switcher::installed(&[("waiting", YELLOW)], &[]);
    let rules = "rules:\n  - match: { app_id: waiting }\n    private: true\n";
    let theme = "capture { background: #00ff00; }";
    let config = [("rules.yaml", rules), ("theme.css", theme)];
    let (mut session, mut client, waiting) = waiting_with_icons(&config, data.path());
    let picture = session.draw();
    assert_eq!(picture.at(480, 226), YELLOW, "in the middle of its blank");
    assert_eq!(picture.at(360, 140), GREEN, "which is round it");
    assert!(on_title_line(&picture, YELLOW), "and before its title");
    client.set_icon(&waiting, None, Some((32, MAGENTA)));
    session.roundtrip(&mut client);
    point(&mut session, 100, 240);
    assert_eq!(session.draw().at(480, 226), MAGENTA, "its program's own");
}
