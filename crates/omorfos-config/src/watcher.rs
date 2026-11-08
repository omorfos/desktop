//! Noticing when configuration files change.

use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, SystemTime};

use inotify::{EventMask, Inotify, WatchMask};
use rustix::event::epoll;
use rustix::io::Errno;
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

use crate::{FILES, Store};

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
///
/// It also wakes when it is told to, as though a file had changed: when the
/// scheme goes by the time of day, at the time it may change.
#[derive(Debug)]
pub struct Watcher {
    inotify: Inotify,
    buffer: Vec<u8>,
    /// Readable once the time it was told to wake at has come.
    timer: OwnedFd,
    /// Readable when either of them is.
    either: OwnedFd,
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
            let watched = inotify.watches().add(directory, mask);
            watched.and_then(|_| {
                // Woken by the time of day, and woken again if the clock is
                // set, as after a suspend, for the time to be asked anew.
                let flags = TimerfdFlags::NONBLOCK | TimerfdFlags::CLOEXEC;
                let timer = timerfd_create(TimerfdClockId::Realtime, flags);
                let either = epoll::create(epoll::CreateFlags::CLOEXEC);
                let both = timer.and_then(|timer| {
                    either.and_then(|either| {
                        let readable = epoll::EventFlags::IN;
                        let first = epoll::EventData::new_u64(0);
                        let second = epoll::EventData::new_u64(1);
                        let added = epoll::add(&either, &inotify, first, readable);
                        let added =
                            added.and_then(|()| epoll::add(&either, &timer, second, readable));
                        added.map(|()| (timer, either))
                    })
                });
                both.map_err(io::Error::from).map(|(timer, either)| Self {
                    inotify,
                    buffer,
                    timer,
                    either,
                })
            })
        })
    }

    /// Starts watching the directory `store` loads from, to wake when it is
    /// due to be loaded again as well as when a file in it changes.
    ///
    /// # Errors
    ///
    /// As [`Watcher::new`].
    pub fn of(store: &Store) -> io::Result<Self> {
        Self::new(store.directory()).map(|mut watcher| {
            watcher.wake_at(store.due());
            watcher
        })
    }

    /// Consumes what is pending, as [`Watcher::drain`] does, and if a file
    /// changed or `store` is due, loads it again and wakes when it is next
    /// due. Returns whether the configuration in effect changed.
    ///
    /// # Errors
    ///
    /// As [`Watcher::drain`].
    pub fn refresh(&mut self, store: &mut Store) -> io::Result<bool> {
        self.drain().map(|changed| {
            let reloaded = changed && store.reload();
            if changed {
                self.wake_at(store.due());
            }
            reloaded
        })
    }

    /// Has it wake at `at`, as though a file had changed, in place of any
    /// time it was told before; or not by the clock at all, if at none. A
    /// time the kernel cannot be told of is not waited for.
    pub fn wake_at(&mut self, at: Option<SystemTime>) {
        let since = at.map(|at| {
            let since = at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            // To the kernel no time at all is never.
            since.max(Duration::from_nanos(1))
        });
        let at = since.map_or(Timespec::default(), |since| Timespec {
            tv_sec: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(since.subsec_nanos()),
        });
        let when = Itimerspec {
            it_interval: Timespec::default(),
            it_value: at,
        };
        let flags = TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET;
        let _told = timerfd_settime(&self.timer, flags, &when);
    }

    /// Whether the time it was told to wake at has come, or the clock was
    /// set since, which is reason enough to ask the time again.
    fn rang(&self) -> bool {
        let mut count = [0; 8];
        let read = rustix::io::read(&self.timer, &mut count);
        read.map_or_else(|error| error != Errno::AGAIN, |_| true)
    }

    /// Consumes all pending events without blocking.
    ///
    /// Returns `true` if any of them concerned a configuration file, or it
    /// is the time it was told to wake at, meaning a reload is worthwhile.
    /// Changes to other files in the directory, such as editor swap files,
    /// are ignored.
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
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(changed | self.rang());
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl AsFd for Watcher {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.either.as_fd()
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
    use std::time::{Duration, SystemTime};

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

    /// Whether the watcher's descriptor becomes readable within `millis`.
    fn readable(watcher: &Watcher, millis: i32) -> bool {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        let mut sources = [PollFd::new(watcher, PollFlags::IN)];
        let wait = Timespec {
            tv_sec: i64::from(millis / 1000),
            tv_nsec: i64::from(millis % 1000) * 1_000_000,
        };
        poll(&mut sources, Some(&wait)).unwrap() > 0
    }

    #[test]
    fn the_descriptor_becomes_readable_when_something_changes() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        assert!(!readable(&watcher, 0));
        fs::write(directory.path().join("config.yaml"), "").unwrap();
        assert!(readable(&watcher, 100));
        assert!(changed(&mut watcher));
        assert!(!readable(&watcher, 0), "and is quiet once drained");
    }

    #[test]
    fn it_wakes_at_the_time_it_is_told_as_though_something_had_changed() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        let soon = SystemTime::now() + Duration::from_millis(30);
        watcher.wake_at(Some(soon));
        assert!(!changed(&mut watcher), "not yet");
        assert!(readable(&watcher, 1000));
        assert!(SystemTime::now() >= soon);
        assert!(changed(&mut watcher));
        assert!(!changed(&mut watcher), "once");
        assert!(!readable(&watcher, 0));
    }

    #[test]
    fn a_store_whose_scheme_goes_by_the_time_of_day_is_loaded_again_when_it_is_due() {
        let directory = TempDir::new().unwrap();
        let mut store = Store::open(directory.path());
        let mut watcher = Watcher::of(&store).unwrap();
        assert!(!readable(&watcher, 20), "nothing is due");
        assert!(!watcher.refresh(&mut store).unwrap());
        fs::write(
            directory.path().join("config.yaml"),
            "appearance: { scheme: auto }\n",
        )
        .unwrap();
        assert!(watcher.refresh(&mut store).unwrap(), "a file changed");
        assert!(store.due().is_some());
        assert!(!readable(&watcher, 20), "and it is woken when it is due");
        // Due at once, whatever it is next due at it is woken then.
        watcher.wake_at(Some(SystemTime::UNIX_EPOCH));
        assert!(!watcher.refresh(&mut store).unwrap(), "nothing changed");
        assert!(!readable(&watcher, 20));
        let watcher = Watcher::of(&store).unwrap();
        assert!(!readable(&watcher, 20));
    }

    #[test]
    fn a_time_gone_by_wakes_it_at_once_and_none_at_all_does_not() {
        let directory = TempDir::new().unwrap();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        watcher.wake_at(Some(SystemTime::UNIX_EPOCH));
        assert!(readable(&watcher, 100));
        assert!(changed(&mut watcher));
        let soon = SystemTime::now() + Duration::from_millis(20);
        watcher.wake_at(Some(soon));
        watcher.wake_at(None);
        assert!(!readable(&watcher, 60), "told of no time after all");
        assert!(!changed(&mut watcher));
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
