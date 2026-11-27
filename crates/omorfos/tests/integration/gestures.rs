//! Gestures on a touchpad: three fingers swiped across go from one
//! workspace to the next, and every other gesture is the program's.

use std::time::Duration;

use omorfos::{Gesture, Input};
use omorfos_config::{Action, WorkspaceNumber};

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, FOUR_WORKSPACES, LayerShape, Session, WIDTH};
use crate::lock::lock;
use crate::windows::{BACKGROUND, BLUE, GREEN, MOVING, RED};

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

/// How far the fingers go across to carry the workspaces `across` units of
/// the screen: a whole workspace, the screen's width, is 400 of theirs.
fn fingers(across: f64) -> f64 {
    across * 400.0 / f64::from(WIDTH)
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

    carry(&mut session, fingers(-100.0), 100);
    assert_eq!(slid(&session), -100, "with no easing at all");
    let picture = session.draw();
    assert_eq!(picture.at(5, 5), RED, "the first gone a little way left");
    assert_eq!(picture.at(545, 5), BLUE, "and the second in after it");
    assert_eq!(active(&session), 0, "still the one shown");
}

#[test]
fn the_wallpaper_goes_with_the_workspaces_drawn_once_for_each() {
    let (mut session, mut client) = two_windows();
    // A wallpaper in the bottom left corner, where no window is.
    let corner = LayerShape {
        layer: Layer::Background,
        anchor: Anchor::Bottom | Anchor::Left,
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &corner, (100, 50), GREEN);
    let picture = session.draw();
    assert_eq!(
        (picture.at(50, 455), picture.at(590, 455)),
        (GREEN, BACKGROUND)
    );

    carry(&mut session, fingers(-100.0), 100);
    let picture = session.draw();
    assert_eq!(picture.at(50, 455), BACKGROUND, "gone left with the first");
    assert_eq!(picture.at(590, 455), GREEN, "and come in with the second");
    lift(&mut session, true);
    session.advance(Duration::from_millis(320));
    let picture = session.draw();
    assert_eq!(
        (picture.at(50, 455), picture.at(590, 455)),
        (GREEN, BACKGROUND)
    );
}

#[test]
fn the_wallpaper_is_drawn_on_the_workspace_a_swipe_is_to_make_before_it_is_made() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    for _ in 0..3 {
        session.compositor_mut().perform(&Action::NextWorkspace);
    }
    session.open(&mut client, "last", (200, 100), BLUE);
    let corner = LayerShape {
        layer: Layer::Background,
        anchor: Anchor::Bottom | Anchor::Left,
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &corner, (100, 50), GREEN);
    // The keys' slide settled.
    session.advance(Duration::from_secs(1));
    let count = session.compositor().desktop().workspace_count();

    // Past the last, towards a new one that is not yet there.
    carry(&mut session, fingers(-100.0), 100);
    assert_eq!(session.compositor().desktop().workspace_count(), count);
    assert_eq!(session.draw().at(590, 455), GREEN, "its wallpaper already");
}

#[test]
fn let_go_past_half_way_the_workspace_that_came_in_is_shown_and_settles() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-400.0), 1000);

    lift(&mut session, false);
    assert_eq!(active(&session), 1, "shown at once");
    assert_eq!(slid(&session), 240, "from where it had got to");
    assert!(session.compositor().deadline().is_some(), "and drawn again");
    session.advance(Duration::from_millis(160));
    let halfway = slid(&session);
    assert!(0 < halfway && halfway < 240, "{halfway}");
    session.advance(Duration::from_millis(160));
    assert_eq!(slid(&session), 0, "where it belongs");
    assert_eq!(session.draw().at(5, 5), BLUE);
}

#[test]
fn let_go_short_of_half_way_the_workspace_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-200.0), 1000);

    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    assert_eq!(slid(&session), -200);
    session.advance(Duration::from_millis(320));
    assert_eq!(slid(&session), 0);
    assert_eq!(session.draw().at(5, 5), RED);
}

#[test]
fn a_quick_flick_turns_the_page_however_short() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-100.0), 20);

    lift(&mut session, false);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_long_slow_swipe_past_half_way_turns_the_page_however_wide_the_screen() {
    let (mut session, _client) = two_windows();
    // Some 220 units of the fingers' way: more than half of the 400 a
    // workspace is, though less than half of the screen.
    carry(&mut session, -220.0, 2000);

    lift(&mut session, false);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_quick_short_flick_turns_the_page_and_a_twitch_does_not() {
    let (mut session, _client) = two_windows();
    // A tenth of the way and a little more, quickly.
    carry(&mut session, -45.0, 30);
    lift(&mut session, false);
    assert_eq!(active(&session), 1);
    session.settle();

    // Less than a tenth, however quick, is the fingers settling.
    carry(&mut session, -20.0, 10);
    lift(&mut session, false);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_flick_is_one_though_the_fingers_slow_as_they_lift() {
    let (mut session, _client) = two_windows();
    carry(&mut session, -45.0, 30);
    // The lighter touch of lifting: a little way, slowly.
    let lifting = Gesture::SwipeUpdate { dx: -1.0, dy: 0.0 };
    touch_at(&mut session, &[(lifting, 1070)]);
    lift(&mut session, false);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_swipe_taken_back_half_way_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-400.0), 1000);
    let back = [
        (
            Gesture::SwipeUpdate {
                dx: fingers(125.0),
                dy: 0.0,
            },
            2025,
        ),
        (
            Gesture::SwipeUpdate {
                dx: fingers(125.0),
                dy: 0.0,
            },
            2050,
        ),
    ];
    touch_at(&mut session, &back);
    assert_eq!(slid(&session), -150);

    lift(&mut session, false);
    assert_eq!(active(&session), 0, "the way they were going decides");
}

#[test]
fn a_cancelled_swipe_goes_back() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-500.0), 1000);

    lift(&mut session, true);
    assert_eq!(active(&session), 0);
}

#[test]
fn three_fingers_right_bring_the_one_before_in_from_the_left() {
    let (mut session, _client) = two_windows();
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    carry(&mut session, fingers(400.0), 1000);
    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    assert_eq!(slid(&session), 400 - 640);
}

#[test]
fn with_one_workspace_it_goes_a_little_way_with_the_fingers_and_back() {
    let mut session = Session::new();
    carry(&mut session, fingers(-400.0), 100);
    assert_eq!(slid(&session), -100, "a quarter of the way");

    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    session.advance(Duration::from_millis(320));
    assert_eq!(slid(&session), 0);
}

#[test]
fn a_swipe_begun_while_the_last_settles_takes_it_up_from_where_it_is() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(-400.0), 1000);
    lift(&mut session, false);
    session.advance(Duration::from_millis(60));
    let there = slid(&session);

    carry(&mut session, fingers(-50.0), 1000);
    assert_eq!(slid(&session), there - 50);
    session.advance(Duration::from_millis(320));
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
fn four_fingers_swipe_as_three_do_and_are_not_the_programs() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "map", (200, 100), RED);
    let _gestures = client.gestures();
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    client.take_events();

    // Across, the next workspace; up, the overview; down, closed again.
    touch(&mut session, &swipe(4, (-400.0, 0.0), false));
    session.eventually(|session| slid(session) == 0);
    assert_eq!(active(&session), 1, "the next workspace");
    touch(&mut session, &swipe(4, (0.0, -300.0), false));
    session.eventually(|session| session.compositor().overview_layout().is_some());
    touch(&mut session, &swipe(4, (0.0, 300.0), false));
    session.eventually(|session| session.compositor().overview_layout().is_none());
    session.roundtrip(&mut client);
    assert_eq!(told(&mut client), [], "none of it the program's");
}

/// A pinch of `fingers`, its fingers ending `scale` as far apart as they
/// began: more than one, a spread.
fn pinch(fingers: u32, scale: f64) -> [Gesture; 4] {
    let update = |scale| Gesture::PinchUpdate {
        dx: 0.0,
        dy: 0.0,
        scale,
        rotation: 0.0,
    };
    [
        Gesture::PinchBegin { fingers },
        update(f64::midpoint(1.0, scale)),
        update(scale),
        Gesture::PinchEnd { cancelled: false },
    ]
}

/// Where the first window is, and where nothing is once it is set aside.
const INSIDE: (i32, i32) = (100, 50);
/// The sliver of the first window left on the screen once it is set
/// aside, at the left edge.
const SLIVER: (i32, i32) = (5, 5);

/// A window, in a session whose windows take their time to move.
fn moving() -> (Session, Client) {
    let mut session = Session::with_config(&[MOVING]);
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    // Come in the whole way.
    session.advance(Duration::from_secs(1));
    (session, client)
}

/// Whether the windows are set aside, glided the whole way.
fn aside(session: &mut Session) -> bool {
    let picture = session.draw();
    let at = |(x, y)| picture.at(x, y);
    (at(INSIDE), at(SLIVER)) == (BACKGROUND, RED)
}

#[test]
fn four_fingers_spread_send_the_windows_aside_and_pinched_bring_them_back() {
    let (mut session, mut client) = moving();
    let _gestures = client.gestures();
    session.roundtrip(&mut client);
    client.take_events();
    let showing = |session: &Session| session.compositor().desktop().showing_desktop();

    touch(&mut session, &pinch(4, 1.6));
    assert!(showing(&session));
    assert_eq!(session.draw().at(INSIDE.0, INSIDE.1), RED, "setting out");
    session.eventually(aside);
    // Spread again, it stays; pinched in hardly or cancelled, too.
    touch(&mut session, &pinch(4, 1.6));
    touch(&mut session, &pinch(4, 0.95));
    let mut cancelled = pinch(4, 0.5);
    cancelled[3] = Gesture::PinchEnd { cancelled: true };
    touch(&mut session, &cancelled);
    assert!(showing(&session));
    touch(&mut session, &pinch(4, 0.5));
    assert!(!showing(&session));
    session.eventually(|session| session.draw().at(INSIDE.0, INSIDE.1) == RED);
    // Pinched in with nothing aside, or hardly spread, no window moves:
    // the pinch is for the launcher, which this session has none of.
    touch(&mut session, &pinch(4, 0.5));
    touch(&mut session, &pinch(4, 1.05));
    assert!(!showing(&session));
    session.roundtrip(&mut client);
    assert_eq!(told(&mut client), [], "none of it the program's");
}

/// A step of four fingers pinching, `scale` times as far apart as they
/// began.
fn spread_to(scale: f64) -> Gesture {
    Gesture::PinchUpdate {
        dx: 0.0,
        dy: 0.0,
        scale,
        rotation: 0.0,
    }
}

#[test]
fn the_windows_go_aside_with_the_fingers_and_back_and_let_go_go_on_quickly() {
    let (mut session, mut client) = moving();
    let _gestures = client.gestures();
    session.roundtrip(&mut client);
    // Half way aside, up and left from the middle: from (0, 0) to (-60,
    // -52), half of it is (-30, -26), its foot at 74.
    let half = 1.5_f64.sqrt();
    touch(
        &mut session,
        &[Gesture::PinchBegin { fingers: 4 }, spread_to(half)],
    );
    let picture = session.draw();
    assert_eq!(
        (picture.at(100, 50), picture.at(100, 80)),
        (RED, BACKGROUND)
    );
    // Together again, home, at once: the fingers lead.
    touch(&mut session, &[spread_to(1.0)]);
    assert_eq!(session.draw().at(100, 80), RED);
    // Spread past half way and let go: aside the rest of the way in the
    // time something sent takes.
    touch(&mut session, &[spread_to(1.3)]);
    session.draw();
    touch(&mut session, &[Gesture::PinchEnd { cancelled: false }]);
    assert!(session.compositor().desktop().showing_desktop());
    session.advance(Duration::from_millis(250));
    let picture = session.draw();
    assert_eq!((picture.at(60, 50), picture.at(5, 5)), (BACKGROUND, RED));
    // Short of half way, back.
    touch(
        &mut session,
        &[Gesture::PinchBegin { fingers: 4 }, spread_to(0.9)],
    );
    touch(&mut session, &[Gesture::PinchEnd { cancelled: false }]);
    assert!(session.compositor().desktop().showing_desktop());
}

#[test]
fn with_motion_reduced_the_fingers_carry_nothing_and_the_windows_go_aside_as_they_lift() {
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let mut session = Session::with_config(&[reduced]);
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    touch(
        &mut session,
        &[Gesture::PinchBegin { fingers: 4 }, spread_to(1.4)],
    );
    assert_eq!(
        session.draw().at(INSIDE.0, INSIDE.1),
        RED,
        "carried nowhere"
    );
    touch(&mut session, &[Gesture::PinchEnd { cancelled: false }]);
    assert!(aside(&mut session), "and there at once");
}

#[test]
fn a_workspace_left_by_a_key_has_its_windows_back_as_it_goes() {
    let mut session = Session::with_config(&[MOVING, ("config.yaml", FOUR_WORKSPACES)]);
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    session.compositor_mut().perform(&Action::ShowDesktop);
    session.eventually(aside);
    session.compositor_mut().perform(&Action::NextWorkspace);
    // Set out on their way back as it goes, and there once they have taken
    // the time it takes.
    session.step();
    session.advance(Duration::from_millis(250));
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.step();
    assert!(!session.compositor().desktop().showing_desktop());
}

#[test]
fn nothing_of_a_workspace_is_drawn_over_the_one_sliding_in_beside_it() {
    let (mut session, _client) = two_windows();
    // On the second, its window set aside up and to the left, hanging past
    // the screen's left edge.
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.advance(Duration::from_secs(1));
    session.compositor_mut().perform(&Action::ShowDesktop);
    session.advance(Duration::from_secs(1));
    assert_eq!(session.draw().at(5, 5), BLUE, "its sliver");
    // Carried right, the first comes in from the left, where the part of
    // the window that hangs past the edge would be.
    carry(&mut session, fingers(100.0), 100);
    let picture = session.draw();
    assert_eq!(picture.at(105, 5), BLUE, "its sliver, carried");
    assert_eq!(picture.at(70, 5), BACKGROUND, "and nothing more of it");
    // And it comes back as the fingers carry its workspace away.
    session.advance(Duration::from_millis(250));
    assert!(!session.compositor().desktop().showing_desktop());
}

#[test]
fn the_windows_set_aside_come_back_by_escape_by_a_sliver_clicked_or_by_the_action() {
    let (mut session, _client) = moving();
    let showing = |session: &Session| session.compositor().desktop().showing_desktop();
    session.compositor_mut().perform(&Action::ShowDesktop);
    session.eventually(aside);
    let key = |session: &mut Session, pressed| {
        let code = crate::common::key::ESCAPE;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    };
    key(&mut session, true);
    key(&mut session, false);
    assert!(!showing(&session), "by Escape");

    session.compositor_mut().perform(&Action::ShowDesktop);
    session.eventually(aside);
    let compositor = session.compositor_mut();
    compositor.input(Input::Motion { x: 5.0, y: 5.0 }, 0);
    for pressed in [true, false] {
        compositor.input(
            Input::Button {
                button: 0x110,
                pressed,
            },
            0,
        );
    }
    assert!(!showing(&session), "by its sliver clicked");
    session.eventually(|session| session.draw().at(INSIDE.0, INSIDE.1) == RED);

    session.compositor_mut().perform(&Action::ShowDesktop);
    session.compositor_mut().perform(&Action::ShowDesktop);
    assert!(!showing(&session), "and by the action again");
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

    // Of five fingers, as three and four are the desktop's.
    let five = swipe(5, (-300.0, 0.0), false);
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
    let all: Vec<Gesture> = [&five[..], &pinch, &hold].concat();
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
    session.advance(Duration::from_millis(160));
    let halfway = slid(&session);
    assert!(0 < halfway && halfway < 640, "{halfway}");
    session.advance(Duration::from_millis(160));
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
    session.advance(Duration::from_millis(320));
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
    session.advance(Duration::from_millis(320));
    assert_eq!(
        session.draw().at(5, 5),
        BACKGROUND,
        "not left behind to fade"
    );

    session.compositor_mut().perform(&Action::NextWorkspace);
    // The first frame comes with the key.
    session.step();
    session.advance(Duration::from_millis(320));
    assert_eq!(session.draw().at(5, 5), BLUE, "nor coming in afresh");
}

// --- The ends -------------------------------------------------------------

#[test]
fn before_the_first_workspace_there_is_nothing_and_a_swipe_gives_a_little() {
    let (mut session, _client) = two_windows();
    carry(&mut session, fingers(400.0), 100);
    assert_eq!(slid(&session), 100, "a quarter of the way");

    lift(&mut session, false);
    assert_eq!(active(&session), 0);
    session.settle();
    assert_eq!(slid(&session), 0);
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    assert_eq!((active(&session), slid(&session)), (0, 0), "nor a key");
}

#[test]
fn past_the_last_with_a_window_on_it_one_empty_workspace_is_made_and_no_more() {
    let (mut session, mut client) = two_windows();
    let last = Action::Workspace(WorkspaceNumber::clamped(4));
    session.compositor_mut().perform(&last);
    session.settle();
    session.open(&mut client, "last", (200, 100), RED);
    let count = |session: &Session| session.compositor().desktop().workspace_count();
    assert_eq!((active(&session), count(&session)), (3, 4));

    carry(&mut session, fingers(-400.0), 1000);
    assert_eq!(slid(&session), -400, "all the way with the fingers");
    lift(&mut session, false);
    assert_eq!(
        (active(&session), count(&session)),
        (4, 5),
        "made and shown"
    );
    session.settle();

    carry(&mut session, fingers(-400.0), 100);
    assert_eq!(slid(&session), -100, "nothing on it, so nothing after it");
    lift(&mut session, false);
    session.settle();
    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!((active(&session), count(&session)), (4, 5), "nor by a key");
}

#[test]
fn the_key_past_the_last_slides_in_the_one_it_makes() {
    let (mut session, mut client) = two_windows();
    let last = Action::Workspace(WorkspaceNumber::clamped(4));
    session.compositor_mut().perform(&last);
    session.settle();
    session.open(&mut client, "last", (200, 100), RED);

    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!((active(&session), slid(&session)), (4, 640));
}

/// The colour of the line between windows that share a workspace in the
/// built-in theme.
const LINE: u32 = 0xff2b_3a6b;

/// Four workspaces, and a blue window sharing the first with nothing beside
/// it, where the red one of the desktop it was made from is offered.
fn shared() -> (Session, Client) {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "offered", (200, 100), RED);
    session.open(&mut client, "tiled", (200, 100), BLUE);
    session
        .compositor_mut()
        .perform(&Action::Tile(omorfos_config::Half::Left));
    session.step();
    (session, client)
}

#[test]
fn the_line_between_windows_that_share_a_workspace_goes_with_it() {
    let (mut session, _client) = shared();
    assert_eq!(session.draw().at(320, 300), LINE);

    carry(&mut session, fingers(-100.0), 1000);
    let picture = session.draw();
    assert_eq!(picture.at(220, 300), LINE, "carried with it");
    assert_ne!(picture.at(320, 300), LINE, "and not left behind");
    lift(&mut session, true);
    session.advance(Duration::from_millis(320));

    // Coming in beside the desktop it was made from.
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.advance(Duration::from_millis(320));
    assert_ne!(session.draw().at(320, 300), LINE);
    carry(&mut session, fingers(-400.0), 1000);
    assert_eq!(session.draw().at(560, 300), LINE, "it comes in with it");
}

#[test]
fn what_is_between_windows_that_share_a_workspace_is_not_taken_hold_of_as_it_slides() {
    let (mut session, _client) = shared();
    let offers = session.compositor().desktop().offers(0);
    let offer = offers[0].whole;
    let (x, y) = (offer.origin.x + 10, offer.origin.y + 10);
    let lines = session.compositor().desktop().lines(0);

    carry(&mut session, fingers(-100.0), 1000);
    let click = |session: &mut Session, from: (i32, i32), to: (i32, i32)| {
        for (at, pressed) in [(from, true), (to, false)] {
            let (x, y) = (f64::from(at.0), f64::from(at.1));
            session.compositor_mut().input(Input::Motion { x, y }, 0);
            let button = 0x110;
            session
                .compositor_mut()
                .input(Input::Button { button, pressed }, 0);
            session.step();
        }
    };
    // The line where it is drawn, dragged; and the window offered where it
    // is drawn, chosen.
    click(&mut session, (220, 300), (150, 300));
    click(&mut session, (x - 100, y), (x - 100, y));
    lift(&mut session, true);
    session.advance(Duration::from_millis(320));

    assert_eq!(session.compositor().desktop().lines(0), lines);
    assert_eq!(session.compositor().desktop().offers(0), offers);
}

#[test]
fn with_motion_reduced_workspaces_change_without_sliding_by_keys_or_fingers() {
    let config = format!("{FOUR_WORKSPACES}appearance: {{ reduce_motion: true }}\n");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!((active(&session), slid(&session)), (1, 0), "at once");
    session.open(&mut client, "second", (200, 100), BLUE);
    assert_eq!(session.draw().at(5, 5), BLUE);

    // The fingers carry nothing, and a swipe far enough, or quick enough,
    // changes the workspace as they lift.
    carry(&mut session, fingers(400.0), 1000);
    assert_eq!((active(&session), slid(&session)), (1, 0), "not carried");
    lift(&mut session, false);
    assert_eq!(
        (active(&session), slid(&session)),
        (0, 0),
        "changed at once"
    );
    assert_eq!(session.draw().at(5, 5), RED);
    assert!(
        session.compositor().deadline().is_none(),
        "with nothing to settle"
    );
    carry(&mut session, fingers(-200.0), 1000);
    lift(&mut session, false);
    assert_eq!(active(&session), 0, "not far enough");
    carry(&mut session, fingers(-200.0), 100);
    lift(&mut session, false);
    assert_eq!(active(&session), 1, "but quick enough");
}
