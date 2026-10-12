//! Backends: where pixels go and where input comes from.
//!
//! A backend owns the screen and the input devices. It adds its input to a
//! [`Session`](crate::Session)'s event loop and draws after each turn of
//! it. Everything else is in [`Compositor`](crate::Compositor).

use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub mod hardware;
pub mod winit;

/// What the panel's executable is called.
const PANEL: &str = "omorfos-panel";

/// Where the panel is, for a compositor whose own executable is `own`.
///
/// When omorfos is run from where it was built, or from wherever else it
/// was put without being installed, the panel is beside it, and is not to
/// be mistaken for some other one installed on the system. Otherwise it is
/// whichever `PATH` has.
#[must_use]
pub fn panel_program(own: Option<&Path>) -> PathBuf {
    beside(own, PANEL)
}

/// What the launcher's executable is called.
const LAUNCHER: &str = "omorfos-launcher";

/// Where the launcher is, found as [`panel_program`] finds the panel.
#[must_use]
pub fn launcher_program(own: Option<&Path>) -> PathBuf {
    beside(own, LAUNCHER)
}

/// The program called `name` beside the executable `own`, if there is one
/// there, and otherwise just its name, to be looked up in `PATH`.
fn beside(own: Option<&Path>, name: &str) -> PathBuf {
    let beside = own.and_then(Path::parent).map(|folder| folder.join(name));
    let beside = beside.filter(|program| program.is_file());
    beside.unwrap_or_else(|| PathBuf::from(name))
}

/// A way of running the compositor until the session ends.
pub type Launcher = fn() -> Result<(), Box<dyn Error>>;

/// The backends there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// A window on an existing Wayland or X11 session.
    Nested,
    /// The screen, keyboard and mouse themselves.
    Hardware,
}

impl Backend {
    /// Picks the backend that suits where the compositor is started: nested
    /// if there is a session to open a window on, which `WAYLAND_DISPLAY`
    /// or `DISPLAY` says, and the hardware otherwise.
    #[must_use]
    pub fn choose(wayland_display: Option<&OsStr>, display: Option<&OsStr>) -> Self {
        let named = |variable: Option<&OsStr>| variable.is_some_and(|value| !value.is_empty());
        if named(wayland_display) || named(display) {
            Self::Nested
        } else {
            Self::Hardware
        }
    }

    /// What runs the compositor on this backend.
    #[must_use]
    pub fn launcher(self) -> Launcher {
        match self {
            Self::Nested => winit::run,
            Self::Hardware => hardware::run,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::fn_addr_eq;

    use super::*;

    #[test]
    fn the_panel_beside_the_compositor_is_the_one_to_start() {
        let folder = tempfile::TempDir::new().unwrap();
        let own = folder.path().join("omorfos");
        assert_eq!(panel_program(Some(&own)), Path::new(PANEL), "not there");
        std::fs::write(folder.path().join(PANEL), "").unwrap();
        assert_eq!(panel_program(Some(&own)), folder.path().join(PANEL));
    }

    #[test]
    fn without_one_beside_it_the_panel_is_looked_up_by_name() {
        assert_eq!(panel_program(None), Path::new(PANEL));
        assert_eq!(launcher_program(None), Path::new(LAUNCHER));
        assert_eq!(panel_program(Some(Path::new("/"))), Path::new(PANEL));
    }

    #[test]
    fn a_session_to_open_a_window_on_means_running_nested() {
        assert_eq!(
            Backend::choose(Some(OsStr::new("wayland-0")), None),
            Backend::Nested
        );
        assert_eq!(
            Backend::choose(None, Some(OsStr::new(":0"))),
            Backend::Nested
        );
        assert_eq!(
            Backend::choose(Some(OsStr::new("wayland-0")), Some(OsStr::new(":0"))),
            Backend::Nested
        );
    }

    #[test]
    fn with_no_session_the_hardware_is_driven() {
        assert_eq!(Backend::choose(None, None), Backend::Hardware);
        assert_eq!(
            Backend::choose(Some(OsStr::new("")), Some(OsStr::new(""))),
            Backend::Hardware,
            "a variable that is set but empty names nothing"
        );
    }

    #[test]
    fn each_backend_has_its_own_way_of_running() {
        let nested: Launcher = winit::run;
        let hardware: Launcher = hardware::run;
        assert!(fn_addr_eq(Backend::Nested.launcher(), nested));
        assert!(fn_addr_eq(Backend::Hardware.launcher(), hardware));
    }
}
