//! Where what a session says is kept: everything the holder, the relay,
//! the compositor and what they start write, in a file of the user's for
//! this login and another for the one before, wherever the login put the
//! holder's own output. The holder writes it, and `omorfosctl report`
//! reads it.

use std::ffi::OsStr;
use std::path::PathBuf;

/// This login's log.
pub const NAME: &str = "session.log";

/// The login before's.
pub const BEFORE: &str = "session.log.old";

/// The directory the logs are kept in, from the environment, as the XDG
/// Base Directory specification has it: `$XDG_STATE_HOME/omorfos`,
/// falling back to `$HOME/.local/state/omorfos`, a relative path being
/// passed over. Nothing, where neither gives an absolute path.
#[must_use]
pub fn directory(xdg_state_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let absolute =
        |value: Option<&OsStr>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_state_home)
        .or_else(|| absolute(home).map(|home| home.join(".local/state")))
        .map(|base| base.join("omorfos"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(state: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        directory(state.map(OsStr::new), home.map(OsStr::new))
    }

    #[test]
    fn the_logs_are_kept_in_the_state_directory_or_else_the_home_ones() {
        let state = resolved(Some("/state"), Some("/home/me"));
        assert_eq!(state, Some(PathBuf::from("/state/omorfos")));
        let home = resolved(Some("state"), Some("/home/me"));
        assert_eq!(home, Some(PathBuf::from("/home/me/.local/state/omorfos")));
        assert_eq!(resolved(None, Some("me")), None, "nowhere absolute");
    }
}
