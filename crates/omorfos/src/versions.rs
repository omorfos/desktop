//! New versions of the programs that come with omorfos, heard of as they
//! are installed, so that each can be taken up without logging in again
//! (ADR 59).
//!
//! What is watched is the directory a program is in, not the program:
//! installing one writes a new file in the old one's place, which a watch
//! on the old would never hear of.
//!
//! The compositor's own new version is taken up by its stepping down to be
//! started again, its programs kept by the relay, at a moment the session
//! is not in use (ADR 60).

use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use inotify::{Inotify, WatchDescriptor, WatchMask};

/// How long after a program was last heard to change it is taken to be
/// installed: long enough for what writes it to have finished, and for the
/// rest of what is installed with it.
pub(crate) const SETTLE: Duration = Duration::from_secs(1);

/// How long the user is to have given no input before the compositor's own
/// new version is taken up: long enough not to fall between two keys, and
/// short enough to come soon after the user has stopped.
pub(crate) const STILL: Duration = Duration::from_secs(5);

/// What keeps the session in use, for the compositor's own new version to
/// wait.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a way of being in use of its own, and none depends on another"
)]
pub(crate) struct InUse {
    /// A window is fullscreen, as a film or a game is.
    pub(crate) fullscreen: bool,
    /// Something records, or has a screen shared with it, as a call does.
    pub(crate) recording: bool,
    /// Something keeps the session awake, or from sleeping, as what plays
    /// does.
    pub(crate) playing: bool,
    /// What would go with the compositor is open: the sealed prompt, with
    /// what is typed into it, or the launcher.
    pub(crate) open: bool,
    /// When the user last gave input, if they have.
    pub(crate) input: Option<Instant>,
    /// A window of a program written for X11 is open, and the program would
    /// close with its X server.
    pub(crate) x11: bool,
    /// The screens are off, as the user is away, and the next compositor
    /// would turn them on.
    pub(crate) dark: bool,
}

/// What is to become of the compositor's own new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Own {
    /// It waits, as the session is in use.
    Waits,
    /// It waits for the programs written for X11 to close, as it would
    /// otherwise be taken up.
    WaitsForX11,
    /// It is taken up: at a moment the session is not in use, or
    /// `regardless` of that, having waited as long as it may.
    TakenUp {
        /// Whether the session is in use.
        regardless: bool,
    },
}

impl InUse {
    /// What is to become of the compositor's own new version, heard of
    /// `since`, as of `now`, having waited for `wait` at most, or however
    /// long without one; and for as long as the screens are off.
    pub(crate) fn own(self, since: Instant, wait: Option<Duration>, now: Instant) -> Own {
        if self.dark {
            return Own::Waits;
        }
        let still = self.input.is_none_or(|input| input + STILL <= now);
        let busy = self.fullscreen || self.recording || self.playing || self.open;
        let quiet = still && !busy;
        let over = wait.is_some_and(|wait| since + wait <= now);
        match (quiet || over, self.x11) {
            (false, _) => Own::Waits,
            (true, true) => Own::WaitsForX11,
            (true, false) => Own::TakenUp { regardless: !quiet },
        }
    }

    /// When what [`InUse::own`] says may next change by the time alone,
    /// after `now`, if it may.
    pub(crate) fn next(
        self,
        since: Instant,
        wait: Option<Duration>,
        now: Instant,
    ) -> Option<Instant> {
        if self.dark {
            return None;
        }
        let still = self.input.map(|input| input + STILL);
        let over = wait.map(|wait| since + wait);
        [still, over]
            .into_iter()
            .flatten()
            .filter(|at| *at > now)
            .min()
    }
}

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
    fn the_compositors_new_version_is_taken_up_once_the_user_has_been_still_a_while() {
        let start = Instant::now();
        let wait = Some(Duration::from_secs(60));
        let typing = InUse {
            input: Some(start),
            ..InUse::default()
        };
        assert_eq!(typing.own(start, wait, start + STILL / 2), Own::Waits);
        assert_eq!(typing.next(start, wait, start), Some(start + STILL));
        let still = Own::TakenUp { regardless: false };
        assert_eq!(typing.own(start, wait, start + STILL), still);
        assert_eq!(
            InUse::default().own(start, wait, start),
            still,
            "no input yet"
        );
    }

    #[test]
    fn the_compositors_new_version_waits_while_the_session_is_in_use_as_long_as_it_may() {
        let start = Instant::now();
        let wait = Duration::from_secs(60);
        let busy = [
            InUse {
                fullscreen: true,
                ..InUse::default()
            },
            InUse {
                recording: true,
                ..InUse::default()
            },
            InUse {
                playing: true,
                ..InUse::default()
            },
            InUse {
                open: true,
                ..InUse::default()
            },
        ];
        let later = start + Duration::from_secs(600);
        for in_use in busy {
            assert_eq!(in_use.own(start, Some(wait), start + wait / 2), Own::Waits);
            assert_eq!(in_use.next(start, Some(wait), start), Some(start + wait));
            let regardless = Own::TakenUp { regardless: true };
            assert_eq!(in_use.own(start, Some(wait), start + wait), regardless);
            assert_eq!(in_use.own(start, None, later), Own::Waits, "for ever");
            assert_eq!(in_use.next(start, None, start), None);
        }
    }

    #[test]
    fn the_compositors_new_version_waits_for_the_programs_written_for_x11_however_long() {
        let start = Instant::now();
        let wait = Some(Duration::from_secs(60));
        let x11 = InUse {
            x11: true,
            input: Some(start),
            ..InUse::default()
        };
        assert_eq!(x11.own(start, wait, start), Own::Waits, "in use");
        assert_eq!(x11.own(start, wait, start + STILL), Own::WaitsForX11);
        let later = start + Duration::from_secs(600);
        assert_eq!(x11.own(start, wait, later), Own::WaitsForX11);
        assert_eq!(
            x11.next(start, wait, later),
            None,
            "nothing more to wait for"
        );
    }

    #[test]
    fn the_compositors_new_version_waits_however_long_while_the_screens_are_off() {
        let start = Instant::now();
        let wait = Some(Duration::from_secs(60));
        let dark = InUse {
            dark: true,
            ..InUse::default()
        };
        let later = start + Duration::from_secs(600);
        assert_eq!(dark.own(start, wait, later), Own::Waits);
        assert_eq!(
            dark.next(start, wait, start),
            None,
            "until the user is back"
        );
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
