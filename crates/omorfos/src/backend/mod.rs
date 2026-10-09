//! Backends: where pixels go and where input comes from.
//!
//! A backend owns the screen and the input devices. It adds its input to a
//! [`Session`](crate::Session)'s event loop and draws after each turn of
//! it. Everything else is in [`Compositor`](crate::Compositor).

use std::error::Error;
use std::ffi::OsStr;

pub mod hardware;
pub mod winit;

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
