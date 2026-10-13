//! The IPC socket, used through the client other programs would use.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use omorfos_ipc::{Client, PROTOCOL, Reply, Request, State};
use serde_json::json;

use crate::common::Session;
use crate::windows::{BLUE, RED};

fn state(session: &mut Session, peer: &mut Client) -> State {
    match session.ask(peer, &Request::State) {
        Reply::State(state) => state,
        other => unreachable!("expected the state, got {other:?}"),
    }
}

/// Reads the next update on a subscription.
fn update(session: &mut Session, peer: &mut Client) -> State {
    session.step();
    let Ok(Reply::State(state)) = peer.receive() else {
        unreachable!("expected an update")
    };
    state
}

#[test]
fn the_socket_is_private_to_its_owner_and_removed_afterwards() {
    let mut session = Session::new();
    let path = session.listen();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);

    let keep = session.config.path().to_owned();
    assert!(keep.join("ipc.sock").exists());
    // The directory outlives the compositor, so the check means something.
    let _directory = session.end();
    assert!(
        !path.exists(),
        "the socket file does not outlive the compositor"
    );
}

#[test]
fn a_socket_left_behind_by_an_earlier_run_is_replaced() {
    let mut session = Session::new();
    std::fs::write(session.config.path().join("ipc.sock"), "stale").unwrap();
    let path = session.listen();
    let mut peer = Client::connect(&path).unwrap();
    assert!(matches!(
        session.ask(&mut peer, &Request::Version),
        Reply::Version { .. }
    ));
}

#[test]
fn a_socket_that_cannot_be_created_is_an_error() {
    let mut session = Session::new();
    let nowhere = session.config.path().join("missing").join("ipc.sock");
    assert!(session.compositor_mut().listen_ipc(nowhere).is_err());
}

#[test]
fn version_reports_the_protocol_and_the_compositor() {
    let mut session = Session::new();
    let mut peer = Client::connect(&session.listen()).unwrap();
    assert_eq!(
        session.ask(&mut peer, &Request::Version),
        Reply::Version {
            protocol: PROTOCOL,
            compositor: env!("CARGO_PKG_VERSION").to_owned()
        }
    );
}

#[test]
fn state_lists_workspaces_windows_and_problems() {
    let config = "workspaces: [web, code]\n";
    let keys = "bindings:\n  - { keys: Super+Retrun, action: close }\n";
    let mut session = Session::with_config(&[("config.yaml", config), ("keys.yaml", keys)]);
    let mut client = session.connect();
    session.open(&mut client, "editor", (200, 100), RED);
    session.open(&mut client, "terminal", (200, 100), BLUE);
    let mut peer = Client::connect(&session.listen()).unwrap();

    let state = state(&mut session, &mut peer);

    let workspaces: Vec<_> = state
        .workspaces
        .iter()
        .map(|w| (w.number, w.name.as_str(), w.active))
        .collect();
    assert_eq!(workspaces, [(1, "web", true), (2, "code", false)]);
    let windows: Vec<_> = state
        .windows
        .iter()
        .map(|w| (w.app_id.as_str(), w.title.as_str(), w.workspace, w.focused))
        .collect();
    assert_eq!(
        windows,
        [
            ("editor", "editor window", 1, false),
            ("terminal", "terminal window", 1, true)
        ]
    );
    assert!(
        state
            .windows
            .iter()
            .all(|window| window.shown && !window.minimized)
    );
    assert_eq!(state.problems.len(), 1);
}

#[test]
fn an_action_is_performed_exactly_as_a_key_binding_would() {
    let mut session = Session::with_workspaces();
    let mut peer = Client::connect(&session.listen()).unwrap();

    let reply = session.ask(
        &mut peer,
        &Request::Perform {
            action: json!({ "workspace": 3 }),
        },
    );
    assert_eq!(reply, Reply::Done);
    assert_eq!(session.compositor().desktop().active_workspace(), 2);

    let reply = session.ask(
        &mut peer,
        &Request::Perform {
            action: json!("next_workspace"),
        },
    );
    assert_eq!(reply, Reply::Done);
    assert_eq!(session.compositor().desktop().active_workspace(), 3);
}

#[test]
fn something_that_is_not_an_action_is_refused_with_the_reason() {
    let mut session = Session::new();
    let mut peer = Client::connect(&session.listen()).unwrap();
    let reply = session.ask(
        &mut peer,
        &Request::Perform {
            action: json!("explode"),
        },
    );
    let Reply::Error { message } = reply else {
        unreachable!("expected an error")
    };
    assert!(
        message.starts_with("not an action: unknown variant `explode`"),
        "{message}"
    );
}

#[test]
fn a_window_can_be_activated_by_its_identifier() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.open(&mut client, "second", (200, 100), BLUE);
    let mut peer = Client::connect(&session.listen()).unwrap();
    let first = state(&mut session, &mut peer).windows[0].id;

    assert_eq!(
        session.ask(&mut peer, &Request::Activate { window: first }),
        Reply::Done
    );
    assert!(state(&mut session, &mut peer).windows[0].focused);
}

#[test]
fn activating_a_window_that_does_not_exist_is_refused() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "only", (200, 100), RED);
    let mut peer = Client::connect(&session.listen()).unwrap();

    assert_eq!(
        session.ask(&mut peer, &Request::Activate { window: 99_999 }),
        Reply::Error {
            message: "there is no window 99999".to_owned()
        }
    );
    assert!(state(&mut session, &mut peer).windows[0].focused);
}

#[test]
fn an_action_naming_a_workspace_that_does_not_exist_is_refused() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "only", (200, 100), RED);
    let mut peer = Client::connect(&session.listen()).unwrap();

    for action in [json!({ "workspace": 6 }), json!({ "move_to_workspace": 6 })] {
        assert_eq!(
            session.ask(&mut peer, &Request::Perform { action }),
            Reply::Error {
                message: "there is no workspace 6".to_owned()
            }
        );
    }
    let state = state(&mut session, &mut peer);
    assert!(state.workspaces[0].active);
    assert_eq!(state.windows[0].workspace, 1);

    // The last workspace does exist.
    let action = json!({ "move_to_workspace": 4 });
    assert_eq!(
        session.ask(&mut peer, &Request::Perform { action }),
        Reply::Done
    );
}

#[test]
fn reload_applies_the_configuration_on_disk() {
    let mut session = Session::new();
    let mut peer = Client::connect(&session.listen()).unwrap();
    session.write_config("config.yaml", "workspaces: [only]\n");

    assert_eq!(session.ask(&mut peer, &Request::Reload), Reply::Done);

    assert_eq!(state(&mut session, &mut peer).workspaces.len(), 1);
}

#[test]
fn a_subscriber_is_sent_the_state_now_and_whenever_it_changes() {
    let mut session = Session::new();
    let mut peer = Client::connect(&session.listen()).unwrap();

    let Reply::State(initial) = session.ask(&mut peer, &Request::Subscribe) else {
        unreachable!("expected the state")
    };
    assert_eq!(initial.windows, []);

    // Nothing changes for a while: nothing is sent.
    session.step();
    session.step();

    let mut client = session.connect();
    session.open(&mut client, "foot", (200, 100), RED);
    let first = update(&mut session, &mut peer);
    assert_eq!(
        first.windows.len(),
        1,
        "the first thing sent after the quiet spell is the change"
    );

    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Workspace(
            omorfos_config::WorkspaceNumber::clamped(2),
        ));
    let moved = update(&mut session, &mut peer);
    assert!(moved.workspaces[1].active);
}

#[test]
fn a_line_that_is_not_a_request_gets_an_error_and_the_connection_survives() {
    let mut session = Session::new();
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    raw.write_all(b"{\"nonsense\": true}\n\xff\xfe not text\n\"version\"\n")
        .unwrap();
    session.step();
    session.step();

    let mut peer = Client::from_stream(raw);
    assert!(
        matches!(peer.receive().unwrap(), Reply::Error { .. }),
        "unknown request"
    );
    assert!(
        matches!(peer.receive().unwrap(), Reply::Error { .. }),
        "not UTF-8"
    );
    assert!(
        matches!(peer.receive().unwrap(), Reply::Version { .. }),
        "still served"
    );
}

/// Waits until the compositor has closed its end of `stream`.
fn closed(session: &mut Session, stream: &mut UnixStream) -> bool {
    let _ = stream.set_nonblocking(true);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut sink = [0; 4096];
    while Instant::now() < deadline {
        session.step();
        match stream.read(&mut sink) {
            Ok(0) => return true,
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    false
}

#[test]
fn a_line_as_long_as_a_message_may_be_is_still_read() {
    let mut session = Session::new();
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    // Blank, so that the error it earns is short enough to read back.
    let mut longest = vec![b' '; omorfos_ipc::MAX_MESSAGE - 1];
    longest.extend_from_slice(b"\n\"version\"\n");
    raw.write_all(&longest).unwrap();
    session.step();
    session.step();

    let mut peer = Client::from_stream(raw);
    assert!(matches!(peer.receive().unwrap(), Reply::Error { .. }));
    assert!(matches!(peer.receive().unwrap(), Reply::Version { .. }));
}

#[test]
fn a_finished_line_longer_than_any_message_ends_the_connection_unanswered() {
    let mut session = Session::new();
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    // One byte too many, sent whole, with a request behind it.
    let mut oversized = vec![b' '; omorfos_ipc::MAX_MESSAGE];
    oversized.extend_from_slice(b"\n\"version\"\n");
    raw.write_all(&oversized).unwrap();

    raw.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut answer = [0; 64];
    let ended = loop {
        session.step();
        match raw.read(&mut answer) {
            Ok(length) => break length,
            Err(_) => assert!(Instant::now() < deadline, "still connected"),
        }
    };
    assert_eq!(ended, 0, "nothing is sent before the connection closes");
}

#[test]
fn a_line_longer_than_any_message_ends_the_connection() {
    let mut session = Session::new();
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    raw.set_nonblocking(true).unwrap();
    // No newline, ever. Writing stops when the socket's buffer is full.
    let _ = raw.write(&vec![b'x'; omorfos_ipc::MAX_MESSAGE + 4096]);
    assert!(closed(&mut session, &mut raw));
}

#[test]
fn a_peer_that_never_reads_is_dropped_instead_of_being_buffered_forever() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    window.role.set_title("t".repeat(3000));
    session.roundtrip(&mut client);
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    raw.set_nonblocking(true).unwrap();

    // Ask for the state, about 3 KiB each time, thousands of times, and
    // read none of the answers.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut refused = false;
    while !refused && Instant::now() < deadline {
        for _ in 0..200 {
            refused |= matches!(raw.write(b"\"state\"\n"), Err(error) if error.kind() != std::io::ErrorKind::WouldBlock);
        }
        session.step();
    }
    assert!(refused, "the compositor closed the connection");
}

#[test]
fn a_peer_that_stops_listening_is_dropped() {
    let mut session = Session::new();
    let path = session.listen();
    let mut raw = UnixStream::connect(&path).unwrap();
    raw.shutdown(std::net::Shutdown::Read).unwrap();
    raw.write_all(b"\"version\"\n").unwrap();
    session.step();
    session.step();

    // The answer could not be written, so the compositor hung up; writing
    // to it now fails.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut failed = false;
    while !failed && Instant::now() < deadline {
        session.step();
        failed = raw.write_all(b"\"version\"\n").is_err();
    }
    assert!(failed);
}

#[test]
fn a_peer_that_disconnects_is_forgotten_and_others_carry_on() {
    let mut session = Session::new();
    let path = session.listen();
    let mut leaving = Client::connect(&path).unwrap();
    let mut staying = Client::connect(&path).unwrap();
    assert!(matches!(
        session.ask(&mut leaving, &Request::Subscribe),
        Reply::State(_)
    ));
    drop(leaving);
    session.step();
    assert_eq!(
        session.ask(&mut staying, &Request::Version),
        session.ask(&mut staying, &Request::Version)
    );
}

#[test]
fn connections_beyond_the_limit_are_turned_away() {
    let mut session = Session::new();
    let path = session.listen();
    let mut peers: Vec<UnixStream> = (0..33)
        .map(|_| UnixStream::connect(&path).unwrap())
        .collect();
    session.step();

    let mut last = peers.pop().unwrap();
    assert!(
        closed(&mut session, &mut last),
        "the thirty-third is refused"
    );
    let mut first = Client::from_stream(peers.remove(0));
    assert!(matches!(
        session.ask(&mut first, &Request::Version),
        Reply::Version { .. }
    ));
}

#[test]
fn programs_started_by_the_compositor_are_told_where_the_socket_is() {
    let mut session = Session::new();
    let path = session.listen();
    let marker = session.config.path().join("spawned");
    let script = format!("printf %s \"$OMORFOS_SOCKET\" > '{}'", marker.display());
    let command = omorfos_config::Command::new("sh", ["-c", script.as_str()]).unwrap();

    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Spawn(command));

    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::read_to_string(&marker)
        .unwrap_or_default()
        .is_empty()
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap(),
        path.display().to_string()
    );
}

#[test]
fn the_state_says_which_workspaces_were_made_and_knows_each_by_what_does_not_change() {
    let mut session = Session::with_config(&[("config.yaml", "workspaces: [web]\n")]);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let mut peer = Client::connect(&session.listen()).unwrap();

    // One past the last is made for whoever asks, a peer as much as a key.
    let action = json!({ "workspace": 2 });
    assert_eq!(
        session.ask(&mut peer, &Request::Perform { action }),
        Reply::Done
    );
    let action = json!({ "workspace": 4 });
    assert_eq!(
        session.ask(&mut peer, &Request::Perform { action }),
        Reply::Error {
            message: "there is no workspace 4".to_owned()
        }
    );

    let state = state(&mut session, &mut peer);
    let told: Vec<(&str, u32, bool, bool)> = state
        .workspaces
        .iter()
        .map(|workspace| {
            (
                workspace.name.as_str(),
                workspace.number,
                workspace.kept,
                workspace.active,
            )
        })
        .collect();
    assert_eq!(told, [("web", 1, true, false), ("2", 2, false, true)]);
    assert_ne!(state.workspaces[0].id, state.workspaces[1].id);
    assert_ne!(state.workspaces[1].id, 0);
}

#[test]
fn the_state_says_which_workspace_was_made_for_a_fullscreen_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "player", (200, 100), RED);
    session.open(&mut client, "chat", (200, 100), RED);
    let mut peer = Client::connect(&session.listen()).unwrap();
    let action = json!("fullscreen");
    assert_eq!(
        session.ask(&mut peer, &Request::Perform { action }),
        Reply::Done
    );

    let state = state(&mut session, &mut peer);
    let chat = state.windows.iter().find(|window| window.app_id == "chat");
    let chat = chat.unwrap();
    let told: Vec<(u32, &str, bool, Option<u64>)> = state
        .workspaces
        .iter()
        .map(|workspace| {
            (
                workspace.number,
                workspace.name.as_str(),
                workspace.active,
                workspace.window,
            )
        })
        .collect();
    // It has no number, is called what its window is, and is being shown.
    assert_eq!(
        told,
        [
            (1, "1", false, None),
            (0, "chat window", true, Some(chat.id))
        ]
    );
    assert_eq!(chat.workspace, 0);
    let player = state
        .windows
        .iter()
        .find(|window| window.app_id == "player");
    assert_eq!(player.unwrap().workspace, 1);
}
