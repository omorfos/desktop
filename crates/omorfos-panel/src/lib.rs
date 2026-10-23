//! The panel of the omorfos desktop: a bar along the bottom of each screen
//! with that screen's workspaces and the windows on the one it shows.
//!
//! The panel is a program of its own. It learns what to show from the
//! compositor's IPC socket and asks for things over it, the same as any
//! script could, and it puts its bars on the screens as any Wayland client
//! would.
//!
//! [`Bar`] is all of the panel that decides anything: what is on a bar,
//! where, how it is drawn, and what a click asks for. It knows nothing of
//! Wayland or of sockets. [`Panel`] carries: news in, pictures out, clicks
//! back.

mod bar;
mod panel;

use std::ffi::OsStr;
use std::path::Path;

pub use bar::Bar;
pub use panel::{Failure, Panel};

/// Runs the panel in the session the environment describes, until it
/// cannot go on, and returns why.
///
/// The Wayland socket is `wayland_display` (`WAYLAND_DISPLAY`) in
/// `runtime_dir` (`XDG_RUNTIME_DIR`). The compositor's own socket and the
/// configuration directory are found as `omorfosctl` and the compositor
/// find them, from `socket` (`OMORFOS_SOCKET`), `config_home`
/// (`XDG_CONFIG_HOME`) and `home` (`HOME`).
///
/// The variables are passed in so that this can be used and tested without
/// touching the environment.
#[must_use]
pub fn start(
    runtime_dir: Option<&OsStr>,
    wayland_display: Option<&OsStr>,
    socket: Option<&OsStr>,
    config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Failure {
    let wayland = runtime_dir.zip(wayland_display);
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let socket = omorfos_ipc::socket_path(socket, runtime_dir, wayland_display);
    let config = omorfos_config::config_dir(config_home, home);
    match wayland.zip(socket).zip(config) {
        Some(((wayland, socket), config)) => panel::run(&wayland, &socket, &config),
        None => "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                 not set"
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_session_the_panel_says_so() {
        let runtime = Some(OsStr::new("/nowhere"));
        let display = Some(OsStr::new("wayland-none"));
        let home = Some(OsStr::new("/nowhere/home"));
        for failure in [
            start(None, display, None, None, home),
            start(runtime, None, None, None, home),
            start(runtime, display, None, None, None),
        ] {
            let message = failure.to_string();
            assert!(message.contains("not an omorfos session"), "{message}");
        }
    }

    #[test]
    fn a_session_that_is_not_there_cannot_be_joined() {
        let directory = tempfile::TempDir::new().unwrap();
        let runtime = Some(directory.path().as_os_str());
        let display = Some(OsStr::new("wayland-none"));
        let failure = start(runtime, display, None, runtime, None);
        let message = failure.to_string();
        assert!(!message.contains("not an omorfos session"), "{message}");
    }
}
