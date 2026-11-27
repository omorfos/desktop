//! The display key: the screens another way for now, side by side, the
//! same on all, or one alone, each kept only if the user says so in time.

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use smithay::input::keyboard::Keycode;
use smithay::output::Output;

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::BACKGROUND;

/// A session with a second screen, 800 by 600, to the right of the first,
/// with the time the compositor knows set going.
fn two_screens() -> (Session, Output) {
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    session.step();
    (session, second)
}

fn tap(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
}

/// Keeps the way tried, as a user does: the keyboard moved from Put back,
/// where it starts, to Keep, and Enter a moment after (ADR 111).
fn keep(session: &mut Session) {
    tap(session, key::TAB);
    session.advance(Duration::from_millis(500));
    tap(session, key::RETURN);
}

fn next(session: &mut Session) {
    session.compositor_mut().perform(&Action::NextScreens);
    session.step();
}

/// How the two screens stand: whether each stands in the space, and is
/// off.
fn stand(session: &Session, second: &Output) -> [(bool, bool); 2] {
    let compositor = session.compositor();
    [session.output(), second.clone()].map(|output| {
        let standing = !compositor.area_of(&output).is_empty();
        (standing, compositor.is_dark(&output))
    })
}

fn said(session: &Session, what: &str) -> bool {
    session
        .compositor()
        .events()
        .any(|event| event.contains(what))
}

const SIDE_BY_SIDE: [(bool, bool); 2] = [(true, false), (true, false)];
const MIRRORED: [(bool, bool); 2] = [(true, false), (false, false)];
const FIRST_ALONE: [(bool, bool); 2] = [(true, false), (false, true)];
const SECOND_ALONE: [(bool, bool); 2] = [(false, true), (true, false)];

#[test]
fn the_display_key_goes_through_the_ways_the_screens_can_stand() {
    let (mut session, second) = two_screens();
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    tap(&mut session, key::DISPLAY);
    assert_eq!(stand(&session, &second), MIRRORED);
    next(&mut session);
    assert_eq!(stand(&session, &second), FIRST_ALONE);
    next(&mut session);
    assert_eq!(stand(&session, &second), SECOND_ALONE);
    next(&mut session);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    next(&mut session);
    assert_eq!(stand(&session, &second), MIRRORED, "and round again");
}

#[test]
fn what_is_asked_says_the_way_the_screens_are_tried_in() {
    let (mut session, _second) = two_screens();
    // What is in the middle of the first screen, a row through it.
    let asked = |session: &mut Session| {
        let picture = session.draw();
        let row = HEIGHT / 2 - 20;
        (0..WIDTH).map(|x| picture.at(x, row)).collect::<Vec<u32>>()
    };
    next(&mut session);
    let mirrored = asked(&mut session);
    next(&mut session);
    let alone = asked(&mut session);
    assert_ne!(mirrored, alone);
}

#[test]
fn a_way_not_kept_in_time_goes_back_to_the_one_before() {
    let (mut session, second) = two_screens();
    next(&mut session);
    // The question is in the middle of each screen that is on.
    let middle = session.draw().at(WIDTH / 2, HEIGHT / 2);
    assert_ne!(middle, BACKGROUND);
    assert!(session.compositor().deadline().is_some());
    session.advance(Duration::from_secs(14));
    assert_eq!(stand(&session, &second), MIRRORED, "not yet");
    session.advance(Duration::from_secs(1));
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    assert!(said(&session, "back as they were"));
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}

#[test]
fn enter_keeps_a_way_and_escape_puts_the_one_before_back() {
    let (mut session, second) = two_screens();
    next(&mut session);
    // Other keys are anyone's, and leave it asked.
    tap(&mut session, key::A);
    assert_ne!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
    // Keep, the keyboard moved to it, keeps it only a moment after.
    tap(&mut session, key::TAB);
    tap(&mut session, key::RETURN);
    assert!(!said(&session, "kept as they are"), "too soon");
    session.advance(Duration::from_millis(500));
    tap(&mut session, key::RETURN);
    assert!(said(&session, "kept as they are"));
    session.advance(Duration::from_secs(30));
    assert_eq!(stand(&session, &second), MIRRORED, "kept");
    // The next is tried from the one kept, and goes back to it.
    next(&mut session);
    assert_eq!(stand(&session, &second), FIRST_ALONE);
    tap(&mut session, key::ESCAPE);
    assert_eq!(stand(&session, &second), MIRRORED);
    // Going round to the configuration's way and keeping it keeps no
    // other.
    for _ in 0..3 {
        next(&mut session);
    }
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    keep(&mut session);
    session.advance(Duration::from_secs(30));
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    // Enter and Escape are anyone's again once nothing is asked.
    tap(&mut session, key::ESCAPE);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
}

#[test]
fn a_screen_plugged_in_or_taken_out_ends_what_was_tried() {
    let (mut session, second) = two_screens();
    next(&mut session);
    let third = session.add_output((400, 300));
    session.step();
    let compositor = session.compositor();
    for output in [&second, &third] {
        assert!(!compositor.area_of(output).is_empty());
    }
    // And what was kept is for the screens it was kept with.
    next(&mut session);
    keep(&mut session);
    assert!(session.compositor().area_of(&second).is_empty());
    session.compositor_mut().remove_output(&third);
    session.step();
    assert!(!session.compositor().area_of(&second).is_empty());
}

#[test]
fn with_one_screen_there_is_no_other_way() {
    let mut session = Session::new();
    next(&mut session);
    assert!(said(&session, "only one screen"));
    // Nor is anything asked.
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}

#[test]
fn a_way_kept_is_how_the_screens_stand_at_the_next_login() {
    let state = tempfile::TempDir::new().unwrap();
    // Each login a session of its own, with the same screens.
    let login = || {
        let mut session = Session::new();
        session.compositor_mut().set_state_directory(state.path());
        let second = session.add_output((800, 600));
        session.step();
        (session, second)
    };
    let (mut session, second) = login();
    next(&mut session);
    keep(&mut session);
    assert_eq!(stand(&session, &second), MIRRORED);
    drop(session);

    let (mut session, second) = login();
    assert_eq!(stand(&session, &second), MIRRORED, "as it was kept");
    next(&mut session);
    keep(&mut session);
    assert_eq!(stand(&session, &second), FIRST_ALONE);
    drop(session);

    let (mut session, second) = login();
    assert_eq!(stand(&session, &second), FIRST_ALONE, "one alone, kept");
    // Going round to the configuration's way and keeping that keeps no
    // other, for the login after too.
    next(&mut session);
    next(&mut session);
    keep(&mut session);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    drop(session);
    let (session, second) = login();
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
}

/// Presses the left button on `at` and lets it go there.
fn click(session: &mut Session, at: (i32, i32)) {
    session.compositor_mut().input(
        Input::Motion {
            x: f64::from(at.0),
            y: f64::from(at.1),
        },
        0,
    );
    for pressed in [true, false] {
        let button = Input::Button {
            button: 0x110,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    session.step();
}

#[test]
fn escape_puts_the_screens_back_at_once_however_soon_or_late() {
    let (mut session, second) = two_screens();
    next(&mut session);
    tap(&mut session, key::ESCAPE);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE, "at once");
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
    // Put off by being there, past its 15 seconds, and then Escape.
    next(&mut session);
    for _ in 0..2 {
        session.advance(Duration::from_secs(10));
        session
            .compositor_mut()
            .input(Input::Motion { x: 9.0, y: 9.0 }, 0);
    }
    session.advance(Duration::from_secs(5));
    assert_eq!(stand(&session, &second), MIRRORED, "put off, at 25 seconds");
    tap(&mut session, key::ESCAPE);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE, "late");
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}

#[test]
fn enter_puts_the_screens_back_where_the_keyboard_starts() {
    let (mut session, second) = two_screens();
    next(&mut session);
    session.advance(Duration::from_secs(1));
    tap(&mut session, key::RETURN);
    assert_eq!(stand(&session, &second), SIDE_BY_SIDE);
    assert!(said(&session, "back as they were"));
}

#[test]
fn a_press_on_keep_keeps_the_screens_and_one_on_put_back_puts_them_back() {
    let (mut session, second) = two_screens();
    next(&mut session);
    session.advance(Duration::from_secs(1));
    let (_, keep) = session.compositor().keep_screens_buttons().unwrap();
    click(&mut session, keep);
    assert_eq!(stand(&session, &second), MIRRORED, "kept");
    assert!(said(&session, "kept as they are"));
    next(&mut session);
    let (put_back, _) = session.compositor().keep_screens_buttons().unwrap();
    click(&mut session, put_back);
    assert_eq!(stand(&session, &second), MIRRORED, "back to the way kept");
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}

#[test]
fn being_there_puts_off_putting_the_screens_back_to_two_minutes_at_most() {
    let (mut session, second) = two_screens();
    next(&mut session);
    for _ in 0..11 {
        session.advance(Duration::from_secs(10));
        session
            .compositor_mut()
            .input(Input::Motion { x: 9.0, y: 9.0 }, 0);
    }
    session.advance(Duration::from_secs(9));
    assert_eq!(
        stand(&session, &second),
        MIRRORED,
        "put off, at 119 seconds"
    );
    session
        .compositor_mut()
        .input(Input::Motion { x: 8.0, y: 8.0 }, 0);
    session.advance(Duration::from_secs(2));
    assert_eq!(
        stand(&session, &second),
        SIDE_BY_SIDE,
        "back by two minutes"
    );
}

#[test]
fn a_press_on_keep_let_go_off_the_question_keeps_nothing() {
    let (mut session, second) = two_screens();
    next(&mut session);
    session.advance(Duration::from_secs(1));
    let (_, keep) = session.compositor().keep_screens_buttons().unwrap();
    let at = |(x, y): (i32, i32)| Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    let button = |pressed| Input::Button {
        button: 0x110,
        pressed,
    };
    session.compositor_mut().input(at(keep), 0);
    session.compositor_mut().input(button(true), 0);
    session.compositor_mut().input(at((5, 5)), 0);
    session.compositor_mut().input(button(false), 0);
    session.step();
    assert!(!said(&session, "kept as they are"));
    assert_eq!(stand(&session, &second), MIRRORED, "still asked");
    assert_ne!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
    // A click off it altogether is the desktop's, and answers nothing.
    click(&mut session, (5, 5));
    assert!(!said(&session, "kept as they are"));
    assert_ne!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}

#[test]
fn space_is_left_to_programs_while_the_screens_are_asked_about() {
    let (mut session, second) = two_screens();
    next(&mut session);
    session.advance(Duration::from_secs(1));
    // As a slide on a projector is gone on with: not the question's.
    tap(&mut session, key::SPACE);
    assert_eq!(stand(&session, &second), MIRRORED, "still asked");
    assert!(!said(&session, "back as they were"));
    assert_ne!(session.draw().at(WIDTH / 2, HEIGHT / 2), BACKGROUND);
}
