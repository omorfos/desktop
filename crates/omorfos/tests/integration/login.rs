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
