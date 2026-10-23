//! The wallpaper of the omorfos desktop: a picture or a colour behind
//! everything on each screen.
//!
//! The wallpaper is a program of its own. It puts a surface behind
//! everything on each screen as any Wayland client would, and it learns
//! what to show from the configuration, which it reads as the compositor
//! does. Without anything to show it puts nothing up, and the theme's
//! `desktop` is all there is behind the windows.
//!
//! What a screen shows is decided by [`Slides`], which picks the picture
//! of a file or a directory, and by laying it on the screen as the
//! configuration says. [`Wallpaper`] carries: the configuration and the
//! screens in, pictures out.

mod cache;
mod lay;
pub mod picture;
mod slides;
mod wallpaper;

use std::ffi::OsStr;
use std::path::Path;

pub use slides::Slides;
pub use wallpaper::{Failure, Wallpaper};

/// Runs the wallpaper in the session the environment describes, until it
/// cannot go on, and returns why.
///
/// The Wayland socket is `wayland_display` (`WAYLAND_DISPLAY`) in
/// `runtime_dir` (`XDG_RUNTIME_DIR`). The configuration directory is found
/// as the compositor finds it, from `config_home` (`XDG_CONFIG_HOME`) and
/// `home` (`HOME`), which is also where a picture named from `~/` is. What
/// is drawn is kept in `wallpaper` in the cache directory, found the same
/// way from `cache_home` (`XDG_CACHE_HOME`).
///
/// The variables are passed in so that this can be used and tested without
/// touching the environment.
#[must_use]
pub fn start(
    runtime_dir: Option<&OsStr>,
    wayland_display: Option<&OsStr>,
    config_home: Option<&OsStr>,
    cache_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Failure {
    let wayland = runtime_dir.zip(wayland_display);
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let config = omorfos_config::config_dir(config_home, home);
    let cache = omorfos_config::cache_dir(cache_home, home);
    let cache = cache.map(|cache| cache.join("wallpaper"));
    let home = home.map(|home| Path::new(home).to_owned());
    match wayland.zip(config) {
        Some((wayland, config)) => wallpaper::run(&wayland, &config, cache, home),
        None => "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                 not set"
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_session_the_wallpaper_says_so() {
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
        let failure = start(runtime, display, runtime, runtime, None);
        let message = failure.to_string();
        assert!(!message.contains("not an omorfos session"), "{message}");
    }
}
