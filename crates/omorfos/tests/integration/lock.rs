//! Locking the session: while it is locked, nothing of it can be seen or
//! reached.

use std::time::Duration;

use omorfos::{Input, Scale};
use omorfos_config::Action;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_v1::ExtSessionLockV1;

use crate::common::{Client, Event, HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const BLACK: u32 = 0xff00_0000;
const LEFT_BUTTON: u32 = 0x110;
const RIGHT_BUTTON: u32 = 0x111;

fn press(session: &mut Session, code: smithay::input::keyboard::Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

fn tap(session: &mut Session, code: smithay::input::keyboard::Keycode) {
    press(session, code, true);
    press(session, code, false);
}

fn keys(events: &[Event]) -> usize {
    let key = |event: &&Event| matches!(event, Event::Key { .. });
    events.iter().filter(key).count()
}

/// Shows a frame, which is when a locker is told the session is locked.
fn show_frame(session: &mut Session) {
    session.draw();
    session.send_frames(Duration::from_millis(16));
}

/// Locks the session with a locker that shows a green surface, and waits
/// until it has been told the session is locked.
fn lock(session: &mut Session) -> (Client, ExtSessionLockV1, WlSurface) {
    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    show_frame(session);
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::Locked));
    (locker, lock, surface)
}

/// A session with a focused application window and the pointer over it.
fn busy() -> (Session, Client) {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut app);
    app.take_events();
    (session, app)
}

#[test]
fn a_locker_is_told_the_session_is_locked_only_once_nothing_of_it_shows() {
    let (mut session, _app) = busy();
    assert_eq!(session.draw().at(50, 50), RED);
    assert!(!session.compositor().is_locked());

    let mut locker = session.connect();
    let lock = locker.lock();
    session.roundtrip(&mut locker);
    assert!(session.compositor().is_locked());
    assert_eq!(locker.take_events(), [], "not told yet");

    // A frame being shown is not enough if it was drawn before locking.
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert_eq!(locker.take_events(), [], "the window is still on screen");

    let picture = session.draw();
    assert_eq!(picture.at(50, 50), BLACK, "where the window was");
    assert_eq!(picture.at(400, 400), BLACK, "where the desktop was");
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert_eq!(locker.take_events(), [Event::Locked]);

    // It is told once.
    show_frame(&mut session);
    session.roundtrip(&mut locker);
    assert_eq!(locker.take_events(), []);
    drop(lock);
}

#[test]
fn the_lockers_surface_covers_the_screen_and_is_all_that_is_drawn() {
    let mut session = Session::with_config(&[("theme.css", ":root { --accent: nope; }\n")]);
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    assert_ne!(session.draw().at(WIDTH - 20, 20), BACKGROUND, "a notice");

    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    let size = Event::LockSize(WIDTH.unsigned_abs(), HEIGHT.unsigned_abs());
    assert!(
        locker.take_events().contains(&size),
        "told to fill the screen"
    );
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    locker.request_frame(&surface);
    session.roundtrip(&mut locker);

    let picture = session.draw();
    for (x, y) in [(50, 50), (WIDTH - 20, 20), (5, 5), (WIDTH - 1, HEIGHT - 1)] {
        assert_eq!(picture.at(x, y), GREEN, "at {x}, {y}");
    }
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    let events = locker.take_events();
    assert!(events.contains(&Event::Locked), "{events:?}");
    assert!(events.contains(&Event::FrameDone), "it can animate");

    // Unlocked, the session is as it was, less the menu.
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert!(!session.compositor().is_locked());
    let picture = session.draw();
    assert_eq!(picture.at(50, 50), RED);
    assert_ne!(picture.at(WIDTH - 20, 20), BACKGROUND, "the notice");
    assert_eq!(picture.at(300, 300), BACKGROUND, "no menu");
}

#[test]
fn the_keyboard_reaches_only_the_locker() {
    let (mut session, mut app) = busy();
    let (mut locker, lock, _surface) = lock(&mut session);
    session.roundtrip(&mut app);
    assert!(app.take_events().contains(&Event::KeyboardLeave));
    locker.take_events();

    tap(&mut session, key::A);
    session.roundtrip(&mut app);
    session.roundtrip(&mut locker);
    assert_eq!(keys(&app.take_events()), 0, "the application hears nothing");
    assert_eq!(keys(&locker.take_events()), 2, "a press and a release");

    // Key bindings are the session's, and are not acted on.
    press(&mut session, key::SUPER, true);
    tap(&mut session, key::TWO);
    press(&mut session, key::SUPER, false);
    assert_eq!(session.compositor().desktop().active_workspace(), 0);
    session.roundtrip(&mut locker);
    assert_eq!(
        keys(&locker.take_events()),
        4,
        "they are typed at the locker"
    );

    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    session.roundtrip(&mut app);
    assert!(app.take_events().contains(&Event::KeyboardEnter));
    press(&mut session, key::SUPER, true);
    tap(&mut session, key::TWO);
    press(&mut session, key::SUPER, false);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
}

#[test]
fn the_pointer_reaches_only_the_locker() {
    let (mut session, mut app) = busy();
    session.open(&mut app, "behind", (200, 100), BLUE);
    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 0);
    session.roundtrip(&mut app);
    let (mut locker, lock, _surface) = lock(&mut session);
    session.roundtrip(&mut app);
    assert!(app.take_events().contains(&Event::PointerLeave));
    locker.take_events();

    let input = |session: &mut Session, input| session.compositor_mut().input(input, 0);
    input(&mut session, Input::Motion { x: 60.0, y: 40.0 });
    input(
        &mut session,
        Input::Button {
            button: LEFT_BUTTON,
            pressed: true,
        },
    );
    input(
        &mut session,
        Input::Button {
            button: LEFT_BUTTON,
            pressed: false,
        },
    );
    input(
        &mut session,
        Input::Scroll {
            horizontal: 0.0,
            vertical: 15.0,
        },
    );
    // Nowhere near a window: this would open the root menu.
    input(
        &mut session,
        Input::MotionBy {
            dx: 400.0,
            dy: 300.0,
        },
    );
    input(
        &mut session,
        Input::Button {
            button: RIGHT_BUTTON,
            pressed: true,
        },
    );
    session.roundtrip(&mut app);
    session.roundtrip(&mut locker);

    assert_eq!(app.take_events(), [], "the application hears nothing");
    let events = locker.take_events();
    assert!(
        events.contains(&Event::PointerEnter { x: 60.0, y: 40.0 }),
        "{events:?}"
    );
    assert!(events.contains(&Event::PointerMotion { x: 460.0, y: 340.0 }));
    let button = |pressed| Event::PointerButton {
        button: LEFT_BUTTON,
        pressed,
    };
    assert!(events.contains(&button(true)) && events.contains(&button(false)));
    let scroll = Event::PointerScroll {
        vertical: true,
        value: 15.0,
    };
    assert!(events.contains(&scroll), "{events:?}");
    assert_eq!(
        session.draw().at(500, 400),
        GREEN,
        "no menu over the locker"
    );

    input(
        &mut session,
        Input::Button {
            button: RIGHT_BUTTON,
            pressed: false,
        },
    );
    input(&mut session, Input::Motion { x: 60.0, y: 60.0 });
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    session.roundtrip(&mut app);
    assert_eq!(session.draw().at(500, 400), BACKGROUND, "nor once unlocked");
    assert!(
        app.take_events()
            .iter()
            .any(|event| matches!(event, Event::PointerEnter { .. })),
        "the window under the pointer has it again"
    );
}

#[test]
fn switching_virtual_terminal_still_works() {
    let (mut session, _app) = busy();
    let (mut locker, _lock, _surface) = lock(&mut session);
    locker.take_events();

    press(&mut session, key::CONTROL, true);
    press(&mut session, key::ALT, true);
    tap(&mut session, key::F2);
    press(&mut session, key::ALT, false);
    press(&mut session, key::CONTROL, false);
    session.roundtrip(&mut locker);

    assert_eq!(session.compositor_mut().take_terminal_switch(), Some(2));
    assert_eq!(session.compositor_mut().take_terminal_switch(), None);
    let f2 = key::F2.raw() - 8;
    let heard = locker.take_events();
    assert!(
        !heard
            .iter()
            .any(|event| matches!(event, Event::Key { code, .. } if *code == f2)),
        "kept from the locker: {heard:?}"
    );
}

#[test]
fn a_second_locker_is_refused_and_cannot_show_anything() {
    let (mut session, _app) = busy();
    let (mut locker, lock, _surface) = lock(&mut session);

    let mut intruder = session.connect();
    let refused = intruder.lock();
    session.roundtrip(&mut intruder);
    assert_eq!(intruder.take_events(), [Event::LockRefused]);

    // It still has an object to ask for a surface with.
    intruder.lock_surface(&refused);
    session.roundtrip(&mut intruder);
    assert_eq!(
        intruder.take_events(),
        [],
        "never told a size, so cannot draw"
    );
    assert_eq!(session.draw().at(50, 50), GREEN, "the locker's own surface");

    // Nor once the session is unlocked.
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert!(!session.compositor().is_locked());
    intruder.lock_surface(&refused);
    session.roundtrip(&mut intruder);
    assert_eq!(intruder.take_events(), []);
    assert_eq!(session.draw().at(50, 50), RED);
}

#[test]
fn a_session_stays_locked_when_its_locker_dies_until_another_takes_over() {
    let (mut session, mut app) = busy();
    let (locker, lock, surface) = lock(&mut session);
    session.roundtrip(&mut app);
    app.take_events();
    drop((lock, surface, locker));
    session.eventually(|session| session.draw().at(50, 50) == BLACK);
    assert!(session.compositor().is_locked());

    tap(&mut session, key::A);
    session
        .compositor_mut()
        .input(Input::Motion { x: 60.0, y: 60.0 }, 0);
    session.roundtrip(&mut app);
    assert_eq!(app.take_events(), [], "still out of reach");

    // The dead locker's hold is gone, so a new one is accepted.
    let (mut locker, lock, _surface) = lock_again(&mut session);
    assert_eq!(session.draw().at(50, 50), GREEN);
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert!(!session.compositor().is_locked());
    assert_eq!(session.draw().at(50, 50), RED);
}

/// As [`lock`], under a name that reads better the second time.
fn lock_again(session: &mut Session) -> (Client, ExtSessionLockV1, WlSurface) {
    lock(session)
}

#[test]
fn the_lockers_surface_follows_the_size_and_scale_of_the_screen() {
    let (mut session, _app) = busy();
    // Before the locker has a surface there is nothing to tell.
    let mut locker = session.connect();
    let lock = locker.lock();
    session.roundtrip(&mut locker);
    session.resize_output((800, 600));
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::LockSize(800, 600)));

    session.resize_output((WIDTH, HEIGHT));
    session.suggest_scale(Scale::new(2.0));
    session.roundtrip(&mut locker);
    session.roundtrip(&mut locker);
    let half = Event::LockSize(WIDTH.unsigned_abs() / 2, HEIGHT.unsigned_abs() / 2);
    assert_eq!(locker.take_events().last(), Some(&half));
    assert_eq!(locker.state.told.buffer_scale, Some(2), "and at what scale");

    locker.fill(&surface, (WIDTH / 2, HEIGHT / 2), GREEN);
    session.roundtrip(&mut locker);
    assert_eq!(session.draw().at(WIDTH - 10, HEIGHT - 10), GREEN);
}

#[test]
fn a_drag_does_not_survive_being_locked() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open_decorated(&mut app, "app", (200, 100), RED);
    let origin = |session: &Session| {
        let frame = session.compositor().desktop().frames().next().unwrap();
        frame.geometry.origin
    };
    let before = origin(&session);

    // Take hold of the titlebar, then lock with the button still down.
    session
        .compositor_mut()
        .input(Input::Motion { x: 100.0, y: 14.0 }, 0);
    let hold = Input::Button {
        button: LEFT_BUTTON,
        pressed: true,
    };
    session.compositor_mut().input(hold, 0);
    let (mut locker, lock, _surface) = lock(&mut session);
    session
        .compositor_mut()
        .input(Input::Motion { x: 300.0, y: 200.0 }, 0);
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    session
        .compositor_mut()
        .input(Input::Motion { x: 350.0, y: 250.0 }, 0);

    assert_eq!(origin(&session), before, "the frame was not carried along");
}

#[test]
fn the_pointer_is_drawn_over_the_locker_as_the_locker_asks() {
    use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

    const YELLOW: u32 = 0xffff_ff00;
    let (mut session, _app) = busy();
    let themes = session.install_cursors("default", &[("default", 8, BLUE), ("text", 8, YELLOW)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    let (mut locker, _lock, _surface) = lock(&mut session);

    session
        .compositor_mut()
        .input(Input::Motion { x: 300.0, y: 200.0 }, 0);
    session.roundtrip(&mut locker);
    assert_eq!(session.draw().at(303, 203), BLUE, "the ordinary pointer");
    assert_eq!(
        session.draw().at(320, 203),
        GREEN,
        "on the locker's surface"
    );

    locker.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut locker);
    session.step();
    assert_eq!(session.draw().at(303, 203), YELLOW);
}
