//! What the session says kept in a file of the user's, for this login and
//! the one before (`omorfos_ipc::session_log`), wherever the login put the
//! holder's own output.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use omorfos_ipc::session_log::{BEFORE, NAME};

/// Begins this login's log in `directory`, keeping what the login before
/// said beside it and letting anything older go: the directory and the
/// log are the user's alone, as what programs write may say what they
/// are doing. Returns the log, open to be written to the end of, and where
/// it is.
///
/// # Errors
///
/// Returns an error if the directory cannot be made, or the log opened.
pub fn begin(directory: &Path) -> io::Result<(File, PathBuf)> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    let log = directory.join(NAME);
    // There is none before the first login.
    let _ = fs::rename(&log, directory.join(BEFORE));
    let opened = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log);
    opened.map(|file| (file, log))
}
