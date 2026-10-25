//! Gestures on a touchpad: three fingers swiped across go from one
//! workspace to the next, and every other gesture is the program's.

use std::time::Duration;

use omorfos::{Gesture, Input};
use omorfos_config::{Action, WorkspaceNumber};

use crate::common::{Client, Event, FOUR_WORKSPACES, Session};
use crate::lock::lock;
use crate::windows::{BACKGROUND, BLUE, RED};

/// Makes a gesture on the touchpad, step by step, each at its time in
/// milliseconds.
fn touch_at(session: &mut Session, steps: &[(Gesture, u32)]) {
    for (step, time) in steps {
        session.compositor_mut().input(Input::Gesture(*step), *time);
    }
    session.step();
}

/// Makes a gesture on the touchpad, step by step, all at once.
fn touch(session: &mut Session, steps: &[Gesture]) {
    let timed: Vec<(Gesture, u32)> = steps.iter().map(|step| (*step, 0)).collect();
    touch_at(session, &timed);
}

/// A swipe of `fingers` that goes by `(dx, dy)` in two steps, and is
/// finished or `cancelled`.
fn swipe(fingers: u32, (dx, dy): (f64, f64), cancelled: bool) -> [Gesture; 4] {
    let half = Gesture::SwipeUpdate {
        dx: dx / 2.0,
        dy: dy / 2.0,
    };
    [
        Gesture::SwipeBegin { fingers },
        half,
        half,
        Gesture::SwipeEnd { cancelled },
    ]
}

/// Three fingers going across by `dx` in ten steps over `taking`
/// milliseconds, without lifting.
fn carry(session: &mut Session, dx: f64, taking: u32) {
    let step = Gesture::SwipeUpdate {
        dx: dx / 10.0,
        dy: 0.0,
    };
    let steps = (1..=10).map(|at| (step, 1000 + taking * at / 10));
    let begin = (Gesture::SwipeBegin { fingers: 3 }, 1000);
    let steps: Vec<(Gesture, u32)> = [begin].into_iter().chain(steps).collect();
    touch_at(session, &steps);
}

/// The fingers lifting, or the swipe being cancelled.
fn lift(session: &mut Session, cancelled: bool) {
    touch(session, &[Gesture::SwipeEnd { cancelled }]);
}

fn active(session: &Session) -> usize {
    session.compositor().desktop().active_workspace()
}

fn slid(session: &Session) -> i32 {
    session.compositor().desktop().slid()
}

/// What the client was told of gestures.
fn told(client: &mut Client) -> Vec<Gesture> {
    let events = client.take_events().into_iter();
    let told = events.filter_map(|event| match event {
        Event::Gesture(gesture) => Some(gesture),
        _ => None,
    });
    told.collect()
}

/// Four workspaces, a red window on the first, which is shown, and a blue
/// one on the second.
fn two_windows() -> (Session, Client) {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.open(&mut client, "second", (200, 100), BLUE);
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.step();
    (session, client)
}

#[test]
fn three_fingers_carry_the_workspace_shown_and_bring_the_next_in_from_the_right() {
    let (mut session, _client) = two_windows();
    assert_eq!(session.draw().at(545, 5), BACKGROUND);

    carry(&mut session, -100.0, 100);
    assert_eq!(slid(&session), -100, "with no easing at all");
    let picture = session.draw();
    assert_eq!(picture.at(5, 5), RED, "the first gone a little way left");
    assert_eq!(picture.at(545, 5), BLUE, "and the second in after it");
    assert_eq!(active(&session), 0, "still the one shown");
}

#[test]
fn let_go_past_half_way_the_workspace_that_came_in_is_shown_and_settles() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -400.0, 1000);

    lift(&mut session, false);
    assert_eq!(active(&session), 1, "shown at once");
    assert_eq!(slid(&session), 240, "from where it had got to");
    assert!(session.compositor().deadline().is_some(), "and drawn again");
    session.advance(Duration::from_millis(120));
    let halfway = slid(&session);
    assert!(0 < halfway && halfway < 240, "{halfway}");
    session.advance(Duration::from_millis(120));
    assert_eq!(slid(&session), 0, "where it belongs");
    assert_eq!(session.draw().at(5, 5), BLUE);
}

#[test]
fn let_go_short_of_half_way_the_workspace_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -200.0, 1000);

    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    assert_eq!(slid(&session), -200);
    session.advance(Duration::from_millis(240));
    assert_eq!(slid(&session), 0);
    assert_eq!(session.draw().at(5, 5), RED);
}

#[test]
fn a_quick_flick_turns_the_page_however_short() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -100.0, 20);

    lift(&mut session, false);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_swipe_taken_back_half_way_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -400.0, 1000);
    let back = [
        (Gesture::SwipeUpdate { dx: 125.0, dy: 0.0 }, 2025),
        (Gesture::SwipeUpdate { dx: 125.0, dy: 0.0 }, 2050),
    ];
    touch_at(&mut session, &back);
    assert_eq!(slid(&session), -150);

    lift(&mut session, false);
    assert_eq!(active(&session), 0, "the way they were going decides");
}

#[test]
fn a_cancelled_swipe_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -500.0, 1000);

    lift(&mut session, true);
    assert_eq!(active(&session), 0);
}

#[test]
fn three_fingers_right_bring_the_one_before_in_from_the_left() {
    let (mut session, _client) = two_windows();
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    carry(&mut session, 400.0, 1000);
    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    assert_eq!(slid(&session), 400 - 640);
}

#[test]
fn with_one_workspace_it_goes_a_little_way_with_the_fingers_and_back() {
    let mut session = Session::new();
    carry(&mut session, -400.0, 100);
    assert_eq!(slid(&session), -100, "a quarter of the way");

    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    session.advance(Duration::from_millis(240));
    assert_eq!(slid(&session), 0);
}

#[test]
fn a_swipe_begun_while_the_last_settles_takes_it_up_from_where_it_is() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -400.0, 1000);
    lift(&mut session, false);
    session.advance(Duration::from_millis(60));
    let there = slid(&session);

    carry(&mut session, -50.0, 1000);
    assert_eq!(slid(&session), there - 50);
    session.advance(Duration::from_millis(240));
    assert_eq!(slid(&session), there - 50, "held by the fingers");
}

#[test]
fn a_swipe_of_three_fingers_is_not_the_programs() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "map", (200, 100), RED);
    let _gestures = client.gestures();
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    client.take_events();

    touch(&mut session, &swipe(3, (0.0, 40.0), false));
    session.roundtrip(&mut client);
    assert_eq!(told(&mut client), []);
}

#[test]
fn every_other_gesture_is_the_programs_under_the_pointer() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "map", (200, 100), RED);
    let _gestures = client.gestures();
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    client.take_events();

    let four = swipe(4, (-300.0, 0.0), false);
    let pinch = [
        Gesture::PinchBegin { fingers: 2 },
        Gesture::PinchUpdate {
            dx: 1.0,
            dy: 2.0,
            scale: 1.5,
            rotation: 10.0,
        },
        Gesture::PinchEnd { cancelled: false },
    ];
    let hold = [
        Gesture::HoldBegin { fingers: 3 },
        Gesture::HoldEnd { cancelled: true },
    ];
    let all: Vec<Gesture> = [&four[..], &pinch, &hold].concat();
    touch(&mut session, &all);
    session.roundtrip(&mut client);

    assert_eq!(told(&mut client), all, "as they were made");
    assert_eq!(active(&session), 0, "and only theirs");
}

#[test]
fn a_gesture_does_nothing_while_the_session_is_locked() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "map", (200, 100), RED);
    let _gestures = client.gestures();
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    let (_locker, _lock, _surface) = lock(&mut session);
    client.take_events();

    touch(&mut session, &swipe(3, (-150.0, 0.0), false));
    touch(&mut session, &swipe(4, (-150.0, 0.0), false));
    session.roundtrip(&mut client);

    assert_eq!((active(&session), slid(&session)), (0, 0), "nothing moves");
    assert_eq!(told(&mut client), [], "and nothing behind the lock is told");
}

// --- The keys -----------------------------------------------------------

#[test]
fn the_keys_slide_the_next_in_from_the_right_and_the_one_before_from_the_left() {
    let (mut session, _client) = two_windows();

    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!(
        (active(&session), slid(&session)),
        (1, 640),
        "shown at once"
    );
    session.step();
    assert_eq!(session.draw().at(5, 5), RED, "from where the first was");
    session.advance(Duration::from_millis(120));
    let halfway = slid(&session);
    assert!(0 < halfway && halfway < 640, "{halfway}");
    session.advance(Duration::from_millis(120));
    assert_eq!(slid(&session), 0);
    assert_eq!(session.draw().at(5, 5), BLUE);

    session.compositor_mut().perform(&Action::PreviousWorkspace);
    assert_eq!((active(&session), slid(&session)), (0, -640));
}

#[test]
fn a_number_key_that_goes_further_slides_with_the_workspace_it_left_beside() {
    let (mut session, _client) = two_windows();

    let third = Action::Workspace(WorkspaceNumber::clamped(3));
    session.compositor_mut().perform(&third);
    assert_eq!((active(&session), slid(&session)), (2, 640));
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(5, 5), RED, "the first beside it, not the second");
    session.advance(Duration::from_millis(240));
    assert_eq!(session.draw().at(5, 5), BACKGROUND);
}

#[test]
fn a_key_pressed_again_before_they_settle_takes_them_on_from_where_they_are() {
    let (mut session, _client) = two_windows();
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.advance(Duration::from_millis(120));
    let there = slid(&session);

    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!((active(&session), slid(&session)), (2, there + 640));
}

#[test]
fn a_key_that_brings_the_workspace_already_shown_slides_nothing() {
    let (mut session, _client) = two_windows();
    let first = Action::Workspace(WorkspaceNumber::clamped(1));
    session.compositor_mut().perform(&first);
    assert_eq!((active(&session), slid(&session)), (0, 0));
}

#[test]
fn a_window_slides_off_and_in_with_its_workspace_without_fading() {
    let theme = "window { transition: 1000ms linear; }\n";
    let mut session =
        Session::with_config(&[("theme.css", theme), ("config.yaml", FOUR_WORKSPACES)]);
    let mut client = session.connect();
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    session.open(&mut client, "second", (200, 100), BLUE);
    session.advance(Duration::from_secs(1));
    assert_eq!(session.draw().at(5, 5), BLUE);

    session.compositor_mut().perform(&Action::PreviousWorkspace);
    // The first frame comes with the key.
    session.step();
    session.advance(Duration::from_millis(240));
    assert_eq!(
        session.draw().at(5, 5),
        BACKGROUND,
        "not left behind to fade"
    );

    session.compositor_mut().perform(&Action::NextWorkspace);
    // The first frame comes with the key.
    session.step();
    session.advance(Duration::from_millis(240));
    assert_eq!(session.draw().at(5, 5), BLUE, "nor coming in afresh");
}
