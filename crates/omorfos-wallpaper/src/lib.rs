//! The wallpaper of the omorfos desktop: a picture or a colour behind
//! everything on each screen.
//!
//! The wallpaper is a program of its own. It puts a surface behind
//! everything on each screen as any Wayland client would, and it learns
//! what to show from the configuration, which it reads as the compositor
//! does. Where the configuration names nothing, it shows the wallpapers
//! that come with omorfos, the dark one or the light one as the theme is;
//! and without those, nothing, and the theme's `desktop` is all there is
//! behind the windows.
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

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use omorfos_config::Picture;

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
/// way from `cache_home` (`XDG_CACHE_HOME`). The wallpapers that come with
/// omorfos are looked for in `data` (`XDG_DATA_HOME` and `XDG_DATA_DIRS`),
/// as [`installed`] says.
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
    data: (Option<&OsStr>, Option<&OsStr>),
) -> Failure {
    let wayland = runtime_dir.zip(wayland_display);
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let config = omorfos_config::config_dir(config_home, home);
    let cache = omorfos_config::cache_dir(cache_home, home);
    let cache = cache.map(|cache| cache.join("wallpaper"));
    let unnamed = installed(data, home);
    let home = home.map(|home| Path::new(home).to_owned());
    match wayland.zip(config) {
        Some((wayland, config)) => wallpaper::run(&wayland, &config, cache, (home, unnamed)),
        None => "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                 not set"
            .into(),
    }
}

/// The wallpapers that come with omorfos, a dark one and a light one, in
/// `backgrounds/omorfos` of the first directory of data that has both: the
/// user's own (`XDG_DATA_HOME`, or `~/.local/share`) and then the
/// system's (`XDG_DATA_DIRS`, or `/usr/local/share` and `/usr/share`), as
/// the XDG base directories have it.
#[must_use]
pub fn installed(
    (data_home, data_dirs): (Option<&OsStr>, Option<&OsStr>),
    home: Option<&OsStr>,
) -> Option<Picture> {
    let own = data_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| Path::new(home).join(".local/share")));
    let system = data_dirs.filter(|dirs| !dirs.is_empty());
    let system = system.unwrap_or_else(|| OsStr::new("/usr/local/share:/usr/share"));
    let directories = own.into_iter().chain(env::split_paths(system));
    // One that is not from the root is passed over, as the specification
    // says.
    let mut directories = directories.filter(|directory| directory.is_absolute());
    directories.find_map(|directory| {
        let there = directory.join("backgrounds/omorfos");
        let (dark, light) = (there.join("prism-dark.png"), there.join("prism-light.png"));
        (dark.is_file() && light.is_file()).then(|| Picture::paired(dark, light))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_session_the_wallpaper_says_so() {
        let runtime = Some(OsStr::new("/nowhere"));
        let display = Some(OsStr::new("wayland-none"));
        let home = Some(OsStr::new("/nowhere/home"));
        let data = (None, None);
        for failure in [
            start(None, display, None, None, home, data),
            start(runtime, None, None, None, home, data),
            start(runtime, display, None, None, None, data),
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
        let failure = start(runtime, display, runtime, runtime, None, (None, None));
        let message = failure.to_string();
        assert!(!message.contains("not an omorfos session"), "{message}");
    }

    /// A directory of data with the wallpapers that come with omorfos in
    /// it, or the dark one alone.
    fn data(both: bool) -> tempfile::TempDir {
        let directory = tempfile::TempDir::new().unwrap();
        let there = directory.path().join("backgrounds/omorfos");
        std::fs::create_dir_all(&there).unwrap();
        std::fs::write(there.join("prism-dark.png"), "").unwrap();
        if both {
            std::fs::write(there.join("prism-light.png"), "").unwrap();
        }
        directory
    }

    #[test]
    fn the_wallpapers_that_come_with_omorfos_are_found_where_data_is() {
        let (own, system, half) = (data(true), data(true), data(false));
        let found = |picture: Option<Picture>| picture.and_then(|picture| picture.path(None, true));
        let dark = |directory: &Path| Some(directory.join("backgrounds/omorfos/prism-dark.png"));
        // The user's own first, then the system's in order; a directory
        // with one of the two, or one not from the root, passed over.
        let dirs = env::join_paths([Path::new("relative"), half.path(), system.path()]).unwrap();
        let own_dir = Some(own.path().as_os_str());
        assert_eq!(
            found(installed((own_dir, Some(&dirs)), None)),
            dark(own.path())
        );
        assert_eq!(
            found(installed((None, Some(&dirs)), None)),
            dark(system.path())
        );
        // Without XDG_DATA_HOME, it is in the home directory.
        let user = tempfile::TempDir::new().unwrap();
        let local = user.path().join(".local/share");
        std::fs::create_dir_all(user.path().join(".local")).unwrap();
        std::fs::rename(own.path(), &local).unwrap();
        let home = Some(user.path().as_os_str());
        let in_home = installed((None, Some(OsStr::new("relative"))), home);
        assert_eq!(found(in_home), dark(&local));
        // Nowhere, nothing; and with XDG_DATA_DIRS empty, the system's own
        // places, which a test cannot count on.
        assert_eq!(installed((None, Some(half.path().as_os_str())), None), None);
        let _ = installed((None, Some(OsStr::new(""))), None);
    }
}
