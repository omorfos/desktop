//! Locating the configuration directory.

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
}
