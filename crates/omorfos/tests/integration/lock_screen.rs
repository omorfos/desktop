//! `omorfos-lock`, the lock screen that comes with omorfos, as a client of
//! a real compositor, with a stand-in for the helpers that ask PAM, which
//! are tested with the lock screen's own crate.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Instant;

use omorfos::Input;
use omorfos_lock::{Lock, Way};
use smithay::input::keyboard::Keycode;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::BACKGROUND;

/// The stand-in for the helpers.
fn helper() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lock-helper")
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
    let user = there.to_string_lossy().into_owned();
    Lock::new(theirs, &config, (helper(), user), ways).unwrap()
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
        std::thread::sleep(std::time::Duration::from_millis(1));
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
    let user = there.path().to_string_lossy().into_owned();
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
    let nowhere = (PathBuf::from("/nowhere/lock-helper"), "someone".to_owned());
    let mut lock = Lock::new(theirs, &config, nowhere, &[Way::Password]).unwrap();
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    // It is tried again, a moment later.
    assert!(lock.deadline().is_some());
    assert!(!settle(&mut session, &mut lock));
    assert!(session.compositor().is_locked());
}

/// A row of the first screen through the field.
fn field(session: &mut Session) -> Vec<u32> {
    let picture = session.draw();
    (0..WIDTH).map(|x| picture.at(x, HEIGHT / 2 + 10)).collect()
}

/// A locked session whose theme is `stylesheet`, with a wrong password typed
/// until it has been heard, and the field as it was at rest before.
fn wrong(stylesheet: &str) -> (Session, Lock, tempfile::TempDir, Vec<u32>) {
    let there = tempfile::TempDir::new().unwrap();
    let mut session = Session::with_config(&[("theme.css", stylesheet)]);
    let mut lock = joined(&mut session, there.path(), &[Way::Password]);
    until(&mut session, &mut lock, |session| {
        session.compositor().is_locked()
    });
    settle(&mut session, &mut lock);
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
        session.send_frames(std::time::Duration::from_millis(shown));
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
        session.send_frames(std::time::Duration::from_millis(shown * 16));
        settle(&mut session, &mut lock);
        assert_eq!(field(&mut session), rest);
    }
}
