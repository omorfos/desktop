//! Each action a binding or menu entry can perform.

use std::time::Duration;

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
    let mut session = Session::with_workspaces();
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
    assert_eq!(active(&session), 0, "the four workspaces wrap around");
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

/// Has the compositor start a shell that first runs `script`, given where
/// the session's directory is, and waits until it is running.
#[expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]
fn started(session: &mut Session, script: impl Fn(&str) -> String) {
    let there = session.config.path().display().to_string();
    let ready = session.config.path().join("ready");
    let script = format!("{} touch '{there}/ready'; sleep 10 & wait", script(&there));
    let command = Command::new("sh", ["-c".to_owned(), script]).unwrap();
    session
        .compositor_mut()
        .perform(&Action::Spawn(command.into()));
    session.eventually(|_| ready.exists());
}

#[test]
fn the_programs_started_in_the_session_are_asked_to_end_before_it_does() {
    let mut session = Session::new();
    started(&mut session, |there| {
        format!("trap 'touch \"{there}/asked\"; exit 0' TERM;")
    });
    session.compositor_mut().perform(&Action::Quit);
    assert!(session.compositor().is_running(), "waiting for it");
    let asked = session.config.path().join("asked");
    session.eventually(|session| !session.compositor().is_running());
    assert!(asked.exists(), "it was asked");
}

#[test]
fn a_program_that_will_not_end_is_waited_for_a_while_or_until_asked_again() {
    let mut session = Session::new();
    started(&mut session, |_| "trap '' TERM;".to_owned());
    session.compositor_mut().perform(&Action::Quit);
    session.step();
    // Woken when the while is up, should nothing else wake it.
    let by = session.compositor().deadline();
    session.advance(Duration::from_millis(1900));
    assert!(session.compositor().is_running(), "a while");
    assert!(by.is_some_and(|by| session.compositor().deadline() <= Some(by)));
    session.advance(Duration::from_millis(200));
    assert!(!session.compositor().is_running(), "and no longer");
    // Asked again, it ends at once.
    let mut session = Session::new();
    started(&mut session, |_| "trap '' TERM;".to_owned());
    session.compositor_mut().perform(&Action::Quit);
    session.step();
    assert!(session.compositor().is_running());
    session.compositor_mut().perform(&Action::Quit);
    assert!(!session.compositor().is_running());
}

#[test]
fn a_program_that_cannot_be_started_is_reported() {
    let mut session = Session::new();
    let command = Command::new("/nonexistent/omorfos-test-program", [""; 0]).unwrap();
    session
        .compositor_mut()
        .perform(&Action::Spawn(command.into()));
    let problems = session.compositor().problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].starts_with("cannot start `/nonexistent/omorfos-test-program`: "));
}

#[test]
fn there_is_one_workspace_and_more_are_made_as_they_are_asked_for() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let active = |session: &Session| session.compositor().desktop().active_workspace();
    let count = |session: &Session| session.compositor().desktop().workspace_count();
    assert_eq!(count(&session), 1);

    // The number one past the last makes that workspace.
    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(2)));
    assert_eq!((active(&session), count(&session)), (1, 2));
    session.step();
    assert_eq!(session.draw().at(100, 50), BACKGROUND);

    // Further than that is nowhere.
    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(4)));
    assert_eq!((active(&session), count(&session)), (1, 2));

    // Left with nothing on it, the one that was made goes.
    session
        .compositor_mut()
        .perform(&Action::Workspace(workspace(1)));
    assert_eq!((active(&session), count(&session)), (0, 1));
}

#[test]
fn a_workspace_is_made_by_asking_for_a_new_one_or_by_sending_a_window_to_one() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let active = |session: &Session| session.compositor().desktop().active_workspace();
    let count = |session: &Session| session.compositor().desktop().workspace_count();

    session.compositor_mut().perform(&Action::NewWorkspace);
    session.step();
    assert_eq!((active(&session), count(&session)), (1, 2));
    assert_eq!(session.draw().at(100, 50), BACKGROUND, "nothing on it yet");

    session.compositor_mut().perform(&Action::PreviousWorkspace);
    assert_eq!(count(&session), 1, "left empty, it has gone");

    // The window goes to a workspace made for it, and stays there when
    // another is shown.
    session
        .compositor_mut()
        .perform(&Action::MoveToNewWorkspace);
    session.step();
    assert_eq!((active(&session), count(&session)), (0, 2));
    assert_eq!(session.draw().at(100, 50), BACKGROUND);
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 50), RED);
}

#[test]
fn a_dialog_opens_over_its_fullscreen_window_and_any_other_window_on_the_desktop() {
    let mut session = Session::new();
    let mut client = session.connect();
    let player = session.open(&mut client, "player", (200, 100), RED);
    session.compositor_mut().perform(&Action::Fullscreen);
    let active = |session: &Session| session.compositor().desktop().active_workspace();
    let frames = |session: &Session| session.compositor().desktop().frames().count();
    assert_eq!((active(&session), frames(&session)), (1, 1));

    // One that says it belongs to the fullscreen window is with it.
    let dialog = client.toplevel_uncommitted("player", "dialog");
    dialog.role.set_parent(Some(&player.role));
    dialog.surface.commit();
    session.roundtrip(&mut client);
    client.fill(&dialog.surface, (100, 50), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!((active(&session), frames(&session)), (1, 2));

    // Another window of the same program is not: it opens on the desktop
    // the fullscreen one came from, which nobody asked to see.
    session.open(&mut client, "player", (200, 100), RED);
    assert_eq!((active(&session), frames(&session)), (1, 2));
    let windows = session.compositor().desktop().windows().count();
    assert_eq!(windows, 3);
}

#[test]
fn two_windows_share_a_workspace_made_for_them_side_by_side() {
    use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "editor", (200, 100), RED);
    let half = omorfos_config::Half::Left;
    session.compositor_mut().perform(&Action::Tile(half));
    session.roundtrip(&mut client);

    // It is told the half it has, and that there is something against
    // every edge of it; not that it is fullscreen.
    let (size, states) = client.last_configure();
    assert_eq!(size, (320, 480));
    assert!(states.contains(&State::TiledLeft) && states.contains(&State::TiledBottom));
    assert!(!states.contains(&State::Fullscreen));
    assert_eq!(session.compositor().desktop().active_workspace(), 1);

    // The next window to open there has the other half.
    let mut other = session.connect();
    session.open(&mut other, "browser", (200, 100), 0xff00_ff00);
    session.roundtrip(&mut other);
    let (size, states) = other.last_configure();
    assert_eq!(size, (320, 480));
    assert!(states.contains(&State::TiledRight));
    let picture = session.draw();
    assert_eq!(picture.at(10, 10), RED, "the first in the left half");
    assert_eq!(picture.at(330, 10), 0xff00_ff00, "the second in the right");

    // When one closes, the other is alone, and is told it is fullscreen.
    drop(other);
    session.eventually(|session| session.compositor().desktop().frames().count() == 1);
    session.roundtrip(&mut client);
    let (size, states) = client.last_configure();
    assert_eq!(size, (640, 480));
    assert!(states.contains(&State::Fullscreen) && !states.contains(&State::TiledLeft));
}
