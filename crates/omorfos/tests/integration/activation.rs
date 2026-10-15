//! xdg-activation: one application handing focus to another's window.

use omorfos::Input;

use crate::common::{Client, Session};
use crate::windows::{BLUE, RED};

fn focused_app(session: &Session) -> Option<String> {
    let desktop = session.compositor().desktop();
    desktop
        .focused()
        .and_then(|id| desktop.window(id))
        .map(|window| window.app_id.clone())
}

/// Gets a token for `client`, tied to user input or not.
fn token(session: &mut Session, client: &mut Client, from_input: bool) -> String {
    client.state.token = None;
    client.request_token(from_input);
    session.roundtrip(client);
    client.state.token.clone().unwrap_or_default()
}

#[test]
fn a_launcher_the_user_clicked_can_bring_a_window_forward() {
    let mut session = Session::new();
    let mut documents = session.connect();
    let report = session.open(&mut documents, "documents", (200, 100), RED);
    let mut launcher = session.connect();
    session.open(&mut launcher, "launcher", (200, 100), BLUE);
    assert_eq!(focused_app(&session).as_deref(), Some("launcher"));

    // The user presses a key in the launcher, which then asks for the
    // documents window to be shown.
    session.compositor_mut().input(
        Input::Key {
            code: crate::common::key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut launcher);
    let token = token(&mut session, &mut launcher, true);
    documents.activate(&token, &report.surface);
    session.roundtrip(&mut documents);

    assert_eq!(focused_app(&session).as_deref(), Some("documents"));
}

#[test]
fn a_window_cannot_raise_itself_without_the_user_having_done_anything() {
    let mut session = Session::new();
    let mut pushy = session.connect();
    let window = session.open(&mut pushy, "pushy", (200, 100), RED);
    let mut other = session.connect();
    session.open(&mut other, "other", (200, 100), BLUE);

    let token = token(&mut session, &mut pushy, false);
    pushy.activate(&token, &window.surface);
    session.roundtrip(&mut pushy);

    assert_eq!(focused_app(&session).as_deref(), Some("other"));
}

#[test]
fn activating_something_that_is_not_a_window_does_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    session.compositor_mut().input(
        Input::Key {
            code: crate::common::key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut client);
    let token = token(&mut session, &mut client, true);

    let stray = client.surface();
    client.activate(&token, &stray);
    session.roundtrip(&mut client);

    assert_eq!(focused_app(&session).as_deref(), Some("app"));
}

/// A session showing a red window fullscreen, on a workspace of its own.
fn fullscreen() -> (Session, Client) {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "player", (200, 100), RED);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Fullscreen);
    session.step();
    (session, client)
}

fn active(session: &Session) -> usize {
    session.compositor().desktop().active_workspace()
}

#[test]
fn a_window_nobody_asked_for_waits_unseen_and_one_asked_for_is_shown() {
    let (mut session, _player) = fullscreen();
    let token = session.started_saying("$XDG_ACTIVATION_TOKEN");
    assert_eq!(token.len(), 32, "{token:?}");

    // It opens on the desktop, and the user stays where they were.
    let mut program = session.connect();
    let window = session.open(&mut program, "started", (200, 100), BLUE);
    assert_eq!(active(&session), 1);
    assert_eq!(focused_app(&session).as_deref(), Some("player"));

    // The program says the user asked for it, with the token it was
    // started with: it is shown.
    program.activate(&token, &window.surface);
    session.roundtrip(&mut program);
    assert_eq!(active(&session), 0);
    assert_eq!(focused_app(&session).as_deref(), Some("started"));

    // A token says so once.
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::NextWorkspace);
    session.step();
    assert_eq!(active(&session), 1);
    program.activate(&token, &window.surface);
    session.roundtrip(&mut program);
    assert_eq!(active(&session), 1);
}

#[test]
fn a_window_asked_for_before_it_opens_is_shown_when_it_does() {
    let (mut session, _player) = fullscreen();
    let token = session.started_saying("$DESKTOP_STARTUP_ID");
    let mut program = session.connect();
    let window = program.toplevel("started", "started");
    session.roundtrip(&mut program);
    program.activate(&token, &window.surface);
    session.roundtrip(&mut program);
    assert_eq!(active(&session), 1, "not before it opens");

    program.fill(&window.surface, (200, 100), BLUE);
    session.roundtrip(&mut program);
    session.roundtrip(&mut program);
    assert_eq!(active(&session), 0);
    assert_eq!(focused_app(&session).as_deref(), Some("started"));
}
