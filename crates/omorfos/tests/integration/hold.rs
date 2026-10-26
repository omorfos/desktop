//! The compositor under `omorfos-hold`, which holds the session while the
//! compositor is started again (ADR 60): it serves the socket it is
//! handed, says what the holder is to keep, and comes up as the last one
//! left the session.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos_config::Action;
use omorfos_ipc::hold::{self, Heard, Told, Word};

use crate::common::{HEIGHT, Session, WIDTH};

/// The holder's end of the connection to a compositor it started, with
/// what has been heard on it so far.
pub(crate) struct Holder {
    connection: UnixStream,
    heard: Heard,
}

impl Holder {
    /// The words the compositor has said since last asked.
    pub(crate) fn said(&mut self) -> Vec<Word> {
        let mut bytes = [0; 256];
        let mut words = Vec::new();
        while let Ok(read) = self.connection.read(&mut bytes) {
            if read == 0 {
                break;
            }
            words.extend(self.heard.hear(&bytes[..read]));
        }
        words
    }
}

/// Puts `session` under a holder that tells it `told` and hands it a socket
/// in the session's own directory, as `omorfos-hold` hands one in the
/// runtime directory; returns the holder and where the socket is.
pub(crate) fn hold(session: &mut Session, told: Told) -> (Holder, PathBuf) {
    let path = session.config.path().join("wayland-held");
    let socket = UnixListener::bind(&path).unwrap();
    let (holder, compositor) = UnixStream::pair().unwrap();
    hold::give(&holder, &told.written(), &[socket.as_fd()]).unwrap();
    session.inner().hold(compositor).unwrap();
    holder.set_nonblocking(true).unwrap();
    let holder = Holder {
        connection: holder,
        heard: Heard::default(),
    };
    (holder, path)
}

/// What the compositor tells the programs it starts `name` is.
fn told_programs(session: &Session, name: &str) -> Option<String> {
    let environment = session.compositor().session_environment();
    let mut environment = environment.into_iter();
    let found = environment.find(|(told, _)| *told == name);
    found.map(|(_, value)| value.into_string().unwrap())
}

/// Puts at `path` a program that leaves its `mark` beside the session's
/// IPC socket each time it starts, and keeps running until it is ended.
fn stand_in(path: &Path, mark: &str) {
    let script = format!(
        "#!/bin/sh\nprintf x >> \"$(dirname \"$OMORFOS_SOCKET\")/{mark}\"\nexec sleep 30\n"
    );
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_compositor_under_a_holder_serves_the_socket_it_is_handed_and_says_it_is_up() {
    let mut session = Session::new();
    let (mut holder, path) = hold(&mut session, Told::default());
    session.step();
    assert_eq!(holder.said(), [Word::Up]);
    assert!(!session.compositor().started_again());

    // Programs are told the socket's name, and find the session there.
    assert_eq!(
        told_programs(&session, "WAYLAND_DISPLAY").as_deref(),
        Some("wayland-held")
    );
    let mut program = session.adopt(UnixStream::connect(&path).unwrap());
    session.open(&mut program, "app", (100, 100), crate::windows::GREEN);

    // And the IPC socket is beside it, named after it.
    let ipc = session.config.path().join("omorfos-wayland-held.sock");
    assert_eq!(
        told_programs(&session, "OMORFOS_SOCKET").as_deref(),
        ipc.to_str()
    );
    let mut peer = omorfos_ipc::Client::connect(&ipc).unwrap();
    let state = session.ask(&mut peer, &omorfos_ipc::Request::State);
    assert!(matches!(state, omorfos_ipc::Reply::State(_)), "{state:?}");

    // It is up once.
    session.advance(Duration::from_secs(1));
    assert_eq!(holder.said(), []);
}

#[test]
fn a_hand_without_a_socket_to_serve_is_refused() {
    let mut session = Session::new();
    let (holder, compositor) = UnixStream::pair().unwrap();
    hold::give(&holder, "\n", &[]).unwrap();
    let refused = session.inner().hold(compositor).unwrap_err();
    assert!(refused.to_string().contains("no socket"), "{refused}");

    // Nor one whose socket has no name to tell programs.
    let (holder, compositor) = UnixStream::pair().unwrap();
    let (nameless, _other) = UnixStream::pair().unwrap();
    hold::give(&holder, "\n", &[nameless.as_fd()]).unwrap();
    let refused = session.inner().hold(compositor).unwrap_err();
    assert!(refused.to_string().contains("no name"), "{refused}");

    // Nor one that never comes.
    let (holder, compositor) = UnixStream::pair().unwrap();
    drop(holder);
    assert!(session.inner().hold(compositor).is_err());
}

#[test]
fn the_holder_hears_when_the_session_is_locked_and_unlocked() {
    let mut session = Session::new();
    let (mut holder, _) = hold(&mut session, Told::default());
    session.step();
    assert_eq!(holder.said(), [Word::Up]);

    let (mut locker, lock, _surface) = crate::lock::lock(&mut session);
    assert_eq!(holder.said(), [Word::Locked]);
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert_eq!(holder.said(), [Word::Unlocked]);
}

#[test]
fn told_the_session_was_locked_it_comes_up_locked_and_starts_its_locker() {
    let locker = "locker: [sh, -c, 'printf x >> \"$(dirname \"$OMORFOS_SOCKET\")/locker\"']\n";
    let mut session = Session::with_config(&[("config.yaml", locker)]);
    let told = Told {
        locked: true,
        ..Told::default()
    };
    let (mut holder, _) = hold(&mut session, told);
    assert!(session.compositor().is_locked(), "before anything is drawn");
    assert_eq!(session.draw().at(WIDTH / 2, HEIGHT / 2), 0xff00_0000);
    let mark = session.config.path().join("locker");
    session.eventually(|_| mark.exists());
    session.step();
    // The holder knew already.
    assert_eq!(holder.said(), [Word::Up]);

    // The locker it started takes the lock that nobody held, and lets it
    // go as any locker does.
    let (mut locker, lock, _surface) = crate::lock::lock(&mut session);
    assert_eq!(holder.said(), []);
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert!(!session.compositor().is_locked());
    assert_eq!(holder.said(), [Word::Unlocked]);
}

#[test]
fn started_again_it_starts_its_own_programs_but_not_what_starts_with_the_session() {
    let autostart =
        "autostart: [[sh, -c, 'printf x >> \"$(dirname \"$OMORFOS_SOCKET\")/autostarted\"']]\n";
    let mut session = Session::with_config(&[("config.yaml", autostart)]);
    let panel = session.config.path().join("omorfos-panel");
    stand_in(&panel, "panel");
    session.compositor_mut().set_panel(&panel);
    let told = Told {
        again: true,
        ..Told::default()
    };
    let _held = hold(&mut session, told);
    assert!(session.compositor().started_again());
    session.compositor_mut().autostart();
    let directory = session.config.path().to_owned();
    session.eventually(|_| directory.join("panel").exists());
    session.advance(Duration::from_secs(2));
    assert!(!directory.join("autostarted").exists());
}

#[test]
fn the_first_compositor_of_a_login_starts_what_starts_with_the_session() {
    let autostart =
        "autostart: [[sh, -c, 'printf x >> \"$(dirname \"$OMORFOS_SOCKET\")/autostarted\"']]\n";
    let mut session = Session::with_config(&[("config.yaml", autostart)]);
    let _held = hold(&mut session, Told::default());
    session.compositor_mut().autostart();
    let mark = session.config.path().join("autostarted");
    session.eventually(|_| mark.exists());
}

#[test]
fn restarting_the_compositor_under_a_holder_says_so_and_ends_without_asking_programs_to() {
    let mut session = Session::new();
    let (mut holder, _) = hold(&mut session, Told::default());
    session.step();
    assert_eq!(holder.said(), [Word::Up]);
    // A program of the user's, which ending the session would ask to end,
    // and which says so if it is.
    let script = "trap 'printf x > \"$(dirname \"$OMORFOS_SOCKET\")/asked\"; exit' TERM; \
                  printf x > \"$(dirname \"$OMORFOS_SOCKET\")/running\"; sleep 30 & wait";
    let command = omorfos_config::Command::new("sh", ["-c".to_owned(), script.to_owned()]);
    session
        .compositor_mut()
        .perform(&Action::Spawn(command.unwrap().into()));
    let directory = session.config.path().to_owned();
    session.eventually(|_| directory.join("running").exists());

    session.compositor_mut().perform(&Action::RestartCompositor);
    session.step();
    assert_eq!(holder.said(), [Word::SteppingDown]);
    assert!(!session.compositor().is_running());
    assert!(session.compositor().stepping_down());
    std::thread::sleep(Duration::from_millis(500));
    assert!(!directory.join("asked").exists(), "it was asked to end");
}

#[test]
fn restarting_the_compositor_with_nothing_to_start_it_again_is_refused() {
    let mut session = Session::new();
    session.compositor_mut().perform(&Action::RestartCompositor);
    session.settle();
    assert!(session.compositor().is_running());
    assert!(!session.compositor().stepping_down());
    let said: Vec<&str> = session.compositor().events().collect();
    assert!(
        said.iter().any(|said| said.contains("omorfos-hold")),
        "{said:?}"
    );
}

#[test]
fn a_compositor_older_than_the_one_installed_says_the_fix_is_not_in() {
    let mut session = Session::new();
    let told = Told {
        older: true,
        ..Told::default()
    };
    let _held = hold(&mut session, told);
    let problems = session.compositor().problems().join("\n");
    assert!(
        problems.contains("new version would not start"),
        "{problems}"
    );
}
