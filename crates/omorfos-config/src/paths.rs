//! Locating the configuration directory, and the one for what is kept
//! only to save work.

use std::ffi::OsStr;
use std::path::PathBuf;

/// Resolves the configuration directory from the environment.
///
/// Follows the XDG Base Directory specification: `$XDG_CONFIG_HOME/omorfos`,
/// falling back to `$HOME/.config/omorfos`. Relative paths are ignored, as
/// the specification requires. Returns `None` when neither variable holds an
/// absolute path.
///
/// The variables are passed in rather than read here so the lookup stays
/// pure; callers supply `std::env::var_os` results.
#[must_use]
pub fn config_dir(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let absolute =
        |value: Option<&OsStr>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_config_home)
        .or_else(|| absolute(home).map(|home| home.join(".config")))
        .map(|base| base.join("omorfos"))
}

/// Resolves the directory for what is kept only to save work, which may
/// be deleted at any time, from the environment.
///
/// As [`config_dir`] does, following the XDG Base Directory specification:
/// `$XDG_CACHE_HOME/omorfos`, falling back to `$HOME/.cache/omorfos`.
#[must_use]
pub fn cache_dir(xdg_cache_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let absolute =
        |value: Option<&OsStr>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_cache_home)
        .or_else(|| absolute(home).map(|home| home.join(".cache")))
        .map(|base| base.join("omorfos"))
}

/// Resolves the directory a copy of each configuration file that loaded
/// is kept in ([`crate::Store::keep_in`]): `$XDG_RUNTIME_DIR/omorfos/last-good`,
/// which is the user's alone and is emptied when they log out. `None`
/// unless the variable holds an absolute path; nothing is kept then.
#[must_use]
pub fn kept_dir(xdg_runtime_dir: Option<&OsStr>) -> Option<PathBuf> {
    let runtime = xdg_runtime_dir.map(PathBuf::from);
    let runtime = runtime.filter(|path| path.is_absolute());
    runtime.map(|runtime| runtime.join("omorfos").join("last-good"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        config_dir(xdg.map(OsStr::new), home.map(OsStr::new))
    }

    #[test]
    fn prefers_xdg_config_home() {
        assert_eq!(
            resolve(Some("/xdg"), Some("/home/me")),
            Some(PathBuf::from("/xdg/omorfos"))
        );
    }

    #[test]
    fn falls_back_to_dot_config_in_home() {
        let expected = Some(PathBuf::from("/home/me/.config/omorfos"));
        assert_eq!(resolve(None, Some("/home/me")), expected);
    }

    #[test]
    fn ignores_a_relative_xdg_config_home() {
        let expected = Some(PathBuf::from("/home/me/.config/omorfos"));
        assert_eq!(resolve(Some("relative"), Some("/home/me")), expected);
    }

    #[test]
    fn is_none_without_an_absolute_base() {
        assert_eq!(resolve(None, None), None);
        assert_eq!(resolve(Some("relative"), Some("also/relative")), None);
    }

    #[test]
    fn what_last_loaded_is_kept_in_the_runtime_directory_if_it_is_absolute() {
        let kept = |runtime: Option<&str>| kept_dir(runtime.map(OsStr::new));
        let expected = PathBuf::from("/run/user/1000/omorfos/last-good");
        assert_eq!(kept(Some("/run/user/1000")), Some(expected));
        assert_eq!(kept(Some("relative")), None);
        assert_eq!(kept(None), None);
    }

    #[test]
    fn what_saves_work_is_kept_in_the_cache_directory() {
        let cache = |xdg: Option<&str>, home: Option<&str>| {
            cache_dir(xdg.map(OsStr::new), home.map(OsStr::new))
        };
        assert_eq!(
            cache(Some("/xdg"), Some("/home/me")),
            Some(PathBuf::from("/xdg/omorfos"))
        );
        assert_eq!(
            cache(Some("relative"), Some("/home/me")),
            Some(PathBuf::from("/home/me/.cache/omorfos"))
        );
        assert_eq!(cache(None, Some("relative")), None);
    }
}
