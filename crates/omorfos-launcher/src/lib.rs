//! The launcher of the omorfos desktop: a box in which a program's name is
//! typed, and the programs that match to pick from.
//!
//! The launcher is a program of its own, started by the compositor when it
//! is asked for and gone once a program is picked or Escape is pressed. It
//! finds the programs there are in the desktop entries installed on the
//! system, and asks the compositor to start the one that is picked, over
//! the compositor's own socket, the same as any script could.
//!
//! [`Picker`] is all of the launcher that decides anything. [`Launcher`]
//! carries: keys in, pictures out, a request back.

mod apps;
mod client;
mod icons;
mod picker;

use std::ffi::OsString;
use std::path::Path;

pub use apps::{App, directories, installed};
pub use client::{Failure, Launcher};
pub use icons::Icons;
pub use picker::{Key, Outcome, Picker};

/// Runs the launcher in the session the environment describes, until a
/// program is picked or it is dismissed.
///
/// `variable` looks a variable of the environment up. It is passed in so
/// that this can be used and tested without touching the environment. The
/// Wayland socket is `WAYLAND_DISPLAY` in `XDG_RUNTIME_DIR`; the
/// compositor's own socket and the configuration directory are found as
/// `omorfosctl` and the compositor find them; and programs are looked for
/// where `XDG_DATA_HOME` and `XDG_DATA_DIRS` say.
///
/// # Errors
///
/// Returns why the launcher could not be shown, or could not go on.
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Result<(), Failure> {
    let get = |name: &str| variable(name);
    let (runtime, display) = (get("XDG_RUNTIME_DIR"), get("WAYLAND_DISPLAY"));
    let (runtime, display) = (runtime.as_deref(), display.as_deref());
    let home = get("HOME");
    let wayland = runtime.zip(display);
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let socket = get("OMORFOS_SOCKET");
    let socket = omorfos_ipc::socket_path(socket.as_deref(), runtime, display);
    let config = get("XDG_CONFIG_HOME");
    let config = omorfos_config::config_dir(config.as_deref(), home.as_deref());
    let data = (get("XDG_DATA_HOME"), get("XDG_DATA_DIRS"));
    let (data_home, data_dirs) = (data.0.as_deref(), data.1.as_deref());
    let places = directories(data_home, home.as_deref(), data_dirs);
    let icons = Icons::directories(data_home, home.as_deref(), data_dirs);
    match wayland.zip(socket).zip(config) {
        Some(((wayland, socket), config)) => {
            let offered = (installed(&places), Icons::new(icons));
            client::run(&wayland, &socket, &config, offered)
        }
        None => Err(
            "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME \
                     is not set"
                .into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_session_the_launcher_says_so() {
        let failure = start(&|_| None).unwrap_err();
        let message = failure.to_string();
        assert!(message.contains("not an omorfos session"), "{message}");
    }

    #[test]
    fn a_session_that_is_not_there_cannot_be_joined() {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().as_os_str().to_owned();
        let variable = move |name: &str| match name {
            "XDG_RUNTIME_DIR" | "HOME" | "XDG_DATA_HOME" => Some(path.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
            _ => None,
        };
        let failure = start(&variable).unwrap_err();
        let message = failure.to_string();
        assert!(!message.contains("not an omorfos session"), "{message}");
    }
}
