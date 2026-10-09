//! Moving and resizing frames with the pointer, and dragging tabs.

use omorfos::Input;
use omorfos_core::geometry::{Point, Rect, Size};
use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

use crate::common::{Client, Event, Session, key};
use crate::windows::{BLUE, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
const MIDDLE: u32 = 0x112;

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

fn move_to(session: &mut Session, x: i32, y: i32) {
    session.compositor.input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
}

fn press(session: &mut Session, button: u32) {
    session.compositor.input(
        Input::Button {
            button,
            pressed: true,
        },
        0,
    );
}

fn release(session: &mut Session, button: u32) {
    session.compositor.input(
        Input::Button {
            button,
            pressed: false,
        },
        0,
    );
}

/// Presses `button` at `from`, moves to `to` and lets go.
fn drag(session: &mut Session, button: u32, from: (i32, i32), to: (i32, i32)) {
    move_to(session, from.0, from.1);
    press(session, button);
    move_to(session, to.0, to.1);
    release(session, button);
    session.step();
}

/// The geometry of each frame on screen, bottom to top.
fn frames(session: &Session) -> Vec<Rect> {
    session
        .compositor
        .desktop()
        .frames()
        .map(|frame| frame.geometry)
        .collect()
}

fn tab_counts(session: &Session) -> Vec<usize> {
    session
        .compositor
        .desktop()
        .frames()
        .map(|frame| frame.tabs.len())
        .collect()
}

fn shown_app(session: &Session) -> String {
    let desktop = session.compositor.desktop();
    desktop
        .frames()
        .last()
        .map(|frame| frame.window().app_id.clone())
        .unwrap_or_default()
}

/// A decorated 200 by 100 window: a 202 by 129 frame at the corner.
fn decorated(session: &mut Session, client: &mut Client, app_id: &str) {
    session.open_decorated(client, app_id, (200, 100), RED);
}

fn buttons(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::PointerButton { .. }))
        .count()
}

#[test]
fn dragging_the_titlebar_moves_the_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    client.take_events();

    drag(&mut session, LEFT, (50, 10), (150, 110));
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(100, 100, 202, 129)]);
    assert_eq!(
        session.draw().at(101, 128),
        RED,
        "the window moved with its frame"
    );
    assert_eq!(
        buttons(&client.take_events()),
        0,
        "the window saw none of it"
    );
}

#[test]
fn dragging_a_border_resizes_the_frame_and_the_window_is_told() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    client.take_events();

    drag(&mut session, LEFT, (201, 60), (301, 60));
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(0, 0, 302, 129)]);
    assert_eq!(client.last_configure().0, (300, 100));
}

#[test]
fn a_frame_can_be_grabbed_just_outside_its_corner() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");

    drag(&mut session, LEFT, (204, 131), (254, 181));

    assert_eq!(frames(&session), [rect(0, 0, 252, 179)]);
}

#[test]
fn a_frame_cannot_be_made_smaller_than_a_small_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");

    drag(&mut session, LEFT, (201, 128), (-500, -500));

    assert_eq!(
        frames(&session),
        [rect(0, 0, 66, 61)],
        "a 64 by 32 window plus its frame"
    );
}

#[test]
fn alt_and_the_left_button_move_a_window_from_anywhere_on_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "plain", (200, 100), RED);
    client.take_events();

    session.compositor.input(
        Input::Key {
            code: key::ALT,
            pressed: true,
        },
        0,
    );
    drag(&mut session, LEFT, (100, 50), (300, 250));
    session.compositor.input(
        Input::Key {
            code: key::ALT,
            pressed: false,
        },
        0,
    );
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(200, 200, 200, 100)]);
    assert_eq!(buttons(&client.take_events()), 0);
}

#[test]
fn alt_and_the_right_button_resize_by_the_nearest_corner() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "plain", (200, 100), RED);
    session.compositor.input(
        Input::Key {
            code: key::ALT,
            pressed: true,
        },
        0,
    );

    drag(&mut session, RIGHT, (150, 80), (250, 130));
    assert_eq!(frames(&session), [rect(0, 0, 300, 150)], "bottom right");

    drag(&mut session, RIGHT, (10, 10), (-90, -40));
    assert_eq!(frames(&session), [rect(-100, -50, 400, 200)], "top left");
}

#[test]
fn a_maximized_frame_stays_put() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "foot", (200, 100), RED);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    let maximized = frames(&session);

    drag(&mut session, LEFT, (50, 10), (150, 110));
    drag(&mut session, LEFT, (0, 200), (100, 200));

    assert_eq!(frames(&session), maximized);
    assert!(
        session
            .compositor
            .desktop()
            .frames()
            .next()
            .unwrap()
            .maximized
    );
}

#[test]
fn a_click_on_a_titlebar_focuses_and_raises_its_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "below");
    decorated(&mut session, &mut client, "above");

    // The lower frame's titlebar shows above and left of the upper frame.
    move_to(&mut session, 100, 10);
    press(&mut session, RIGHT);
    release(&mut session, RIGHT);
    session.step();

    assert_eq!(shown_app(&session), "below");
    assert_eq!(
        frames(&session).len(),
        2,
        "a right click does not start a drag"
    );
}

#[test]
fn a_window_can_ask_to_be_moved_or_resized() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "own-titlebar", (200, 100), RED);

    // The client starts the move when its own titlebar is pressed.
    move_to(&mut session, 50, 10);
    press(&mut session, LEFT);
    session.roundtrip(&mut client);
    window.role._move(client.seat(), client.state.serial);
    session.roundtrip(&mut client);
    move_to(&mut session, 150, 60);
    release(&mut session, LEFT);
    session.step();
    assert_eq!(frames(&session), [rect(100, 50, 200, 100)]);

    move_to(&mut session, 299, 149);
    press(&mut session, LEFT);
    session.roundtrip(&mut client);
    window
        .role
        .resize(client.seat(), client.state.serial, ResizeEdge::BottomRight);
    session.roundtrip(&mut client);
    move_to(&mut session, 349, 179);
    release(&mut session, LEFT);
    session.step();
    assert_eq!(frames(&session), [rect(100, 50, 250, 130)]);

    move_to(&mut session, 101, 51);
    press(&mut session, LEFT);
    session.roundtrip(&mut client);
    window
        .role
        .resize(client.seat(), client.state.serial, ResizeEdge::TopLeft);
    session.roundtrip(&mut client);
    move_to(&mut session, 91, 41);
    release(&mut session, LEFT);
    session.step();
    assert_eq!(frames(&session), [rect(90, 40, 260, 140)]);
}

#[test]
fn requests_about_a_window_that_is_not_shown_yet_are_ignored() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel("early", "");
    session.roundtrip(&mut client);

    window.role._move(client.seat(), 0);
    window.role.resize(client.seat(), 0, ResizeEdge::Right);
    session.roundtrip(&mut client);
    move_to(&mut session, 100, 100);

    assert_eq!(frames(&session), []);
}

#[test]
fn a_tab_dropped_on_another_titlebar_joins_that_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "editor");
    session.open_decorated(&mut client, "terminal", (200, 100), BLUE);
    assert_eq!(tab_counts(&session), [1, 1]);

    // Carry the upper frame's tab, at (32, 32), to the lower one's titlebar.
    drag(&mut session, MIDDLE, (100, 40), (100, 10));
    session.roundtrip(&mut client);

    assert_eq!(tab_counts(&session), [2]);
    assert_eq!(shown_app(&session), "terminal", "the dropped tab is shown");
    assert_eq!(
        frames(&session),
        [rect(0, 0, 202, 129)],
        "in the frame it was dropped on"
    );
    assert_eq!(session.draw().at(100, 60), BLUE);
}

#[test]
fn clicking_a_tab_shows_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "editor");
    session.open_decorated(&mut client, "terminal", (200, 100), BLUE);
    drag(&mut session, MIDDLE, (100, 40), (100, 10));
    session.roundtrip(&mut client);

    // Two tabs share the 202 pixel titlebar: the editor's is the left half.
    move_to(&mut session, 50, 10);
    press(&mut session, LEFT);
    release(&mut session, LEFT);
    session.roundtrip(&mut client);

    assert_eq!(shown_app(&session), "editor");
    assert_eq!(session.draw().at(100, 60), RED);
}

#[test]
fn a_tab_dropped_away_from_any_titlebar_gets_its_own_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "editor");
    session.open_decorated(&mut client, "terminal", (200, 100), BLUE);
    drag(&mut session, MIDDLE, (100, 40), (100, 10));
    session.roundtrip(&mut client);
    assert_eq!(tab_counts(&session), [2]);

    drag(&mut session, MIDDLE, (150, 10), (500, 400));
    session.roundtrip(&mut client);

    assert_eq!(tab_counts(&session), [1, 1]);
    assert_eq!(shown_app(&session), "terminal");
}

#[test]
fn a_tab_dropped_back_on_its_own_titlebar_stays_where_it_was() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "editor");

    drag(&mut session, MIDDLE, (50, 10), (150, 10));

    assert_eq!(tab_counts(&session), [1]);
    assert_eq!(frames(&session), [rect(0, 0, 202, 129)]);
}

#[test]
fn other_buttons_are_swallowed_while_a_drag_is_in_progress() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    client.take_events();

    move_to(&mut session, 50, 10);
    press(&mut session, LEFT);
    press(&mut session, RIGHT);
    move_to(&mut session, 150, 110);
    release(&mut session, RIGHT);
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(100, 100, 202, 129)]);
    assert_eq!(buttons(&client.take_events()), 0);

    // The release ended the drag, so further motion moves nothing.
    move_to(&mut session, 400, 400);
    session.step();
    assert_eq!(frames(&session), [rect(100, 100, 202, 129)]);
}
