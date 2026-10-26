//! New versions of the programs that come with omorfos, heard of as they
//! are installed, so that each can be taken up without logging in again
//! (ADR 59).
//!
//! What is watched is the directory a program is in, not the program:
//! installing one writes a new file in the old one's place, which a watch
//! on the old would never hear of.

use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use inotify::{Inotify, WatchDescriptor, WatchMask};

/// How long after a program was last heard to change it is taken to be
/// installed: long enough for what writes it to have finished, and for the
/// rest of what is installed with it.
pub(crate) const SETTLE: Duration = Duration::from_secs(1);

/// The directories of the programs watched, and the programs heard to have
/// changed in them, each with when it last did.
#[derive(Debug)]
pub(crate) struct Versions {
    /// Absent where the kernel gives no more watches; then new versions
    /// are taken up at the next login, as they always were.
    inotify: Option<Inotify>,
    watched: Vec<(WatchDescriptor, PathBuf)>,
    heard: Vec<(PathBuf, Instant)>,
    buffer: Vec<u8>,
}

impl Versions {
    /// Hears of nothing yet.
    pub(crate) fn new() -> Self {
        Self {
            inotify: Inotify::init().ok(),
            watched: Vec::new(),
            heard: Vec::new(),
            buffer: vec![0; 4096],
        }
    }

    /// What is readable when something was heard, if anything can be.
    pub(crate) fn source(&self) -> Option<BorrowedFd<'_>> {
        self.inotify.as_ref().map(AsFd::as_fd)
    }

    /// Hears of `programs` changing, from now on, besides those already.
    /// One named without a directory, to be looked for where programs are
    /// looked for, is not: which it is depends on where it is looked.
    pub(crate) fn follow<'a>(&mut self, programs: impl Iterator<Item = &'a Path>) {
        let mask = WatchMask::CLOSE_WRITE | WatchMask::MOVED_TO | WatchMask::ONLYDIR;
        let places =
            programs.filter_map(|program| program.parent().filter(|_| program.is_absolute()));
        for place in places {
            let known = self.watched.iter().any(|(_, watched)| watched == place);
            let inotify = self.inotify.as_mut().filter(|_| !known);
            let added = inotify.and_then(|inotify| inotify.watches().add(place, mask).ok());
            self.watched
                .extend(added.map(|added| (added, place.to_owned())));
        }
    }

    /// Takes in what was heard by `now`: each file written, or put in place
    /// of another, in a directory watched.
    pub(crate) fn hear(&mut self, now: Instant) {
        let Some(inotify) = self.inotify.as_mut() else {
            return;
        };
        // Nothing more to read is the end of it, as is a failure to; what
        // was not read is heard of at the next login.
        while let Ok(events) = inotify.read_events(&mut self.buffer) {
            for event in events {
                let mut places = self.watched.iter();
                let place = places.find(|(watched, _)| *watched == event.wd);
                let file = place
                    .zip(event.name)
                    .map(|((_, place), name)| place.join(name));
                self.heard.retain(|(heard, _)| Some(heard) != file.as_ref());
                self.heard.extend(file.map(|file| (file, now)));
            }
        }
    }

    /// When what was heard is next to have settled.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.heard.iter().map(|(_, at)| *at + SETTLE).min()
    }

    /// The files that have settled by `now`, each once.
    pub(crate) fn settled(&mut self, now: Instant) -> Vec<PathBuf> {
        let settled = self.heard.extract_if(.., |(_, at)| *at + SETTLE <= now);
        settled.map(|(file, _)| file).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn a_program_written_or_put_in_place_is_heard_of_once_it_has_settled() {
        let place = tempfile::TempDir::new().unwrap();
        let program = place.path().join("omorfos-panel");
        fs::write(&program, "").unwrap();
        let mut versions = Versions::new();
        versions.follow([program.as_path(), Path::new("omorfos-panel")].into_iter());
        versions.follow([program.as_path()].into_iter());
        assert!(versions.source().is_some());
        let start = Instant::now();
        versions.hear(start);
        assert_eq!(versions.due(), None, "nothing yet");

        // Written, and put in place again a little later, as installing
        // does: heard of once, a moment after the last.
        fs::write(&program, "1").unwrap();
        versions.hear(start);
        assert_eq!(versions.due(), Some(start + SETTLE));
        let later = start + Duration::from_millis(500);
        let newer = place.path().join(".omorfos-panel.new");
        fs::write(&newer, "2").unwrap();
        fs::rename(&newer, &program).unwrap();
        versions.hear(later);
        assert_eq!(versions.due(), Some(later + SETTLE));
        let before = (later + SETTLE)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert_eq!(versions.settled(before), [] as [PathBuf; 0]);
        assert_eq!(versions.settled(later + SETTLE), [newer, program]);
        assert_eq!(versions.due(), None);
    }

    #[test]
    fn without_a_watch_nothing_is_heard() {
        let mut versions = Versions {
            inotify: None,
            ..Versions::new()
        };
        let place = tempfile::TempDir::new().unwrap();
        let program = place.path().join("omorfos-panel");
        versions.follow([program.as_path()].into_iter());
        fs::write(&program, "").unwrap();
        versions.hear(Instant::now());
        assert!(versions.source().is_none());
        assert_eq!(versions.due(), None);
    }
}
