//! The program that keeps what is copied, run against a compositor as it is
//! in a session.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::ffi::OsString;
use std::io::Read;
use std::thread::JoinHandle;
use std::time::Duration;

use omorfos::Roles;
use omorfos_clipboard::Limits;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{Client, Session, TEXT};
use crate::windows::{BLUE, RED};

/// The form a password manager says what it copies is not to be kept in.
const SECRET: &str = "x-kde-passwordManagerHint";

/// Starts the keeper in a session, over a socket that gives `roles`. It
/// says why it stopped when it does.
pub(crate) fn keeper(session: &mut Session, roles: Roles, limits: Limits) -> JoinHandle<String> {
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-keeper")).unwrap();
    session.inner().serve(socket, roles).unwrap();
    std::thread::spawn(move || {
        let variable = |name: &str| match name {
            "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-keeper")),
            _ => None,
        };
        said(omorfos_clipboard::start_with(&variable, limits))
    })
}

/// What a component that ended said: why it could not go on, or, as its
/// program would say nothing then, that the session ended.
pub(crate) fn said(failure: Box<dyn std::error::Error + Send + Sync>) -> String {
    omorfos_shell::ended::quietly(failure).err().map_or_else(
        || "the session ended".to_owned(),
        |failure| failure.to_string(),
    )
}

/// A session with the keeper, and a window that has the keyboard.
fn kept(limits: Limits) -> (Session, JoinHandle<String>, Client) {
    let mut session = Session::new();
    let keeper = keeper(&mut session, Roles::CLIPBOARD, limits);
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    (session, keeper, editor)
}

/// Copies `text` from `client`, offered in `formats`, and lets the keeper
/// read it, until `done` says it is.
fn copied(
    session: &mut Session,
    client: &mut Client,
    (text, formats): (&str, &[&str]),
    done: impl Fn(&Client) -> bool,
) {
    client.copy_as(text, formats);
    session.eventually(|session| {
        session.roundtrip(client);
        done(client)
    });
}

/// What another window, given the keyboard, finds on the clipboard: the
/// text, or `None` if there is nothing there.
fn pasted(session: &mut Session) -> Option<String> {
    let mut terminal = session.connect();
    session.open(&mut terminal, "terminal", (200, 100), BLUE);
    let mut pipe = terminal.paste()?;
    session.roundtrip(&mut terminal);
    let mut text = String::new();
    pipe.read_to_string(&mut text).unwrap();
    Some(text)
}

/// Ends the session, and with it the keeper, and says why it stopped.
fn end(session: Session, keeper: JoinHandle<String>) -> String {
    drop(session);
    keeper.join().unwrap()
}

#[test]
fn what_is_copied_can_be_pasted_after_the_program_that_copied_closes() {
    let (mut session, keeper, mut editor) = kept(Limits::default());
    let forms = [TEXT, "text/plain", ""];
    copied(&mut session, &mut editor, ("kept", &forms), |editor| {
        editor.state.replaced
    });
    assert_eq!(editor.state.asked, 2, "in every form it has a name for");
    // Selecting, for the middle button, is not kept. It is done once the
    // keeper has shown it is there by keeping a copy: the keeper is told
    // of what is selected only as it changes, so one that came later would
    // not be told of it at all.
    editor.select("only selected");
    session.roundtrip(&mut editor);
    drop(editor);
    assert_eq!(pasted(&mut session).as_deref(), Some("kept"));

    // Copied over, what was kept is let go, and the new one kept.
    let mut other = session.connect();
    session.open(&mut other, "other", (200, 100), BLUE);
    copied(&mut session, &mut other, ("newer", &[TEXT]), |other| {
        other.state.replaced
    });
    drop(other);
    assert_eq!(pasted(&mut session).as_deref(), Some("newer"));

    // What comes later, a screen say, is nothing to it.
    session.add_output((640, 480));
    assert_eq!(pasted(&mut session).as_deref(), Some("newer"));
    let said = end(session, keeper);
    assert!(said.starts_with("the session ended"), "{said}");
}

#[test]
fn what_is_kept_is_each_form_of_data_once_that_comes_in_time() {
    let quick = Limits {
        wait: Duration::from_millis(100),
        ..Limits::default()
    };
    let (mut session, keeper, mut editor) = kept(quick);
    // As a program written for X11 offers what it copied, through the X
    // server: forms twice, and what it can be asked to do.
    editor.state.withheld = vec!["UTF8_STRING".to_owned()];
    let forms = [TEXT, "UTF8_STRING", TEXT, "SAVE_TARGETS", "TARGETS"];
    copied(&mut session, &mut editor, ("kept", &forms), |editor| {
        editor.state.replaced
    });
    assert_eq!(editor.state.asked, 2, "once each, and only for data");
    drop(editor);

    let mut terminal = session.connect();
    session.open(&mut terminal, "terminal", (200, 100), BLUE);
    let offered: Vec<&str> = terminal.state.formats.iter().map(String::as_str).collect();
    assert_eq!(offered, [TEXT, "application/x-omorfos-kept"], "what came");
    drop(terminal);
    assert_eq!(pasted(&mut session).as_deref(), Some("kept"));
    end(session, keeper);
}

#[test]
fn what_a_password_manager_copies_is_not_kept() {
    let (mut session, keeper, mut editor) = kept(Limits::default());
    editor.copy_as("secret", &[TEXT, SECRET]);
    for _ in 0..20 {
        session.roundtrip(&mut editor);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(editor.state.asked, 0, "not even read");
    assert!(!editor.state.replaced);
    drop(editor);
    session.eventually(|session| pasted(session).is_none());
    end(session, keeper);
}

#[test]
fn what_is_too_large_or_too_slow_is_left_with_the_program_that_copied() {
    let small = Limits {
        most: 4,
        ..Limits::default()
    };
    let (mut session, keeper, mut editor) = kept(small);
    copied(
        &mut session,
        &mut editor,
        ("too large", &[TEXT]),
        |editor| editor.state.asked == 1,
    );
    for _ in 0..20 {
        session.roundtrip(&mut editor);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!editor.state.replaced);
    end(session, keeper);

    let slow = Limits {
        wait: Duration::from_millis(50),
        ..Limits::default()
    };
    let (mut session, keeper, mut editor) = kept(slow);
    editor.state.holding = true;
    copied(&mut session, &mut editor, ("slow", &[TEXT]), |editor| {
        editor.state.asked == 1
    });
    std::thread::sleep(Duration::from_millis(100));
    // Handed over too late, it is not kept; what is copied next is.
    editor.hand_over();
    assert!(!editor.state.replaced);
    let mut other = session.connect();
    session.open(&mut other, "other", (200, 100), BLUE);
    copied(&mut session, &mut other, ("quick", &[TEXT]), |other| {
        other.state.replaced
    });
    drop((editor, other));
    assert_eq!(pasted(&mut session).as_deref(), Some("quick"));
    end(session, keeper);
}

#[test]
fn something_copied_while_the_last_is_read_is_kept_in_its_place() {
    let (mut session, keeper, mut editor) = kept(Limits::default());
    editor.state.holding = true;
    copied(&mut session, &mut editor, ("first", &[TEXT]), |editor| {
        editor.state.asked == 1
    });
    // Something else is copied before the first is handed over.
    let mut other = session.connect();
    session.open(&mut other, "other", (200, 100), BLUE);
    other.copy("second");
    session.roundtrip(&mut other);
    editor.hand_over();
    session.eventually(|session| {
        session.roundtrip(&mut editor);
        session.roundtrip(&mut other);
        other.state.replaced
    });
    drop((editor, other));
    assert_eq!(pasted(&mut session).as_deref(), Some("second"));
    end(session, keeper);
}

#[test]
fn what_the_program_that_copied_takes_away_while_it_is_read_is_not_kept() {
    let (mut session, keeper, mut editor) = kept(Limits::default());
    editor.state.holding = true;
    copied(&mut session, &mut editor, ("going", &[TEXT]), |editor| {
        editor.state.asked == 1
    });
    drop(editor);
    for _ in 0..20 {
        session.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(pasted(&mut session), None);
    end(session, keeper);
}

#[test]
fn the_keeper_needs_the_clipboard_role_and_a_session() {
    let mut session = Session::new();
    let keeper = keeper(&mut session, Roles::NONE, Limits::default());
    session.eventually(|_| keeper.is_finished());
    let said = keeper.join().unwrap();
    assert!(said.contains("may: [clipboard]"), "{said}");

    let nowhere = |_: &str| None;
    let said = omorfos_clipboard::start(&nowhere).to_string();
    assert!(said.starts_with("this is not an omorfos session"), "{said}");
    let runtime = session.config.path().to_owned();
    let absent = |name: &str| match name {
        "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
        "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
        _ => None,
    };
    let said = omorfos_clipboard::start(&absent).to_string();
    assert!(said.contains("wayland-none"), "{said}");
}
