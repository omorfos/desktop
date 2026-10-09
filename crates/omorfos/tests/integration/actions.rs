//! Each action a binding or menu entry can perform.

use omorfos_config::{Action, Command, WorkspaceNumber};

use crate::common::{Event, Session};
use crate::windows::{BACKGROUND, BLUE, RED};

fn focused_app(session: &Session) -> Option<String> {
    let desktop = session.compositor().desktop();
    desktop
        .focused()
        .and_then(|id| desktop.window(id))
        .map(|window| window.app_id.clone())
}

fn workspace(number: u32) -> WorkspaceNumber {
    WorkspaceNumber::clamped(number)
}

#[test]
fn close_asks_the_focused_window_to_close() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    session.compositor_mut().perform(&Action::Close);
    session.roundtrip(&mut client);

    assert!(client.take_events().contains(&Event::Close));
}

#[test]
fn maximize_and_fullscreen_toggle_the_focused_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let state = |session: &Session| {
        let frame = session.compositor().desktop().frames().next().unwrap();
        (frame.maximized, frame.fullscreen)
    };

    session.compositor_mut().perform(&Action::Maximize);
    assert_eq!(state(&session), (true, false));
    session.compositor_mut().perform(&Action::Fullscreen);
    assert_eq!(state(&session), (true, true));
    session.compositor_mut().perform(&Action::Fullscreen);
    session.compositor_mut().perform(&Action::Maximize);
    assert_eq!(state(&session), (false, false));
}

#[test]
fn minimize_hides_the_focused_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    session.compositor_mut().perform(&Action::Minimize);
    session.step();

    assert_eq!(session.draw().at(5, 5), BACKGROUND);
}

#[test]
fn focus_cycles_between_windows() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.open(&mut client, "second", (200, 100), BLUE);

    session.compositor_mut().perform(&Action::FocusNext);
    assert_eq!(focused_app(&session).as_deref(), Some("first"));
    session.compositor_mut().perform(&Action::FocusPrevious);
    assert_eq!(focused_app(&session).as_deref(), Some("second"));

    // A window alone in its frame has no other tab to show.
    session.compositor_mut().perform(&Action::NextTab);
    session.compositor_mut().perform(&Action::PreviousTab);
    assert_eq!(focused_app(&session).as_deref(), Some("second"));
}

#[test]
fn workspaces_can_be_switched_and_windows_sent_between_them() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let active = |session: &Session| session.compositor().desktop().active_workspace();

    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(3)));
    assert_eq!(active(&session), 2);
    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!(active(&session), 3);
    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!(
        active(&session),
        0,
        "the default four workspaces wrap around"
    );
    session.compositor_mut().perform(&Action::PreviousWorkspace);
    assert_eq!(active(&session), 3);

    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(1)));
    session
        .compositor_mut()
        .perform(&Action::MoveToWorkspace(workspace(2)));
    session.step();
    assert_eq!(
        session.draw().at(5, 5),
        BACKGROUND,
        "the window left this workspace"
    );
    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(2)));
    session.step();
    assert_eq!(session.draw().at(5, 5), RED);
}

#[test]
fn actions_on_the_focused_window_do_nothing_without_one() {
    let mut session = Session::new();
    for action in [
        Action::Close,
        Action::Maximize,
        Action::Minimize,
        Action::NextTab,
    ] {
        session.compositor_mut().perform(&action);
    }
    assert_eq!(session.compositor().desktop().frames().count(), 0);
}

#[test]
fn reload_config_applies_changes_immediately() {
    let mut session = Session::new();
    session.write_config("config.yaml", "workspaces: [only]\n");
    session.compositor_mut().perform(&Action::ReloadConfig);
    assert_eq!(session.compositor().desktop().workspace_count(), 1);
}

#[test]
fn quit_ends_the_session() {
    let mut session = Session::new();
    assert!(session.compositor().is_running());
    session.compositor_mut().perform(&Action::Quit);
    assert!(!session.compositor().is_running());
}

#[test]
fn a_program_that_cannot_be_started_is_reported() {
    let mut session = Session::new();
    let command = Command::new("/nonexistent/omorfos-test-program", [""; 0]).unwrap();
    session.compositor_mut().perform(&Action::Spawn(command));
    let problems = session.compositor().problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].starts_with("cannot start `/nonexistent/omorfos-test-program`: "));
}
