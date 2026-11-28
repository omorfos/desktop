//! Moving and resizing frames with the pointer, and dragging tabs.

use omorfos::Input;
use omorfos_core::geometry::{Point, Rect, Size};
use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

use crate::common::{Client, Event, Session, key};
use crate::windows::{BACKGROUND, BLUE, RED, shaded};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
pub(crate) const MIDDLE: u32 = 0x112;

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

fn move_to(session: &mut Session, x: i32, y: i32) {
    session.compositor_mut().input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
}

fn press(session: &mut Session, button: u32) {
    session.compositor_mut().input(
        Input::Button {
            button,
            pressed: true,
        },
        0,
    );
}

fn release(session: &mut Session, button: u32) {
    session.compositor_mut().input(
        Input::Button {
            button,
            pressed: false,
        },
        0,
    );
}

/// Presses `button` at `from`, moves to `to` and lets go.
pub(crate) fn drag(session: &mut Session, button: u32, from: (i32, i32), to: (i32, i32)) {
    move_to(session, from.0, from.1);
    press(session, button);
    move_to(session, to.0, to.1);
    release(session, button);
    session.step();
}

/// The geometry of each frame on screen, bottom to top.
fn frames(session: &Session) -> Vec<Rect> {
    session
        .compositor()
        .desktop()
        .frames()
        .map(|frame| frame.geometry)
        .collect()
}

fn tab_counts(session: &Session) -> Vec<usize> {
    session
        .compositor()
        .desktop()
        .frames()
        .map(|frame| frame.tabs.len())
        .collect()
}

fn shown_app(session: &Session) -> String {
    let desktop = session.compositor().desktop();
    desktop
        .frames()
        .last()
        .map(|frame| frame.window().app_id.clone())
        .unwrap_or_default()
}

/// A decorated 200 by 100 window: a 202 by 149 frame at the corner.
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

    drag(&mut session, LEFT, (100, 10), (200, 110));
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(100, 100, 202, 149)]);
    assert_eq!(
        session.draw().at(101, 150),
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

    assert_eq!(frames(&session), [rect(0, 0, 302, 149)]);
    assert_eq!(client.last_configure().0, (300, 100));
}

#[test]
fn a_window_dragged_larger_fills_its_frame_until_it_draws_and_keeps_its_corners_round() {
    let moving = ("theme.css", "window { transition: 180ms ease-out; }\n");
    let mut session = Session::with_config(&[moving]);
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    session.advance(std::time::Duration::from_secs(1));

    drag(&mut session, LEFT, (201, 60), (301, 60));
    assert_eq!(frames(&session), [rect(0, 0, 302, 149)]);
    let picture = session.draw();
    assert_eq!(picture.at(295, 60), RED, "the window across the frame");
    assert_eq!(picture.at(295, 142), RED, "down to its rounded corner");
    let outside = picture.at(301, 148);
    assert!(shaded(outside, BACKGROUND), "outside it: {outside:#x}");
}

#[test]
fn a_frame_can_be_grabbed_just_outside_its_corner() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");

    drag(&mut session, LEFT, (204, 151), (254, 201));

    assert_eq!(frames(&session), [rect(0, 0, 252, 199)]);
}

#[test]
fn a_frame_cannot_be_made_smaller_than_a_small_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");

    drag(&mut session, LEFT, (201, 148), (-500, -500));

    assert_eq!(
        frames(&session),
        [rect(0, 0, 66, 81)],
        "a 64 by 32 window plus its frame"
    );
}

#[test]
fn alt_and_the_left_button_move_a_window_from_anywhere_on_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "plain", (200, 100), RED);
    client.take_events();

    session.compositor_mut().input(
        Input::Key {
            code: key::ALT,
            pressed: true,
        },
        0,
    );
    drag(&mut session, LEFT, (100, 50), (300, 250));
    session.compositor_mut().input(
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
    session.compositor_mut().input(
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

    drag(&mut session, LEFT, (100, 10), (200, 110));
    drag(&mut session, LEFT, (0, 200), (100, 200));

    assert_eq!(frames(&session), maximized);
    assert!(
        session
            .compositor()
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
    window.role.set_maximized();
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

    // Carry the upper frame's tab, at (32, 32) and clear of its buttons,
    // to the lower one's titlebar.
    drag(&mut session, MIDDLE, (150, 40), (100, 10));
    session.roundtrip(&mut client);

    assert_eq!(tab_counts(&session), [2]);
    assert_eq!(shown_app(&session), "terminal", "the dropped tab is shown");
    assert_eq!(
        frames(&session),
        [rect(0, 0, 202, 149)],
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
    drag(&mut session, MIDDLE, (150, 40), (100, 10));
    session.roundtrip(&mut client);

    // Two tabs share what the buttons leave of the 202 pixel titlebar,
    // the 122 at its right: the editor's is the left half of that.
    move_to(&mut session, 100, 10);
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
    drag(&mut session, MIDDLE, (150, 40), (100, 10));
    session.roundtrip(&mut client);
    assert_eq!(tab_counts(&session), [2]);

    // The terminal's tab is the right half of what the buttons leave.
    drag(&mut session, MIDDLE, (170, 10), (500, 400));
    session.roundtrip(&mut client);

    assert_eq!(tab_counts(&session), [1, 1]);
    assert_eq!(shown_app(&session), "terminal");
}

#[test]
fn a_tab_dropped_back_on_its_own_titlebar_stays_where_it_was() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "editor");

    drag(&mut session, MIDDLE, (100, 10), (130, 10));

    assert_eq!(tab_counts(&session), [1]);
    assert_eq!(frames(&session), [rect(0, 0, 202, 149)]);
}

#[test]
fn other_buttons_are_swallowed_while_a_drag_is_in_progress() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    client.take_events();

    move_to(&mut session, 100, 10);
    press(&mut session, LEFT);
    press(&mut session, RIGHT);
    move_to(&mut session, 200, 110);
    release(&mut session, RIGHT);
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [rect(100, 100, 202, 149)]);
    assert_eq!(buttons(&client.take_events()), 0);

    // The release ended the drag, so further motion moves nothing.
    move_to(&mut session, 400, 400);
    session.step();
    assert_eq!(frames(&session), [rect(100, 100, 202, 149)]);
}

/// Presses and lets go of `code`, as a key alone, or with Shift held.
fn tap(session: &mut Session, code: smithay::input::keyboard::Keycode, shift: bool) {
    let shift_key = smithay::input::keyboard::Keycode::new(50);
    let mut keys = vec![(code, true), (code, false)];
    if shift {
        keys.insert(0, (shift_key, true));
        keys.push((shift_key, false));
    }
    for (code, pressed) in keys {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
}

fn perform(session: &mut Session, action: &omorfos_config::Action) {
    session.compositor_mut().perform(action);
    session.step();
}

#[test]
fn a_window_is_moved_by_the_keyboard_with_no_pointer_event() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    client.take_events();
    perform(&mut session, &omorfos_config::Action::Move);
    for _ in 0..4 {
        tap(&mut session, key::RIGHT, false);
    }
    tap(&mut session, key::DOWN, true);
    tap(&mut session, key::RETURN, false);
    assert_eq!(frames(&session), [rect(40, 50, 202, 149)]);
    // The window saw none of it, and the arrows are its own again.
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert_eq!(buttons(&events), 0);
    // Shift, a modifier, is the window's still, so that it is not left
    // held; the arrows and Enter are not.
    let shift = 50 - 8;
    let keys = |events: &[Event]| {
        let pressed = events
            .iter()
            .filter(|event| matches!(event, Event::Key { pressed: true, code } if *code != shift));
        pressed.count()
    };
    assert_eq!(keys(&events), 0, "{events:?}");
    tap(&mut session, key::RIGHT, false);
    session.roundtrip(&mut client);
    assert_eq!(keys(&client.take_events()), 1, "the window's");
    assert_eq!(frames(&session), [rect(40, 50, 202, 149)]);
}

#[test]
fn escape_puts_a_window_moved_or_resized_by_the_keyboard_back() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    perform(&mut session, &omorfos_config::Action::Move);
    tap(&mut session, key::RIGHT, false);
    tap(&mut session, key::DOWN, false);
    assert_eq!(
        frames(&session),
        [rect(10, 10, 202, 149)],
        "it moves as it goes"
    );
    tap(&mut session, key::ESCAPE, false);
    assert_eq!(frames(&session), [rect(0, 0, 202, 149)], "put back");
    perform(&mut session, &omorfos_config::Action::Resize);
    tap(&mut session, key::RIGHT, false);
    tap(&mut session, key::ESCAPE, false);
    assert_eq!(
        frames(&session),
        [rect(0, 0, 202, 149)],
        "its size put back"
    );
}

#[test]
fn a_window_is_resized_by_the_keyboard_from_its_corner_and_no_smaller_than_a_drag_lets_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    perform(&mut session, &omorfos_config::Action::Resize);
    for _ in 0..3 {
        tap(&mut session, key::RIGHT, false);
    }
    tap(&mut session, key::DOWN, true);
    tap(&mut session, key::RETURN, false);
    assert_eq!(frames(&session), [rect(0, 0, 232, 199)]);
    // Made smaller and smaller, it stops where a drag would.
    perform(&mut session, &omorfos_config::Action::Resize);
    for _ in 0..20 {
        tap(&mut session, key::LEFT, true);
        tap(&mut session, key::UP, true);
    }
    tap(&mut session, key::RETURN, false);
    let smallest = frames(&session)[0];
    assert_eq!(smallest.origin, Point::new(0, 0), "the corner stays");
    assert!(
        smallest.size.width > 0 && smallest.size.height > 0,
        "{smallest:?}"
    );
    // By its right edge, just outside the frame, as far left as it goes.
    let edge = i32::try_from(smallest.size.width).unwrap() + 2;
    drag(&mut session, LEFT, (edge, 60), (0, 60));
    assert_eq!(frames(&session)[0], smallest, "as small as a drag makes it");
    // And it is small: far smaller than it was.
    assert!(
        smallest.size.width < 202 && smallest.size.height < 149,
        "{smallest:?}"
    );
}

#[test]
fn while_a_window_is_moved_by_the_keyboard_other_keys_reach_nothing_and_a_click_keeps_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    session.roundtrip(&mut client);
    client.take_events();
    perform(&mut session, &omorfos_config::Action::Move);
    tap(&mut session, key::RIGHT, false);
    tap(
        &mut session,
        smithay::input::keyboard::Keycode::new(38),
        false,
    );
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Key { .. })),
        "{events:?}"
    );
    move_to(&mut session, 500, 400);
    press(&mut session, LEFT);
    release(&mut session, LEFT);
    session.step();
    tap(&mut session, key::RIGHT, false);
    assert_eq!(
        frames(&session),
        [rect(10, 0, 202, 149)],
        "kept at the click"
    );
}

#[test]
fn a_maximized_window_is_not_moved_or_resized_by_the_keyboard() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    perform(&mut session, &omorfos_config::Action::Maximize);
    let maximized = frames(&session);
    session.roundtrip(&mut client);
    client.take_events();
    for action in [omorfos_config::Action::Move, omorfos_config::Action::Resize] {
        perform(&mut session, &action);
        tap(&mut session, key::RIGHT, false);
    }
    assert_eq!(frames(&session), maximized);
    session.roundtrip(&mut client);
    let events = client.take_events();
    let keys = events
        .iter()
        .filter(|event| matches!(event, Event::Key { pressed: true, .. }));
    assert_eq!(keys.count(), 2, "the window's: {events:?}");
}

#[test]
fn the_window_menu_opens_by_its_action_and_begins_a_move() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    move_to(&mut session, 600, 400);
    perform(&mut session, &omorfos_config::Action::WindowMenu);
    assert!(session.compositor().menu_open());
    // Its last row but one, Move.
    tap(&mut session, key::UP, false);
    tap(&mut session, key::UP, false);
    tap(&mut session, key::RETURN, false);
    assert!(!session.compositor().menu_open());
    tap(&mut session, key::RIGHT, false);
    tap(&mut session, key::RETURN, false);
    assert_eq!(frames(&session), [rect(10, 0, 202, 149)]);
}

#[test]
fn a_window_that_closes_while_moved_by_the_keyboard_leaves_the_keys_to_the_next() {
    let mut session = Session::new();
    let mut other = session.connect();
    decorated(&mut session, &mut other, "other");
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "foot", (200, 100), RED);
    perform(&mut session, &omorfos_config::Action::Move);
    window.role.destroy();
    window.xdg_surface.destroy();
    window.surface.destroy();
    session.roundtrip(&mut client);
    session.step();
    session.roundtrip(&mut other);
    other.take_events();
    tap(&mut session, key::RIGHT, false);
    session.roundtrip(&mut other);
    let events = other.take_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Key { pressed: true, .. })),
        "{events:?}"
    );
}

#[test]
fn a_window_that_loses_the_keyboard_while_moved_by_it_is_left_where_it_is() {
    let mut session = Session::new();
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    perform(&mut session, &omorfos_config::Action::Move);
    tap(&mut session, key::RIGHT, false);
    // Another window comes and takes the keyboard: the arrows are its.
    let mut other = session.connect();
    session.open(&mut other, "other", (100, 100), BLUE);
    session.step();
    session.roundtrip(&mut other);
    other.take_events();
    tap(&mut session, key::RIGHT, false);
    session.roundtrip(&mut other);
    let events = other.take_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Key { pressed: true, .. })),
        "{events:?}"
    );
    assert_eq!(
        frames(&session)[0],
        rect(10, 0, 202, 149),
        "left where it was moved to"
    );
}

#[test]
fn the_window_menu_opens_on_the_windows_screen_though_the_pointer_is_on_another() {
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    let mut client = session.connect();
    decorated(&mut session, &mut client, "foot");
    // Moved onto the second screen, by the keyboard.
    perform(&mut session, &omorfos_config::Action::Move);
    for _ in 0..14 {
        tap(&mut session, key::RIGHT, true);
    }
    tap(&mut session, key::RETURN, false);
    let corner = frames(&session)[0].origin;
    let second_left = session.compositor().area_of(&second).loc.x;
    assert!(corner.x >= second_left, "{corner:?}");
    // The pointer is on the first screen still, the window's the current.
    assert_eq!(session.compositor().menu_area(), None, "none yet");
    perform(&mut session, &omorfos_config::Action::WindowMenu);
    let menu = session.compositor().menu_area().unwrap();
    assert!(
        menu.origin.x >= second_left,
        "on the window's screen: {menu:?}"
    );
}
