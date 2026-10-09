//! xdg-activation: one application handing focus to another's window.

use omorfos::Input;

use crate::common::{Client, Session};
use crate::windows::{BLUE, RED};

fn focused_app(session: &Session) -> Option<String> {
    let desktop = session.compositor.desktop();
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
    session.compositor.input(
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
    session.compositor.input(
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
