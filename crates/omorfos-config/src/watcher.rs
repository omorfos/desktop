//! Noticing when configuration files change.

use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;

use inotify::{EventMask, Inotify, WatchMask};

use crate::FILES;

/// Watches the configuration directory for changes to its files.
///
/// The watcher is a file descriptor that becomes readable when something in
/// the directory changes. Register [`Watcher::as_fd`] with an event loop and
/// call [`Watcher::drain`] when it is readable; nothing here blocks or spawns
/// a thread.
///
/// The directory is watched rather than the files, because editors usually
/// save by writing a temporary file and renaming it over the original, which
/// replaces the file a per-file watch would be attached to.
///
/// A file that is a symbolic link is only seen to change when the link
/// itself is replaced, not when its target is edited in place.
#[derive(Debug)]
pub struct Watcher {
    inotify: Inotify,
    buffer: Vec<u8>,
}

impl Watcher {
    /// Starts watching `directory`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory does not exist or cannot be watched,
    /// for example because the user's inotify watch limit is exhausted.
    pub fn new(directory: &Path) -> io::Result<Self> {
        Self::with_buffer(directory, vec![0; 4096])
    }

    fn with_buffer(directory: &Path, buffer: Vec<u8>) -> io::Result<Self> {
        let mask = WatchMask::CLOSE_WRITE
            | WatchMask::CREATE
            | WatchMask::DELETE
            | WatchMask::MOVED_FROM
            | WatchMask::MOVED_TO
            | WatchMask::ONLYDIR;
        Inotify::init().and_then(|inotify| {
            inotify
                .watches()
                .add(directory, mask)
                .map(|_| Self { inotify, buffer })
        })
    }

    /// Consumes all pending events without blocking.
    ///
    /// Returns `true` if any of them concerned a configuration file, meaning
    /// a reload is worthwhile. Changes to other files in the directory, such
    /// as editor swap files, are ignored.
    ///
    /// # Errors
    ///
    /// Returns an error if the events cannot be read.
    pub fn drain(&mut self) -> io::Result<bool> {
        let mut changed = false;
        loop {
            match self.inotify.read_events(&mut self.buffer) {
                Ok(events) => {
                    for event in events {
                        changed |= is_relevant(event.mask, event.name);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(changed),
                Err(error) => return Err(error),
            }
        }
    }
}

impl AsFd for Watcher {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inotify.as_fd()
    }
}

/// Whether an event means the configuration may have changed.
fn is_relevant(mask: EventMask, name: Option<&OsStr>) -> bool {
    // After an overflow the kernel has dropped events, so assume the worst.
    mask.contains(EventMask::Q_OVERFLOW)
        || name.is_some_and(|name| FILES.iter().any(|file| name == OsStr::new(file)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    /// Inotify queues events before the calls that cause them return, so a
    /// single drain sees everything done so far.
    fn changed(watcher: &mut Watcher) -> bool {
        watcher.drain().unwrap()
    }

    #[test]
    fn a_quiet_directory_reports_no_change() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        assert!(!changed(&mut watcher));
    }

    #[test]
    fn writing_a_configuration_file_is_a_change() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        fs::write(directory.path().join("keys.yaml"), "bindings: []\n").unwrap();
        assert!(changed(&mut watcher));
        assert!(!changed(&mut watcher), "events are consumed by draining");
    }

    #[test]
    fn saving_by_renaming_a_temporary_file_is_a_change() {
        let directory = TempDir::new().unwrap();
        let temporary = directory.path().join(".theme.css.tmp");
        let mut watcher = Watcher::new(directory.path()).unwrap();

        fs::write(&temporary, "").unwrap();
        assert!(
            !changed(&mut watcher),
            "the temporary file is not configuration"
        );

        fs::rename(&temporary, directory.path().join("theme.css")).unwrap();
        assert!(changed(&mut watcher));
    }

    #[test]
    fn deleting_a_configuration_file_is_a_change() {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("menu.yaml"), "").unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        fs::remove_file(directory.path().join("menu.yaml")).unwrap();
        assert!(changed(&mut watcher));
    }

    #[test]
    fn unrelated_files_are_ignored() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        fs::write(directory.path().join("notes.txt"), "").unwrap();
        fs::write(directory.path().join("config.yaml.bak"), "").unwrap();
        assert!(!changed(&mut watcher));
    }

    #[test]
    fn more_events_than_fit_in_the_buffer_are_all_drained() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        for index in 0..500 {
            fs::write(directory.path().join(format!("unrelated-{index}")), "").unwrap();
        }
        fs::write(directory.path().join("rules.yaml"), "").unwrap();
        assert!(changed(&mut watcher));
        assert!(!changed(&mut watcher));
    }

    #[test]
    fn a_queue_overflow_counts_as_a_change() {
        assert!(is_relevant(EventMask::Q_OVERFLOW, None));
        assert!(!is_relevant(EventMask::IGNORED, None));
    }

    #[test]
    fn the_descriptor_becomes_readable_when_something_changes() {
        let directory = TempDir::new().unwrap();
        let watcher = Watcher::new(directory.path()).unwrap();
        let mut reader = fs::File::from(watcher.as_fd().try_clone_to_owned().unwrap());
        fs::write(directory.path().join("config.yaml"), "").unwrap();
        sleep(Duration::from_millis(10));
        let mut buffer = [0; 1024];
        assert!(io::Read::read(&mut reader, &mut buffer).unwrap() > 0);
    }

    #[test]
    fn watching_a_missing_directory_fails() {
        let directory = TempDir::new().unwrap();
        let error = Watcher::new(&directory.path().join("missing")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn watching_a_file_instead_of_a_directory_fails() {
        let directory = TempDir::new().unwrap();
        let file = directory.path().join("file");
        fs::write(&file, "").unwrap();
        assert!(Watcher::new(&file).is_err());
    }

    #[test]
    fn a_failed_read_is_reported() {
        let directory = TempDir::new().unwrap();
        // A buffer too small for even one event makes the kernel refuse the
        // read, which stands in for any other read failure.
        let mut watcher = Watcher::with_buffer(directory.path(), vec![0; 1]).unwrap();
        fs::write(directory.path().join("config.yaml"), "").unwrap();
        assert!(watcher.drain().is_err());
    }
}
