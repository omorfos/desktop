//! Notifications for the omorfos desktop: what programs have to tell the
//! user, shown in a corner of the screen.
//!
//! Programs send notifications over D-Bus, by the desktop notification
//! specification, to whichever program has taken the name for it. This is
//! that program for an omorfos session. It is a program of its own, a
//! Wayland client like the panel and the launcher.
//!
//! [`Stack`] is all of it that decides anything: which notifications are
//! showing, for how long, how they are drawn and what a click does. [`Bus`]
//! hears programs out, and [`Notifier`] carries between the two and the
//! screen.

mod bus;
mod client;
mod note;

use std::error::Error;
use std::ffi::OsString;
use std::path::Path;

pub use bus::{Bus, Event};
pub use client::Notifier;
pub use note::{Note, Picture, Reason, Stack};
use omorfos_icons::Icons;

/// Why the notifications could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// Shows notifications in the session the environment describes, until it
/// cannot go on, and returns why.
///
/// `variable` looks a variable of the environment up. It is passed in so
/// that this can be used and tested without touching the environment. The
/// Wayland socket is `WAYLAND_DISPLAY` in `XDG_RUNTIME_DIR`, the bus is at
/// `DBUS_SESSION_BUS_ADDRESS`, the compositor's own socket is at
/// `OMORFOS_SOCKET`, the configuration directory is found as the
/// compositor finds it, and icons are looked for where `XDG_DATA_HOME` and
/// `XDG_DATA_DIRS` say.
#[must_use]
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Failure {
    let get = |name: &str| variable(name);
    let (runtime, display) = (get("XDG_RUNTIME_DIR"), get("WAYLAND_DISPLAY"));
    let wayland = runtime.as_deref().zip(display.as_deref());
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let home = get("HOME");
    let config = get("XDG_CONFIG_HOME");
    let config = omorfos_config::config_dir(config.as_deref(), home.as_deref());
    let data = (get("XDG_DATA_HOME"), get("XDG_DATA_DIRS"));
    let icons = Icons::directories(data.0.as_deref(), home.as_deref(), data.1.as_deref());
    let address = get("DBUS_SESSION_BUS_ADDRESS");
    let address = address.as_deref().and_then(|address| address.to_str());
    let socket = get("OMORFOS_SOCKET");
    let socket =
        omorfos_ipc::socket_path(socket.as_deref(), runtime.as_deref(), display.as_deref());
    match wayland.zip(config) {
        Some((wayland, config)) => {
            let icons = Icons::new(icons);
            client::run(&wayland, address, socket.as_deref(), &config, icons)
        }
        None => "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                 not set"
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_session_it_says_so() {
        let message = start(&|_| None).to_string();
        assert!(message.contains("not an omorfos session"), "{message}");
    }

    #[test]
    fn a_session_that_is_not_there_cannot_be_joined() {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().as_os_str().to_owned();
        let variable = move |name: &str| match name {
            "XDG_RUNTIME_DIR" | "HOME" => Some(path.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
            "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from("unix:path=/nowhere/bus")),
            _ => None,
        };
        let message = start(&variable).to_string();
        assert!(!message.contains("not an omorfos session"), "{message}");
    }
}
