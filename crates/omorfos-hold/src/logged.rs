//! What the session says kept in a file of the user's, for this login and
//! the one before (`omorfos_ipc::session_log`), wherever the login put the
//! holder's own output; and kept from growing without end.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::fs::{DirBuilderExt, FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use omorfos_ipc::session_log::{BEFORE, NAME};
use rustix::fs::{fstat, ftruncate, stat};
use rustix::io::write;

/// How large a login's log may grow, in bytes, before what came first
/// is let go. A login of weeks with a program that complains every second
/// stays within it.
pub const BOUND: u64 = 8 * 1024 * 1024;

/// How much of the end of a log past its bound is kept, in bytes: much
/// more than a report puts in.
const KEPT: u64 = 2 * 1024 * 1024;

/// What a log that was cut begins with.
const LEFT_OUT: &str = "omorfos-hold: what came before is left out, as the log grew too large";

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

/// Keeps `log`, what the session writes to, within [`BOUND`] if it is the
/// log at `path`: cut, where it has grown past it, to its last 2 MiB
/// from the start of a line, saying so. Returns whether it was.
///
/// Everything the session starts writes to the end of the log itself, not
/// through the holder, so that a holder that is busy never holds a program
/// up; so the log can only be cut where it is, and a line written between
/// its end being read and its being cut is lost, and one written between
/// its being cut and its end being written back goes before that end.
#[must_use]
pub fn bound(log: BorrowedFd<'_>, path: &Path) -> bool {
    let (written, there) = (fstat(log).ok(), stat(path).ok());
    let same = written.zip(there).filter(|(written, there)| {
        (written.st_dev, written.st_ino) == (there.st_dev, there.st_ino)
    });
    let size = same.and_then(|(written, _)| u64::try_from(written.st_size).ok());
    let Some(size) = size.filter(|size| *size > BOUND) else {
        return false;
    };
    // Read where it is, as the session writes to it only.
    let mut end = vec![0; usize::try_from(KEPT).unwrap_or(usize::MAX)];
    let read = File::open(path).and_then(|file| file.read_at(&mut end, size - KEPT));
    let end = read.ok().map(|read| &end[..read]);
    let lines = end.map(|end| {
        let begun = end.iter().position(|byte| *byte == b'\n');
        let lines = begun.map_or(&end[end.len()..], |begun| &end[begun + 1..]);
        [LEFT_OUT.as_bytes(), b"\n", lines].concat()
    });
    // The log is written to its end, wherever that is: after it is cut,
    // to its beginning.
    lines.is_some_and(|lines| {
        ftruncate(log, 0).is_ok() && write(log, &lines).is_ok_and(|wrote| wrote == lines.len())
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A log of `lines` numbered lines, begun as the holder begins one.
    fn log_of(lines: usize) -> (tempfile::TempDir, File, PathBuf) {
        let directory = tempfile::TempDir::new().unwrap();
        let (mut file, path) = begin(directory.path()).unwrap();
        for line in 0..lines {
            writeln!(file, "line {line:08} of what the session said").unwrap();
        }
        (directory, file, path)
    }

    /// How many lines are written past the bound.
    fn past_the_bound() -> usize {
        usize::try_from(BOUND).unwrap() / 36 + 100
    }

    #[test]
    fn a_log_past_its_bound_keeps_its_end_from_the_start_of_a_line() {
        let (_directory, mut file, path) = log_of(past_the_bound());
        assert!(bound(file.as_fd(), &path));
        let kept = fs::read_to_string(&path).unwrap();
        let (said, rest) = kept.split_once('\n').unwrap();
        assert_eq!(said, LEFT_OUT);
        assert!(rest.starts_with("line "), "{rest:.80}");
        let last = past_the_bound() - 1;
        assert!(rest.ends_with(&format!("line {last:08} of what the session said\n")));
        assert!(kept.len() <= usize::try_from(KEPT).unwrap() + LEFT_OUT.len() + 1);
        // What is written after goes after it.
        writeln!(file, "and then").unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .ends_with("said\nand then\n")
        );
    }

    #[test]
    fn a_log_within_its_bound_is_left_as_it_is() {
        let (_directory, file, path) = log_of(10);
        let before = fs::read_to_string(&path).unwrap();
        assert!(!bound(file.as_fd(), &path));
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn only_the_log_itself_is_bounded() {
        let (directory, file, path) = log_of(past_the_bound());
        let size = fs::metadata(&path).unwrap().len();
        // Output that went elsewhere, as where the login put it, is not
        // the holder's to cut.
        let elsewhere = directory.path().join("elsewhere");
        fs::write(&elsewhere, "").unwrap();
        assert!(!bound(file.as_fd(), &elsewhere));
        assert!(!bound(file.as_fd(), &directory.path().join("gone")));
        // Nor is one it cannot cut, or read the end of.
        let reading = File::open(&path).unwrap();
        assert!(!bound(reading.as_fd(), &path));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o200)).unwrap();
        assert!(!bound(file.as_fd(), &path));
        assert_eq!(fs::metadata(&path).unwrap().len(), size);
    }
}
