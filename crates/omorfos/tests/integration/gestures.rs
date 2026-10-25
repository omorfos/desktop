//! Gestures on a touchpad: three fingers swiped across go from one
//! workspace to the next, and every other gesture is the program's.

use omorfos::{Gesture, Input};

use crate::common::{Client, Event, Session};
use crate::lock::lock;
use crate::windows::RED;

/// Makes a gesture on the touchpad, step by step.
fn touch(session: &mut Session, steps: &[Gesture]) {
    for step in steps {
        session.compositor_mut().input(Input::Gesture(*step), 0);
    }
    session.step();
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

fn active(session: &Session) -> usize {
    session.compositor().desktop().active_workspace()
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

#[test]
fn three_fingers_swiped_left_bring_the_next_workspace_and_right_the_one_before() {
    let mut session = Session::with_workspaces();
    assert_eq!(active(&session), 0);

    touch(&mut session, &swipe(3, (-150.0, 20.0), false));
    assert_eq!(active(&session), 1, "as a page turned");
    touch(&mut session, &swipe(3, (-150.0, 0.0), false));
    assert_eq!(active(&session), 2);
    touch(&mut session, &swipe(3, (150.0, -20.0), false));
    assert_eq!(active(&session), 1, "and back");
}

#[test]
fn a_swipe_that_goes_up_or_down_not_far_enough_or_is_cancelled_comes_to_nothing() {
    let mut session = Session::with_workspaces();

    touch(&mut session, &swipe(3, (-120.0, 200.0), false));
    assert_eq!(active(&session), 0, "more down than across");
    touch(&mut session, &swipe(3, (-60.0, 0.0), false));
    assert_eq!(active(&session), 0, "not far enough");
    touch(&mut session, &swipe(3, (-300.0, 0.0), true));
    assert_eq!(active(&session), 0, "cancelled");
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

    assert_eq!(active(&session), 0, "no workspace changes");
    assert_eq!(told(&mut client), [], "and nothing behind the lock is told");
}
