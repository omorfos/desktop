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
