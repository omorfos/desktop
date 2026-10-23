//! `omorfos-lock`, the lock screen that comes with omorfos, as a client of
//! a real compositor, with a stand-in for the helpers that ask PAM, which
//! are tested with the lock screen's own crate.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos::Input;
use omorfos_lock::{Lock, User, Way};
use smithay::input::keyboard::Keycode;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, RED};

/// The stand-in for the helpers.
fn helper() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lock-helper")
}

/// Where on the first screen, once someone is to unlock, the lock screen
/// shows their face and what they are called, and the middle of the field
/// and of the line under the password's: rows, from the top.
const FACE_ROW: i32 = 270;
const NAME_ROW: i32 = 340;
const FIELD_ROW: i32 = 384;
const SECOND_LINE_ROW: i32 = 452;

/// Who the lock screen in a test is for: named `there`, called "Some One",
/// with a face if there is a picture `face.png` in `there`.
fn someone(there: &Path) -> User {
    User {
        name: there.to_string_lossy().into_owned(),
        called: "Some One".to_owned(),
        faces: vec![there.join("nowhere.png"), there.join("face.png")],
    }
}

/// A lock screen in the session, with the role to lock, asking `ways`; the
/// helpers are steered by what is put in `there`.
fn joined(session: &mut Session, there: &Path, ways: &[Way]) -> Lock {
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::LOCK)
        .unwrap();
    let config = session.config.path().to_owned();
    Lock::new(theirs, &config, (helper(), someone(there)), ways, None).unwrap()
}

/// A row of the first screen.
fn row(session: &mut Session, y: i32) -> Vec<u32> {
    let picture = session.draw();
    (0..WIDTH).map(|x| picture.at(x, y)).collect()
}

/// Whether a row of the first screen has anything on it.
fn inked(session: &mut Session, y: i32) -> bool {
    row(session, y).iter().any(|pixel| *pixel != BACKGROUND)
}

/// A locked session, its lock screen showing who is to unlock: woken by a
/// key, which types nothing.
fn awake(session: &mut Session, there: &Path, ways: &[Way]) -> Lock {
    let mut lock = joined(session, there, ways);
    until(session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    settle(session, &mut lock);
    tap(session, key::SHIFT);
    settle(session, &mut lock);
    lock
}

/// Lets the lock screen and the compositor talk until `done` holds, and
/// returns whether the lock screen said it is done.
fn until(session: &mut Session, lock: &mut Lock, mut done: impl FnMut(&Session) -> bool) -> bool {
    for _ in 0..5000 {
        let unlocked = lock.turn(Instant::now()).unwrap();
        session.step();
        session.draw();
        if unlocked || done(session) {
            return unlocked;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    unreachable!("the lock screen never got there");
}

/// Lets the lock screen and the compositor talk for a while, so that what
/// is due has been done, and returns whether the session was unlocked.
fn settle(session: &mut Session, lock: &mut Lock) -> bool {
    let mut turns = 0;
    until(session, lock, |_| {
        turns += 1;
        turns > 50
    })
}

fn tap(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
}

fn typed(session: &mut Session, keys: &[Keycode]) {
    for code in keys {
        tap(session, *code);
    }
}

/// Types the password, afresh, until the session is unlocked: until the
/// helper has started and asks, Enter goes nowhere.
fn unlock(session: &mut Session, lock: &mut Lock) {
    for _ in 0..500 {
        typed(session, &[key::ESCAPE, key::A, key::T, key::X, key::RETURN]);
        if settle(session, lock) {
            return;
        }
    }
    unreachable!("the session was never unlocked");
}

/// How much of the first screen is not the desktop's colour.
fn drawn(session: &mut Session) -> usize {
    let picture = session.draw();
    let places = (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x, y)));
    places
        .filter(|(x, y)| picture.at(*x, *y) != BACKGROUND)
        .count()
}

#[test]
fn the_password_typed_on_the_lock_screen_unlocks_the_session() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let mut lock = joined(&mut session, there.path(), &[Way::Password]);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    // The time and the field are drawn.
    until(&mut session, &mut lock, |_| true);
    assert!(drawn(&mut session) > 500);
    // Wrong first, typed until the helper has heard it, and asked again;
    // Escape empties the field too, and keys held with Ctrl type nothing.
    // The screen changes scale meanwhile, and is drawn again for it. What
    // PAM says of the wrong try is said instead of the lock screen's own.
    session.suggest_scale(omorfos::Scale::new(2.0));
    std::fs::write(there.path().join("locked"), "").unwrap();
    let wrong = there.path().join("wrong");
    for _ in 0..500 {
        typed(&mut session, &[key::ESCAPE, key::A, key::RETURN]);
        settle(&mut session, &mut lock);
        if wrong.exists() {
            break;
        }
    }
    assert!(wrong.exists(), "a wrong try was heard");
    until(&mut session, &mut lock, |_| true);
    assert!(session.compositor().is_locked());
    typed(
        &mut session,
        &[key::X, key::BACKSPACE, key::A, key::ESCAPE, key::TAB],
    );
    session.compositor_mut().input(
        Input::Key {
            code: key::CONTROL,
            pressed: true,
        },
        0,
    );
    tap(&mut session, key::Q);
    session.compositor_mut().input(
        Input::Key {
            code: key::CONTROL,
            pressed: false,
        },
        0,
    );
    // Enter with nothing asked yet goes nowhere.
    tap(&mut session, key::RETURN);
    let unlocked = settle(&mut session, &mut lock);
    assert!(!unlocked);
    unlock(&mut session, &mut lock);
    session.step();
    assert!(!session.compositor().is_locked());
}

#[test]
fn a_finger_unlocks_without_a_word_typed() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let ways = [Way::Password, Way::Fingerprint, Way::Key];
    let mut lock = joined(&mut session, there.path(), &ways);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    std::fs::write(there.path().join("finger"), "").unwrap();
    assert!(until(&mut session, &mut lock, |_| false));
}

#[test]
fn a_key_that_asks_for_its_pin_is_not_asked_here() {
    let there = tempfile::TempDir::new().unwrap();
    std::fs::write(there.path().join("pin"), "").unwrap();
    let mut session = Session::new();
    let mut lock = joined(&mut session, there.path(), &[Way::Password, Way::Key]);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    // What is typed goes to the password, which the key cannot have.
    settle(&mut session, &mut lock);
    unlock(&mut session, &mut lock);
}

#[test]
fn a_session_another_locker_holds_is_not_locked_again() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let (_locker, _held, _surface) = crate::lock::lock(&mut session);
    let mut lock = joined(&mut session, there.path(), &[Way::Password]);
    let refused = (0..200).find_map(|_| {
        session.step();
        lock.turn(Instant::now()).err()
    });
    let refused = refused.unwrap().to_string();
    assert!(refused.contains("another program"), "{refused}");
}

#[test]
fn a_screen_unplugged_while_locked_takes_its_lock_surface_with_it() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    session.suggest_scale(omorfos::Scale::new(2.0));
    let mut lock = joined(&mut session, there.path(), &[Way::Password]);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    settle(&mut session, &mut lock);
    session.compositor_mut().remove_output(&second);
    settle(&mut session, &mut lock);
    assert!(session.compositor().is_locked());
    unlock(&mut session, &mut lock);
}

#[test]
fn a_locker_that_ends_with_the_session_locked_is_started_again_a_few_times() {
    let mut session = Session::with_config(&[("config.yaml", "locker: [\"true\"]\n")]);
    // Locked by another, as by a locker that then crashed.
    let (_locker, _held, _surface) = crate::lock::lock(&mut session);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Lock);
    let said = "The locker keeps stopping";
    session.eventually(|session| {
        session
            .compositor()
            .problems()
            .iter()
            .any(|p| p.contains(said))
    });
    assert!(
        session.compositor().is_locked(),
        "and the session stays locked"
    );
}

#[test]
fn the_lock_screen_runs_by_itself_in_the_session_it_is_started_in() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-lock")).unwrap();
    session.inner().serve(socket, omorfos::Roles::LOCK).unwrap();
    // Only the password, as the configuration it reads has it.
    std::fs::create_dir_all(runtime.join("omorfos")).unwrap();
    let config = "lock: { fingerprint: false, key: false }\n";
    std::fs::write(runtime.join("omorfos/config.yaml"), config).unwrap();
    let there = tempfile::TempDir::new().unwrap();
    let user = omorfos_lock::who("", 0, Some(there.path().as_os_str()), None);
    let running = std::thread::spawn(move || {
        let runtime = Some(runtime.as_os_str());
        let display = Some(std::ffi::OsStr::new("wayland-lock"));
        omorfos_lock::start((runtime, display), (runtime, None), (helper(), user))
            .map_err(|failure| failure.to_string())
    });
    // Said to be locked once the lock is drawn on every screen, and asked
    // once the helper has started, which takes a moment: until then Enter
    // goes nowhere, so the password is typed again, afresh, until it does.
    session.eventually(|session| {
        session.draw();
        session.compositor().is_locked()
    });
    session.eventually(|session| {
        session.draw();
        typed(session, &[key::ESCAPE, key::A, key::T, key::X, key::RETURN]);
        !session.compositor().is_locked()
    });
    session.eventually(|_| running.is_finished());
    assert_eq!(running.join().unwrap(), Ok(()));
}

#[test]
fn a_helper_that_cannot_be_started_leaves_the_session_locked() {
    let mut session = Session::new();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::LOCK)
        .unwrap();
    let config = session.config.path().to_owned();
    let nowhere = (
        PathBuf::from("/nowhere/lock-helper"),
        someone(Path::new("/nowhere")),
    );
    // Nor can `omorfos-session`, which offers nothing then.
    let session_nowhere = Some((PathBuf::from("/nowhere/omorfos-session"), Vec::new()));
    let mut lock = Lock::new(theirs, &config, nowhere, &[Way::Password], session_nowhere).unwrap();
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    // Woken, it asks `omorfos-session` what is offered, which cannot be
    // started either.
    settle(&mut session, &mut lock);
    tap(&mut session, key::SHIFT);
    settle(&mut session, &mut lock);
    assert!(inked(&mut session, NAME_ROW));
    // It is tried again, a moment later.
    assert!(lock.deadline().is_some());
    assert!(!settle(&mut session, &mut lock));
    assert!(session.compositor().is_locked());
}

/// A row of the first screen through the field.
fn field(session: &mut Session) -> Vec<u32> {
    row(session, FIELD_ROW)
}

/// A locked session whose theme is `stylesheet`, with a wrong password typed
/// until it has been heard, and the field as it was at rest before.
fn wrong(stylesheet: &str) -> (Session, Lock, tempfile::TempDir, Vec<u32>) {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::with_config(&[("theme.css", stylesheet)]);
    let mut lock = awake(&mut session, there.path(), &[Way::Password]);
    // As long as it takes to come into view.
    for shown in 1..30 {
        session.send_frames(Duration::from_millis(shown * 16));
        settle(&mut session, &mut lock);
    }
    let rest = field(&mut session);
    let wrong = there.path().join("wrong");
    for _ in 0..500 {
        typed(&mut session, &[key::ESCAPE, key::A, key::RETURN]);
        settle(&mut session, &mut lock);
        if wrong.exists() {
            break;
        }
    }
    assert!(wrong.exists(), "a wrong try was heard");
    (session, lock, there, rest)
}

#[test]
fn the_field_shakes_when_the_password_was_not_right_and_comes_to_rest() {
    // The tests' sessions keep still unless a part says otherwise.
    let (mut session, mut lock, _there, rest) = wrong("question { transition: 120ms; }\n");
    // Frames are shown a sixtieth of a second apart, by the time the
    // compositor tells them at.
    let mut shown = 0;
    let mut moved = false;
    for _ in 0..200 {
        shown += 16;
        session.send_frames(Duration::from_millis(shown));
        settle(&mut session, &mut lock);
        moved |= field(&mut session) != rest;
        if moved && field(&mut session) == rest {
            return;
        }
    }
    unreachable!("the field never shook and came to rest, moved: {moved}");
}

#[test]
fn a_theme_that_moves_nothing_has_the_field_keep_still() {
    let (mut session, mut lock, _there, rest) = wrong("question { transition: none; }\n");
    for shown in 1..30 {
        session.send_frames(Duration::from_millis(shown * 16));
        settle(&mut session, &mut lock);
        assert_eq!(field(&mut session), rest);
    }
}

#[test]
fn only_the_time_is_shown_until_a_key_or_a_button_is_pressed_and_again_after_a_while() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let mut lock = joined(&mut session, there.path(), &[Way::Password]);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    settle(&mut session, &mut lock);
    assert!(drawn(&mut session) > 0, "the time");
    for y in [FACE_ROW, NAME_ROW, FIELD_ROW] {
        assert!(!inked(&mut session, y), "nothing else, at {y}");
    }
    // The key that shows the rest types nothing.
    tap(&mut session, key::A);
    settle(&mut session, &mut lock);
    assert!(inked(&mut session, NAME_ROW), "who is to unlock");
    let empty = field(&mut session);
    assert_ne!(empty, row(&mut session, 0), "the field");
    tap(&mut session, key::ESCAPE);
    settle(&mut session, &mut lock);
    assert_eq!(field(&mut session), empty, "nothing was typed");
    tap(&mut session, key::A);
    settle(&mut session, &mut lock);
    assert_ne!(field(&mut session), empty, "what is typed now is");
    // Left alone for a while, it shows only the time again, and what was
    // typed has gone.
    let later = Instant::now() + Duration::from_secs(31);
    assert!(lock.deadline().is_some_and(|due| due < later));
    lock.turn(later).unwrap();
    settle(&mut session, &mut lock);
    assert!(!inked(&mut session, FIELD_ROW));
    // A button shows it again, empty.
    let input = |session: &mut Session, input| session.compositor_mut().input(input, 0);
    input(&mut session, Input::Motion { x: 20.0, y: 20.0 });
    for pressed in [true, false] {
        input(
            &mut session,
            Input::Button {
                button: 0x110,
                pressed,
            },
        );
    }
    settle(&mut session, &mut lock);
    assert_eq!(field(&mut session), empty);
}

#[test]
fn who_is_to_unlock_is_shown_by_their_face_and_what_they_are_called() {
    let there = tempfile::TempDir::new().unwrap();
    crate::wallpaper::png(&there.path().join("face.png"), (8, 8), |_, _| RED);
    let mut session = Session::new();
    let _lock = awake(&mut session, there.path(), &[Way::Password]);
    let face = row(&mut session, FACE_ROW);
    let middle = usize::try_from(WIDTH / 2).unwrap();
    assert_eq!(face[middle], RED, "the face, in the middle");
    assert_eq!(face[middle - 60], BACKGROUND, "as large as a face is");
    assert!(inked(&mut session, NAME_ROW), "and what they are called");
}

#[test]
fn who_has_no_face_of_their_own_is_shown_a_circle_with_their_initials() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let _lock = awake(&mut session, there.path(), &[Way::Password]);
    let middle = usize::try_from(WIDTH / 2).unwrap();
    let edge = row(&mut session, FACE_ROW - 40);
    assert_ne!(edge[middle], BACKGROUND, "a circle");
    assert_eq!(edge[middle - 40], BACKGROUND, "and not a square");
    // The initials are drawn in it, unlike the circle's own colour.
    let across = row(&mut session, FACE_ROW);
    let circle = across[middle - 40];
    assert_ne!(circle, BACKGROUND);
    assert!(
        across[middle - 30..middle + 30]
            .iter()
            .any(|pixel| *pixel != circle)
    );
}

#[test]
fn who_is_to_unlock_comes_into_view_as_the_theme_has_things_move() {
    let there = tempfile::TempDir::new().unwrap();
    crate::wallpaper::png(&there.path().join("face.png"), (8, 8), |_, _| RED);
    let mut session = Session::with_config(&[("theme.css", "question { transition: 120ms; }\n")]);
    let mut lock = awake(&mut session, there.path(), &[Way::Password]);
    assert!(!inked(&mut session, NAME_ROW), "not yet");
    let middle = usize::try_from(WIDTH / 2).unwrap();
    let mut seen = Vec::new();
    for shown in 1..30 {
        session.send_frames(Duration::from_millis(shown * 16));
        settle(&mut session, &mut lock);
        seen.push((field(&mut session), row(&mut session, FACE_ROW)[middle]));
    }
    let (all, face) = seen.last().unwrap().clone();
    assert_ne!(all, row(&mut session, 0), "in view at last");
    assert_eq!(face, RED, "the face too");
    // On its way, the field and the face are partly there.
    let on_its_way = seen.iter().any(|(field, face)| {
        *field != all
            && field.iter().any(|pixel| *pixel != BACKGROUND)
            && ![RED, BACKGROUND].contains(face)
    });
    assert!(on_its_way, "on its way");
}

#[test]
fn a_security_key_is_shown_only_once_one_is_plugged_in() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let mut lock = awake(&mut session, there.path(), &[Way::Password, Way::Key]);
    // Tried, and failing, for a while.
    for _ in 0..3 {
        settle(&mut session, &mut lock);
        assert!(!inked(&mut session, SECOND_LINE_ROW));
    }
    std::fs::write(there.path().join("key"), "").unwrap();
    let shown = (0..5000).any(|_| {
        lock.turn(Instant::now()).unwrap();
        session.step();
        std::thread::sleep(Duration::from_millis(1));
        inked(&mut session, SECOND_LINE_ROW)
    });
    assert!(shown, "the key says to touch it");
    std::fs::write(there.path().join("touched"), "").unwrap();
    assert!(until(&mut session, &mut lock, |_| false));
}

#[test]
fn a_reader_that_is_not_there_is_not_offered() {
    let there = tempfile::TempDir::new().unwrap();
    std::fs::write(there.path().join("no-reader"), "").unwrap();
    let mut session = Session::new();
    let ways = [Way::Password, Way::Fingerprint];
    let mut lock = awake(&mut session, there.path(), &ways);
    settle(&mut session, &mut lock);
    assert!(!inked(&mut session, SECOND_LINE_ROW));
    unlock(&mut session, &mut lock);
}

/// Where on the first screen the power buttons are, just inside their top
/// edge, above what they say, and the line just above them: rows, from the
/// top.
const BUTTONS_ROW: i32 = 428;
const ABOVE_BUTTONS_ROW: i32 = 402;

/// The stand-in for `omorfos-session`.
fn power_helper() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/power-helper")
}

/// A locked session whose lock screen offers what `offered` says, as
/// `omorfos-session offered` would, steered by what is put in `there`.
fn powered(session: &mut Session, there: &Path, offered: &str) -> Lock {
    std::fs::write(there.join("offered"), offered).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::LOCK)
        .unwrap();
    let config = session.config.path().to_owned();
    let power = Some((power_helper(), vec![there.as_os_str().to_owned()]));
    let user = (helper(), someone(there));
    let mut lock = Lock::new(theirs, &config, user, &[Way::Password], power).unwrap();
    until(session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    settle(session, &mut lock);
    lock
}

/// The middle of each power button on the first screen, from the left.
fn buttons(session: &mut Session) -> Vec<f64> {
    let row = row(session, BUTTONS_ROW);
    let mut middles = Vec::new();
    let mut start = None;
    for (x, pixel) in row.iter().chain([&BACKGROUND]).enumerate() {
        match (start, *pixel == BACKGROUND) {
            (None, false) => start = Some(x),
            (Some(from), true) => {
                middles.push(f64::from(u32::try_from(from + x).unwrap()) / 2.0);
                start = None;
            }
            _ => {}
        }
    }
    middles
}

/// Lets the lock screen and the compositor talk until `count` buttons are
/// shown, and returns where. Every screen is drawn meanwhile, as the
/// session is said to be locked only once each has been.
fn shown(session: &mut Session, lock: &mut Lock, count: usize) -> Vec<f64> {
    for _ in 0..5000 {
        lock.turn(Instant::now()).unwrap();
        session.step();
        session.draw_all();
        let middles = buttons(session);
        if middles.len() == count {
            return middles;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    unreachable!("never {count} buttons, but {:?}", buttons(session));
}

/// Moves the pointer to `x` across on the row of buttons.
fn point(session: &mut Session, x: f64) {
    let y = f64::from(BUTTONS_ROW);
    session.compositor_mut().input(Input::Motion { x, y }, 0);
}

/// Presses the left button where the pointer is, and lets it go.
fn click(session: &mut Session) {
    for pressed in [true, false] {
        session.compositor_mut().input(
            Input::Button {
                button: 0x110,
                pressed,
            },
            0,
        );
    }
}

/// What the stand-in for `omorfos-session` was asked to do, once it has
/// been asked `count` things.
fn asked(session: &mut Session, lock: &mut Lock, there: &Path, count: usize) -> Vec<String> {
    let file = there.join("asked");
    let read = |file: &Path| std::fs::read_to_string(file).unwrap_or_default();
    for _ in 0..5000 {
        lock.turn(Instant::now()).unwrap();
        session.step();
        if read(&file).lines().count() >= count {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    read(&file).lines().map(str::to_owned).collect()
}

#[test]
fn the_power_buttons_offered_are_shown_once_awake_and_sleep_is_done_at_once() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let offered = "can sleep\ncan restart\ncan shut-down\ncan log-out\n";
    let mut lock = powered(&mut session, there.path(), offered);
    // Only the time, until woken; and a click that wakes it presses
    // nothing.
    assert_eq!(buttons(&mut session), Vec::<f64>::new());
    point(&mut session, f64::from(WIDTH / 2));
    click(&mut session);
    let middles = shown(&mut session, &mut lock, 4);
    assert!(!there.path().join("asked").exists());
    point(&mut session, middles[0]);
    click(&mut session);
    assert_eq!(
        asked(&mut session, &mut lock, there.path(), 1),
        ["sleep --locked"]
    );
    // Done, and heard to have been.
    settle(&mut session, &mut lock);
    assert!(session.compositor().is_locked(), "and it stays locked");
    assert!(
        !inked(&mut session, ABOVE_BUTTONS_ROW),
        "nothing went wrong"
    );
}

#[test]
fn what_loses_what_is_not_saved_is_asked_about_first_and_may_be_said_no_to() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let mut lock = powered(&mut session, there.path(), "can shut-down\ncan log-out\n");
    tap(&mut session, key::SHIFT);
    let middles = shown(&mut session, &mut lock, 2);
    assert!(!inked(&mut session, ABOVE_BUTTONS_ROW));
    // Asked about: yes, or no.
    point(&mut session, middles[1]);
    click(&mut session);
    settle(&mut session, &mut lock);
    assert!(inked(&mut session, ABOVE_BUTTONS_ROW), "what it loses");
    let asking = shown(&mut session, &mut lock, 2);
    point(&mut session, asking[1]);
    click(&mut session);
    settle(&mut session, &mut lock);
    assert!(!inked(&mut session, ABOVE_BUTTONS_ROW), "no");
    // Escape says no too, and a while left alone forgets it was asked.
    point(&mut session, middles[1]);
    click(&mut session);
    settle(&mut session, &mut lock);
    tap(&mut session, key::ESCAPE);
    settle(&mut session, &mut lock);
    assert!(!inked(&mut session, ABOVE_BUTTONS_ROW));
    point(&mut session, middles[0]);
    click(&mut session);
    settle(&mut session, &mut lock);
    lock.turn(Instant::now() + Duration::from_secs(31)).unwrap();
    tap(&mut session, key::SHIFT);
    settle(&mut session, &mut lock);
    assert!(!inked(&mut session, ABOVE_BUTTONS_ROW));
    assert!(!there.path().join("asked").exists(), "nothing was done");
    // Yes.
    let middles = shown(&mut session, &mut lock, 2);
    point(&mut session, middles[1]);
    click(&mut session);
    settle(&mut session, &mut lock);
    let asking = shown(&mut session, &mut lock, 2);
    point(&mut session, asking[0]);
    click(&mut session);
    assert_eq!(asked(&mut session, &mut lock, there.path(), 1), ["log-out"]);
}

#[test]
fn what_could_not_be_done_says_why_and_a_press_meanwhile_does_nothing() {
    let there = tempfile::TempDir::new().unwrap();
    std::fs::write(there.path().join("refuse"), "").unwrap();
    std::fs::write(there.path().join("hold"), "").unwrap();
    let mut session = Session::new();
    let mut lock = powered(&mut session, there.path(), "can sleep\n");
    tap(&mut session, key::SHIFT);
    let middles = shown(&mut session, &mut lock, 1);
    point(&mut session, middles[0]);
    click(&mut session);
    assert_eq!(asked(&mut session, &mut lock, there.path(), 1).len(), 1);
    // Still being done: pressed again, it is not asked again.
    click(&mut session);
    settle(&mut session, &mut lock);
    std::fs::remove_file(there.path().join("hold")).unwrap();
    let said = (0..5000).any(|_| {
        lock.turn(Instant::now()).unwrap();
        session.step();
        std::thread::sleep(Duration::from_millis(1));
        inked(&mut session, ABOVE_BUTTONS_ROW)
    });
    assert!(said, "why it did not happen");
    assert_eq!(
        asked(&mut session, &mut lock, there.path(), 2),
        ["sleep --locked"]
    );
}

#[test]
fn who_keeps_the_machine_from_turning_off_is_said() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let offered = "can sleep\nheld shut-down Firefox: Downloading\n";
    let mut lock = powered(&mut session, there.path(), offered);
    tap(&mut session, key::SHIFT);
    shown(&mut session, &mut lock, 1);
    settle(&mut session, &mut lock);
    assert!(inked(&mut session, ABOVE_BUTTONS_ROW));
}

#[test]
fn a_button_under_the_pointer_looks_so_on_each_screen() {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let _second = session.add_output((800, 600));
    let mut lock = powered(&mut session, there.path(), "can sleep\ncan restart\n");
    tap(&mut session, key::SHIFT);
    let middles = shown(&mut session, &mut lock, 2);
    point(&mut session, 4.0);
    settle(&mut session, &mut lock);
    let away = row(&mut session, BUTTONS_ROW);
    point(&mut session, middles[0]);
    settle(&mut session, &mut lock);
    assert_ne!(row(&mut session, BUTTONS_ROW), away, "under the pointer");
    point(&mut session, middles[0] + 4.0);
    settle(&mut session, &mut lock);
    // Onto the next screen, and off this one.
    let x = f64::from(WIDTH) + 10.0;
    session
        .compositor_mut()
        .input(Input::Motion { x, y: 10.0 }, 0);
    settle(&mut session, &mut lock);
    assert_eq!(
        row(&mut session, BUTTONS_ROW),
        away,
        "and not once it has gone"
    );
}
