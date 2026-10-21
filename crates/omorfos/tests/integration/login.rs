//! Being the session a user logs in to: what programs are told about it,
//! and the entry that offers it on a login screen.

use std::collections::HashMap;
use std::ffi::OsString;

use omorfos_config::Action;
use serde_json::json;

use crate::common::Session;

#[test]
fn programs_are_told_what_kind_of_session_they_are_in() {
    let mut session = Session::new();
    let marker = session.config.path().join("spawned");
    let script = format!(
        "printf '%s %s' \"$XDG_SESSION_TYPE\" \"$XDG_CURRENT_DESKTOP\" > '{}'",
        marker.display()
    );
    let spawn: Action = serde_json::from_value(json!({ "spawn": ["sh", "-c", script] })).unwrap();
    session.compositor_mut().perform(&spawn);
    session
        .eventually(|_| std::fs::read_to_string(&marker).unwrap_or_default() == "wayland omorfos");
}

#[test]
fn the_session_is_described_by_where_it_is_once_that_is_known() {
    let mut session = Session::new();
    let told = |session: &Session| -> HashMap<&str, OsString> {
        session
            .compositor()
            .session_environment()
            .into_iter()
            .collect()
    };
    let before = told(&session);
    assert_eq!(before.get("WAYLAND_DISPLAY"), None, "no socket yet");
    assert_eq!(before.get("OMORFOS_SOCKET"), None);
    assert_eq!(before["XDG_SESSION_TYPE"], "wayland");
    assert_eq!(before["XCURSOR_SIZE"], "24");

    session.compositor_mut().set_socket_name("wayland-7");
    let path = session.listen();
    let after = told(&session);
    assert_eq!(after["WAYLAND_DISPLAY"], "wayland-7");
    assert_eq!(after["OMORFOS_SOCKET"], path.as_os_str());
    assert_eq!(after["XDG_CURRENT_DESKTOP"], omorfos::DESKTOP_NAME);
}

#[test]
fn the_login_screen_entry_starts_omorfos_under_the_name_programs_are_told() {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/omorfos.desktop"
    );
    let entry = std::fs::read_to_string(file).unwrap();
    let mut lines = entry.lines();
    assert_eq!(lines.next(), Some("[Desktop Entry]"));
    let keys: HashMap<&str, &str> = lines.filter_map(|line| line.split_once('=')).collect();

    assert_eq!(keys["Type"], "Application");
    assert_eq!(keys["Exec"], env!("CARGO_PKG_NAME"), "the installed binary");
    assert_eq!(
        keys["DesktopNames"],
        omorfos::DESKTOP_NAME,
        "what the login screen sets XDG_CURRENT_DESKTOP to"
    );
    assert_ne!(keys["Name"], "", "shown in the list of sessions");
}

/// A program that stands in for the panel, and leaves a mark in the
/// session's directory when it is started.
fn stand_in_panel() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/panel")
}

#[test]
fn the_panel_is_started_with_the_session_and_told_where_the_session_is() {
    let mut session = Session::new();
    session.listen();
    session.compositor_mut().set_socket_name("wayland-test");
    let directory = session.config.path().to_owned();
    let mark = |name: &str| std::fs::read_to_string(directory.join(name)).unwrap_or_default();

    session.compositor_mut().set_panel(stand_in_panel());
    assert_eq!(mark("panel-started"), "", "not before the backend is ready");
    session.compositor_mut().autostart();

    session.eventually(|_| mark("panel-display") == "wayland-test");
    // Once, however often it is asked for.
    session.compositor_mut().autostart();
    session.advance(std::time::Duration::from_millis(50));
    assert_eq!(mark("panel-started"), "x");
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
}

#[test]
fn the_notifications_are_started_with_the_session_unless_turned_off() {
    let mut session = Session::new();
    session.listen();
    let directory = session.config.path().to_owned();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    // The stand-in leaves the same mark whatever it stands in for.
    session.compositor_mut().set_notifier(stand_in_panel());
    session.compositor_mut().autostart();
    session.eventually(|_| started() == "x");

    let off = "notifications: { enabled: false }\n";
    let mut session = Session::with_config(&[("config.yaml", off)]);
    session.listen();
    let directory = session.config.path().to_owned();
    session.compositor_mut().set_notifier(stand_in_panel());
    session.compositor_mut().set_panel(stand_in_panel());
    session.compositor_mut().autostart();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    // The panel is, and nothing else.
    session.eventually(|_| started() == "x");
    session.advance(std::time::Duration::from_millis(100));
    assert_eq!(started(), "x");
}

#[test]
fn what_keeps_the_clipboard_is_started_with_the_session_unless_turned_off() {
    let mut session = Session::new();
    session.listen();
    let directory = session.config.path().to_owned();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    session.compositor_mut().set_keeper(stand_in_panel());
    session.compositor_mut().autostart();
    session.eventually(|_| started() == "x");

    let off = "clipboard: { enabled: false }\n";
    let mut session = Session::with_config(&[("config.yaml", off)]);
    session.listen();
    let directory = session.config.path().to_owned();
    session.compositor_mut().set_keeper(stand_in_panel());
    session.compositor_mut().set_panel(stand_in_panel());
    session.compositor_mut().autostart();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    // The panel is, and nothing else.
    session.eventually(|_| started() == "x");
    session.advance(std::time::Duration::from_millis(100));
    assert_eq!(started(), "x");
}

#[test]
fn what_answers_the_portals_is_started_with_the_session_unless_turned_off() {
    let mut session = Session::new();
    session.listen();
    let directory = session.config.path().to_owned();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    session.compositor_mut().set_portal(stand_in_panel());
    session.compositor_mut().autostart();
    session.eventually(|_| started() == "x");

    let off = "portals: { enabled: false }\n";
    let mut session = Session::with_config(&[("config.yaml", off)]);
    session.listen();
    let directory = session.config.path().to_owned();
    session.compositor_mut().set_portal(stand_in_panel());
    session.compositor_mut().set_panel(stand_in_panel());
    session.compositor_mut().autostart();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    // The panel is, and nothing else.
    session.eventually(|_| started() == "x");
    session.advance(std::time::Duration::from_millis(100));
    assert_eq!(started(), "x");
}

#[test]
fn what_watches_over_the_session_is_started_with_it() {
    let mut session = Session::new();
    session.listen();
    let directory = session.config.path().to_owned();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    session.compositor_mut().set_session(stand_in_panel());
    session.compositor_mut().autostart();
    // Once to watch over the session, and once to start what the autostart
    // entries say, which it does and ends.
    session.eventually(|_| started() == "xx");
    session.advance(std::time::Duration::from_millis(100));
    assert_eq!(started(), "xx", "and only once");
    let asked = || std::fs::read_to_string(directory.join("panel-asked")).unwrap_or_default();
    session.eventually(|_| asked().lines().count() == 2);
    let asked = asked();
    let mut asked: Vec<&str> = asked.lines().collect();
    asked.sort_unstable();
    assert_eq!(asked, ["", "autostart"]);
}

#[test]
fn what_asks_for_secrets_is_started_with_the_session() {
    let mut session = Session::new();
    session.listen();
    let directory = session.config.path().to_owned();
    let started = || std::fs::read_to_string(directory.join("panel-started")).unwrap_or_default();
    session.compositor_mut().set_asker(stand_in_panel());
    session.compositor_mut().autostart();
    session.eventually(|_| started() == "x");
}

#[test]
fn the_panel_is_not_started_when_the_configuration_turns_it_off() {
    let directory = tempfile::TempDir::new().unwrap();
    let after = directory.path().join("after");
    let config = format!(
        "panel: {{ enabled: false }}\nautostart:\n  - [sh, -c, \"printf x > '{}'\"]\n",
        after.display()
    );
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    session.listen();
    let started = session.config.path().join("panel-started");

    session.compositor_mut().set_panel(stand_in_panel());
    session.compositor_mut().autostart();

    // What is started after the panel has been, and the panel has not.
    session.eventually(|_| after.exists());
    assert!(!started.exists());
}

/// A program that stands in for one that runs for the whole session, and
/// does what the session's directory tells it to.
fn stand_in_component() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/component")
}

/// A session whose panel is the stand-in, told to do `what`, and how many
/// times it has been started.
fn session_whose_panel_will(what: &str, config: &str) -> Session {
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.listen();
    session.write_config("component-does", what);
    session.compositor_mut().set_panel(stand_in_component());
    session.compositor_mut().autostart();
    session
}

/// How many times the stand-in has been started.
fn starts(session: &Session) -> usize {
    let started = session.config.path().join("component-started");
    std::fs::read_to_string(started).unwrap_or_default().len()
}

/// The process the stand-in is, once it has said.
fn standing_in(session: &Session) -> Option<u32> {
    let process = session.config.path().join("component-process");
    std::fs::read_to_string(process).ok()?.parse().ok()
}

fn kill(process: u32) {
    let killed = std::process::Command::new("kill")
        .args(["-9", &process.to_string()])
        .status();
    assert!(killed.is_ok_and(|status| status.success()));
}

#[test]
fn a_panel_that_crashes_is_started_again() {
    let mut session = session_whose_panel_will("stay", "");
    session.eventually(|session| standing_in(session).is_some());
    let first = standing_in(&session).unwrap();
    assert_eq!(starts(&session), 1);

    kill(first);

    session.eventually(|session| standing_in(session).is_some_and(|now| now != first));
    assert_eq!(starts(&session), 2);
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
    // The session ending takes it down, which is not a crash.
    kill(standing_in(&session).unwrap());
}

#[test]
fn a_panel_that_keeps_crashing_is_given_up_on_and_the_user_is_told() {
    let mut session = session_whose_panel_will("fail", "");

    // Once with the session, and three times more. No time passes for the
    // compositor meanwhile: how often it starts something again goes by
    // the clock.
    for _ in 0..20_000 {
        session.step();
        if !session.compositor().problems().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(starts(&session), 4);
    let problems = session.compositor().problems().join("\n");
    assert!(problems.contains("keeps stopping"), "{problems}");
    assert!(problems.contains("fixtures/component"), "{problems}");
    session.advance(std::time::Duration::from_millis(50));
    assert_eq!(starts(&session), 4, "and no more");
}

#[test]
fn a_panel_that_stops_of_its_own_accord_is_left_stopped() {
    let mut session = session_whose_panel_will("stop", "");
    session.eventually(|session| starts(session) == 1);
    for _ in 0..20 {
        session.advance(std::time::Duration::from_millis(10));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(starts(&session), 1);
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
}

#[test]
fn a_panel_turned_off_since_it_was_started_is_not_started_again() {
    let mut session = session_whose_panel_will("stay", "");
    session.eventually(|session| standing_in(session).is_some());
    session.write_config("config.yaml", "panel: { enabled: false }\n");
    session.compositor_mut().reload_config();

    kill(standing_in(&session).unwrap());

    for _ in 0..20 {
        session.advance(std::time::Duration::from_millis(10));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(starts(&session), 1);
}

#[test]
fn the_notifications_are_started_again_if_they_crash_and_other_programs_are_not() {
    let directory = tempfile::TempDir::new().unwrap();
    let crashed = directory.path().join("crashed");
    // A program of the user's that fails as soon as it is started.
    let config = format!(
        "autostart:\n  - [sh, -c, \"printf x >> '{}'; exit 1\"]\n",
        crashed.display()
    );
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    session.listen();
    session.write_config("component-does", "stay");
    session.compositor_mut().set_notifier(stand_in_component());
    session.compositor_mut().autostart();
    session.eventually(|session| standing_in(session).is_some());
    let first = standing_in(&session).unwrap();

    kill(first);

    session.eventually(|session| standing_in(session).is_some_and(|now| now != first));
    // The user's program has failed by now, and was started the once.
    session.eventually(|_| crashed.exists());
    session.advance(std::time::Duration::from_millis(50));
    assert_eq!(std::fs::read_to_string(&crashed).unwrap(), "x", "once");
    kill(standing_in(&session).unwrap());
}

#[test]
fn a_panel_that_is_not_installed_is_a_problem_the_user_is_told_of() {
    let mut session = Session::new();
    session.compositor_mut().set_panel("/no/such/panel");
    session.compositor_mut().autostart();
    assert_eq!(
        session.compositor().problems(),
        ["cannot start `/no/such/panel`: No such file or directory (os error 2)"]
    );
}

#[test]
fn the_programs_listed_are_started_with_the_session_and_only_once() {
    let directory = tempfile::TempDir::new().unwrap();
    let marker = |name: &str| directory.path().join(name);
    let append = |name: &str| format!("printf x >> '{}'", marker(name).display());
    let config = format!(
        "autostart:\n  - [sh, -c, \"{}\"]\n  - [sh, -c, \"{}\"]\n  - [/no/such/program]\n",
        append("first"),
        append("second"),
    );
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    assert!(!marker("first").exists(), "not before the backend is ready");

    session.compositor_mut().autostart();
    let ran = |name: &str| std::fs::read_to_string(marker(name)).unwrap_or_default();
    session.eventually(|_| ran("first") == "x" && ran("second") == "x");
    assert_eq!(
        session.compositor().problems(),
        ["cannot start `/no/such/program`: No such file or directory (os error 2)"],
        "one that cannot be started is reported, and the rest still are"
    );

    // Neither asking again nor reloading the configuration starts them
    // a second time.
    session.compositor_mut().autostart();
    session.compositor_mut().reload_config();
    session.compositor_mut().autostart();
    let marker = session.config.path().join("later");
    let script = format!("printf x > '{}'", marker.display());
    let spawn: Action = serde_json::from_value(json!({ "spawn": ["sh", "-c", script] })).unwrap();
    session.compositor_mut().perform(&spawn);
    session.eventually(|_| marker.exists());
    assert_eq!(ran("first"), "x");
    assert_eq!(ran("second"), "x");
}
