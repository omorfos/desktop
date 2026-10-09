//! Configuration: what is applied at startup and when files change.

use tempfile::TempDir;

use crate::common::Session;
use crate::windows::{BACKGROUND, RED};

#[test]
fn the_defaults_have_no_problems() {
    let session = Session::new();
    assert_eq!(session.compositor.problems(), [""; 0]);
    assert_eq!(session.compositor.desktop().workspace_count(), 4);
}

#[test]
fn the_configuration_directory_is_created_on_first_run() {
    let home = TempDir::new().unwrap();
    let directory = home.path().join("config").join("omorfos");
    let session = Session::in_directory(home, Some(&directory));
    assert!(directory.is_dir());
    assert_eq!(session.compositor.problems(), [""; 0]);
}

#[test]
fn a_directory_that_cannot_be_used_is_reported_and_defaults_apply() {
    let home = TempDir::new().unwrap();
    std::fs::write(home.path().join("file"), "").unwrap();
    let directory = home.path().join("file").join("omorfos");
    let session = Session::in_directory(home, Some(&directory));

    let problems = session.compositor.problems();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("changes will not be picked up automatically")),
        "{problems:?}"
    );
    assert_eq!(session.compositor.desktop().workspace_count(), 4);
}

#[test]
fn settings_are_applied_at_startup() {
    let session = Session::with_config(&[("config.yaml", "workspaces: [web, code]\n")]);
    assert_eq!(session.compositor.desktop().workspace_count(), 2);
    assert_eq!(
        session.compositor.config().general.workspaces.names(),
        ["web", "code"]
    );
}

#[test]
fn rules_from_the_configuration_apply_to_new_windows() {
    let rules = "rules:\n  - match: { app_id: chat }\n    workspace: 2\n";
    let mut session = Session::with_config(&[("rules.yaml", rules)]);
    let mut client = session.connect();

    session.open(&mut client, "chat", (200, 100), RED);

    assert_eq!(session.compositor.desktop().frames().count(), 0);
    assert_eq!(session.draw().at(5, 5), BACKGROUND);
}

#[test]
fn a_mistake_in_a_file_is_reported_with_its_location() {
    let session = Session::with_config(&[("theme.yaml", "colors:\n  accent: nope\n")]);
    let problems = session.compositor.problems();
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0]
            .ends_with("theme.yaml:2:11: invalid color `nope`: expected `#rrggbb` or `#rrggbbaa`"),
        "{problems:?}"
    );
}

#[test]
fn a_binding_for_a_key_that_does_not_exist_is_reported() {
    let bindings = "bindings:\n  - { keys: Super+Retrun, action: close }\n";
    let session = Session::with_config(&[("keys.yaml", bindings)]);
    assert_eq!(
        session.compositor.problems(),
        ["keys.yaml: `Super+Retrun` names a key that does not exist; the binding is ignored"]
    );
}

#[test]
fn an_invalid_keyboard_layout_is_reported() {
    let layout = "keyboard: { layout: no-such-layout }\n";
    let session = Session::with_config(&[("config.yaml", layout)]);
    let problems = session.compositor.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("config.yaml: keyboard layout `no-such-layout`"));
}

#[test]
fn a_saved_change_is_applied_once_the_directory_goes_quiet() {
    let mut session = Session::new();
    assert_eq!(session.draw().at(5, 5), BACKGROUND);

    session.write_config("theme.yaml", "colors: { background: '#ff0000' }\n");
    session.step();
    assert_eq!(
        session.draw().at(5, 5),
        BACKGROUND,
        "not reloaded while changes may still arrive"
    );

    session.eventually(|session| session.draw().at(5, 5) == RED);
}

#[test]
fn a_broken_save_keeps_the_previous_configuration_and_reports_the_problem() {
    let mut session =
        Session::with_config(&[("theme.yaml", "colors: { background: '#ff0000' }\n")]);

    session.write_config("theme.yaml", "colors: { background: oops }\n");
    session.eventually(|session| session.compositor.problems().len() == 1);
    assert_eq!(
        session.draw().at(5, 5),
        RED,
        "the previous colour is still in effect"
    );

    session.write_config("theme.yaml", "colors: { background: '#ff0000' }\n");
    session.eventually(|session| session.compositor.problems().is_empty());
}

#[test]
fn a_window_a_rule_opens_fullscreen_is_told_so_before_it_draws() {
    use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

    use crate::common::{Event, HEIGHT, WIDTH};

    let rules = "rules:\n  - match: { app_id: player }\n    fullscreen: true\n  - match: { app_id: editor }\n    maximized: true\n";
    let mut session = Session::with_config(&[("rules.yaml", rules)]);
    let mut client = session.connect();

    let player = client.toplevel("player", "");
    session.roundtrip(&mut client);
    assert_eq!(
        client.take_events(),
        [Event::Configure {
            size: (WIDTH, HEIGHT),
            states: vec![State::Fullscreen]
        }],
        "the very first configure already carries the final size"
    );
    client.fill(&player.surface, (WIDTH, HEIGHT), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(session.draw().at(WIDTH - 1, HEIGHT - 1), RED);

    client.toplevel("editor", "");
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::Configure {
            size: (WIDTH, HEIGHT),
            states: vec![State::Maximized]
        }),
        "{events:?}"
    );
}
