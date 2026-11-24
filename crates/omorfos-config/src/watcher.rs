//! Noticing when configuration files change.

use std::ffi::OsStr;
use std::io;
use std::mem::replace;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use rustix::event::epoll;
use rustix::io::Errno;
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

use crate::{FILES, Store};

/// How long the watcher waits before it has a directory whose path leads
/// nowhere looked for again, where nothing would say it leads somewhere.
const LOOKS_AGAIN: Duration = Duration::from_secs(1);

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
/// The directory itself may be taken away and put back, as a backup is
/// restored, moved aside, or be a link pointed elsewhere, as home-manager
/// does, or one to a directory that is itself taken away or moved and made
/// again: the directory it is in is watched for its name, and it is
/// watched again wherever its path then leads. Its going is a change, and
/// what it became, if it was moved, is not watched; while its path leads
/// nowhere it is tried again at each [`Watcher::drain`], and its coming
/// back is a change too. Where nothing would say that it has come back,
/// as nothing does of a link, whose name stays as it was, the watcher
/// becomes readable of itself every second meanwhile, so that what sleeps
/// until it is drains it, and so looks again.
///
/// The directory above it being replaced, as `~/.config` moved aside and
/// put back, is not seen: both are still watched where they were.
///
/// It also wakes when it is told to, as though a file had changed: when the
/// scheme goes by the time of day, at the time it may change.
#[derive(Debug)]
pub struct Watcher {
    inotify: Inotify,
    buffer: Vec<u8>,
    /// The directory, and what watches it, made again when it comes back:
    /// none while its path leads nowhere.
    directory: PathBuf,
    watched: Option<WatchDescriptor>,
    /// What watches the directory it is in, for its name coming and going,
    /// if that may be watched.
    parent: Option<WatchDescriptor>,
    /// Readable once the time it was told to wake at has come.
    timer: OwnedFd,
    /// Readable once it is time to look again for a directory whose path
    /// leads nowhere, and how long it waits before that.
    looking: OwnedFd,
    looks_again: Duration,
    /// Readable when any of them is.
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
        // By its name, not through `..`, which a link would lead elsewhere.
        let parent = directory.parent();
        let parent = parent.filter(|parent| !parent.as_os_str().is_empty());
        let parent = parent.unwrap_or(Path::new("."));
        let named = WatchMask::CREATE
            | WatchMask::DELETE
            | WatchMask::MOVED_FROM
            | WatchMask::MOVED_TO
            | WatchMask::ONLYDIR;
        Inotify::init().and_then(|inotify| {
            let watched = inotify.watches().add(directory, mask());
            watched.and_then(|watched| {
                let parent = inotify.watches().add(parent, named).ok();
                // Woken by the time of day, and woken again if the clock is
                // set, as after a suspend, for the time to be asked anew.
                let flags = TimerfdFlags::NONBLOCK | TimerfdFlags::CLOEXEC;
                let timer = timerfd_create(TimerfdClockId::Realtime, flags);
                // Looked for again by how long has passed, whatever the
                // clock on the wall is set to meanwhile.
                let looking = timerfd_create(TimerfdClockId::Monotonic, flags);
                let timers = timer.and_then(|timer| looking.map(|looking| (timer, looking)));
                let either = epoll::create(epoll::CreateFlags::CLOEXEC);
                let all = timers.and_then(|(timer, looking)| {
                    either.and_then(|either| {
                        let readable = epoll::EventFlags::IN;
                        let numbered = epoll::EventData::new_u64;
                        let added = epoll::add(&either, &inotify, numbered(0), readable);
                        let added =
                            added.and_then(|()| epoll::add(&either, &timer, numbered(1), readable));
                        let added = added
                            .and_then(|()| epoll::add(&either, &looking, numbered(2), readable));
                        added.map(|()| (timer, looking, either))
                    })
                });
                all.map_err(io::Error::from)
                    .map(|(timer, looking, either)| Self {
                        inotify,
                        buffer,
                        directory: directory.to_owned(),
                        watched: Some(watched),
                        parent,
                        timer,
                        looking,
                        looks_again: LOOKS_AGAIN,
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
    /// Returns `true` if any of them concerned a configuration file, or
    /// the directory itself came, went or was moved, or it came back after
    /// its path led nowhere, or it is the time it was told to wake at,
    /// meaning a reload is worthwhile. Changes to other files in the
    /// directory, such as editor swap files, are ignored, and so is its
    /// having been time to look for the directory again where it was not
    /// found.
    ///
    /// # Errors
    ///
    /// Returns an error if the events cannot be read.
    pub fn drain(&mut self) -> io::Result<bool> {
        let mut changed = false;
        let mut again = false;
        loop {
            match self.inotify.read_events(&mut self.buffer) {
                Ok(events) => {
                    for event in events {
                        let parent = self.parent.as_ref() == Some(&event.wd);
                        // The directory's own name come or gone where it is.
                        again |= parent && event.name == self.directory.file_name();
                        // The directory gone, or moved, and its watch with it.
                        let gone = EventMask::IGNORED | EventMask::MOVE_SELF;
                        let own = self.watched.as_ref() == Some(&event.wd);
                        again |= own && event.mask.intersects(gone);
                        // Its going may be among what the kernel dropped.
                        again |= event.mask.contains(EventMask::Q_OVERFLOW);
                        changed |= !parent && is_relevant(event.mask, event.name);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // Tried again until its path leads somewhere, as nothing
                    // may say when it does: a link's name stays as it was.
                    let lost = self.watched.is_none();
                    let back = (again || lost) && self.watch_again();
                    // Only then: a turn with nothing changed asks the
                    // kernel nothing more for it.
                    if again || lost {
                        self.look_again();
                    }
                    return Ok(changed | again | back | self.rang());
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Watches the directory again, wherever its path now leads, and lets
    /// go of what watched it before if that was another, or if it leads
    /// nowhere: what was moved aside is no longer it. Returns whether it is
    /// there to be watched, and so is to be loaded again.
    fn watch_again(&mut self) -> bool {
        let watched = self.inotify.watches().add(&self.directory, mask()).ok();
        let before = replace(&mut self.watched, watched);
        let other = before.filter(|before| self.watched.as_ref() != Some(before));
        let _ = other.map(|before| self.inotify.watches().remove(before));
        self.watched.is_some()
    }

    /// Has the watcher become readable in a while, for the directory to be
    /// looked for again by whoever drains it then, if its path leads
    /// nowhere and nothing would say when it leads somewhere: its name is
    /// a link's, which stays as it was while what it leads to comes back,
    /// or the directory it is in is not watched. Not otherwise, nor once it
    /// is found: a name that comes back where it is watched for is heard
    /// of, and nothing wakes to look for it.
    fn look_again(&mut self) {
        // Whether or not it was time yet, it has just been looked for.
        let mut count = [0; 8];
        let _rang = rustix::io::read(&self.looking, &mut count);
        let named = std::fs::symlink_metadata(&self.directory);
        let link = named.is_ok_and(|named| named.is_symlink());
        let unheard = self.parent.is_none() | link;
        let wait = (self.watched.is_none() & unheard).then_some(self.looks_again);
        // To the kernel no time at all is not to wake.
        let wait = wait.unwrap_or_default();
        let when = Itimerspec {
            it_interval: Timespec::default(),
            it_value: Timespec {
                tv_sec: i64::try_from(wait.as_secs()).unwrap_or(i64::MAX),
                tv_nsec: i64::from(wait.subsec_nanos()),
            },
        };
        let _told = timerfd_settime(&self.looking, TimerfdTimerFlags::empty(), &when);
    }
}

/// What the directory is watched for: its files written, made, taken away
/// or renamed, and itself moved.
fn mask() -> WatchMask {
    WatchMask::CLOSE_WRITE
        | WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
        | WatchMask::MOVE_SELF
        | WatchMask::ONLYDIR
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
    fn a_directory_taken_away_and_put_back_is_watched_again() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("config.yaml"), "").unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();

        // Taken away, as `rm -r` does: its file went with it.
        fs::remove_dir_all(&directory).unwrap();
        assert!(changed(&mut watcher));
        // Put back, as a backup is restored.
        fs::create_dir(&directory).unwrap();
        assert!(changed(&mut watcher), "put back, it is to be loaded");
        assert!(!changed(&mut watcher));
        fs::write(directory.join("theme.css"), "").unwrap();
        assert!(changed(&mut watcher), "and is watched again");
    }

    #[test]
    fn a_directory_that_is_a_link_pointed_elsewhere_is_watched_where_it_now_leads() {
        let parent = TempDir::new().unwrap();
        let before = parent.path().join("before");
        let after = parent.path().join("after");
        fs::create_dir(&before).unwrap();
        fs::create_dir(&after).unwrap();
        let directory = parent.path().join("omorfos");
        std::os::unix::fs::symlink(&before, &directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();

        // Pointed elsewhere as `ln -sfn` does: a new link renamed over it.
        let new = parent.path().join("omorfos.new");
        std::os::unix::fs::symlink(&after, &new).unwrap();
        fs::rename(&new, &directory).unwrap();
        assert!(changed(&mut watcher), "it leads elsewhere, to be loaded");
        fs::write(after.join("keys.yaml"), "").unwrap();
        assert!(changed(&mut watcher), "where it leads now is watched");
        fs::write(before.join("keys.yaml"), "").unwrap();
        assert!(!changed(&mut watcher), "and where it led is not");
    }

    #[test]
    fn a_directory_that_is_a_link_whose_directory_is_made_again_is_watched_again() {
        let parent = TempDir::new().unwrap();
        // Elsewhere, as in a checkout of dotfiles: nothing of it is heard
        // by what watches the directory the link is in.
        let elsewhere = TempDir::new().unwrap();
        let target = elsewhere.path().join("omorfos");
        fs::create_dir(&target).unwrap();
        let directory = parent.path().join("omorfos");
        std::os::unix::fs::symlink(&target, &directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();

        fs::remove_dir(&target).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        assert!(!changed(&mut watcher), "and still gone");
        fs::create_dir(&target).unwrap();
        assert!(changed(&mut watcher), "made again, it is to be loaded");
        assert!(!changed(&mut watcher));
        fs::write(target.join("config.yaml"), "").unwrap();
        assert!(changed(&mut watcher), "and is watched again");
    }

    /// A link, in a directory of its own, to a directory elsewhere, as in
    /// a checkout of dotfiles, and a watcher of it that looks again soon.
    fn linked() -> (TempDir, TempDir, PathBuf, Watcher) {
        let parent = TempDir::new().unwrap();
        let elsewhere = TempDir::new().unwrap();
        let target = elsewhere.path().join("omorfos");
        fs::create_dir(&target).unwrap();
        let directory = parent.path().join("omorfos");
        std::os::unix::fs::symlink(&target, &directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();
        watcher.looks_again = Duration::from_millis(20);
        (parent, elsewhere, target, watcher)
    }

    #[test]
    fn a_link_whose_directory_has_gone_is_looked_for_again_without_being_drained() {
        let (_parent, _elsewhere, target, mut watcher) = linked();

        fs::remove_dir(&target).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        // Nothing says when a link leads somewhere again, so what sleeps
        // until the watcher can be read is woken to look.
        assert!(readable(&watcher, 5000), "woken to look again");
        assert!(!changed(&mut watcher), "still gone, which is no change");
        fs::create_dir(&target).unwrap();
        assert!(readable(&watcher, 5000), "and woken again");
        assert!(changed(&mut watcher), "made again, it is to be loaded");
        // Found, it is looked for no longer, and heard of as it was.
        assert!(!readable(&watcher, 200), "nothing wakes for it now");
        fs::write(target.join("config.yaml"), "").unwrap();
        assert!(readable(&watcher, 5000));
        assert!(changed(&mut watcher), "and it is watched again");
    }

    #[test]
    fn a_directory_gone_whose_name_would_be_heard_of_is_not_looked_for_again() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        fs::create_dir(&directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();
        watcher.looks_again = Duration::from_millis(20);

        // The directory it is in says when the name is back: nothing wakes
        // to look meanwhile, however long there is no configuration.
        fs::remove_dir(&directory).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        assert!(!readable(&watcher, 200), "nothing wakes for it");
        // Nor for a file put where it was, which is no link.
        fs::write(&directory, "").unwrap();
        assert!(changed(&mut watcher));
        assert!(!readable(&watcher, 200), "nor for what is no link");
        fs::remove_file(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        assert!(readable(&watcher, 5000), "its name is heard of");
        assert!(changed(&mut watcher), "put back, it is to be loaded");
    }

    #[test]
    fn a_directory_gone_from_one_that_is_not_watched_is_looked_for_again() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        fs::create_dir(&directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();
        watcher.looks_again = Duration::from_millis(20);
        // As where the directory it is in may not be read.
        let above = watcher.parent.take().unwrap();
        watcher.inotify.watches().remove(above).unwrap();

        fs::remove_dir(&directory).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        assert!(readable(&watcher, 5000), "woken to look again");
        assert!(!changed(&mut watcher), "still gone");
        fs::create_dir(&directory).unwrap();
        assert!(readable(&watcher, 5000), "and woken again");
        assert!(changed(&mut watcher), "made again, it is to be loaded");
        assert!(!readable(&watcher, 200), "and looked for no longer");
    }

    #[test]
    fn a_directory_that_is_a_link_whose_directory_is_renamed_away_is_let_go_of() {
        let parent = TempDir::new().unwrap();
        let elsewhere = TempDir::new().unwrap();
        let target = elsewhere.path().join("omorfos");
        let aside = elsewhere.path().join("omorfos.aside");
        fs::create_dir(&target).unwrap();
        let directory = parent.path().join("omorfos");
        std::os::unix::fs::symlink(&target, &directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();

        fs::rename(&target, &aside).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        fs::write(aside.join("config.yaml"), "").unwrap();
        assert!(!changed(&mut watcher), "what it became is not watched");
        fs::create_dir(&target).unwrap();
        assert!(changed(&mut watcher), "made again, it is to be loaded");
        fs::write(target.join("config.yaml"), "").unwrap();
        assert!(changed(&mut watcher), "and is watched again");
    }

    #[test]
    fn a_directory_moved_aside_is_a_change_and_what_it_became_is_not_watched() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        let aside = parent.path().join("omorfos.aside");
        fs::create_dir(&directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();

        // As `mv omorfos omorfos.aside`, to try the defaults.
        fs::rename(&directory, &aside).unwrap();
        assert!(changed(&mut watcher), "it is gone, to be loaded");
        fs::write(aside.join("config.yaml"), "").unwrap();
        assert!(!changed(&mut watcher), "what it became is not watched");
        fs::rename(&aside, &directory).unwrap();
        assert!(changed(&mut watcher), "put back, it is to be loaded");
        fs::write(directory.join("config.yaml"), "").unwrap();
        assert!(changed(&mut watcher), "and is watched again");
    }

    #[test]
    fn a_directory_made_again_while_the_queue_overflowed_is_watched_again() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        fs::create_dir(&directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();
        // Two files written by turns, as the same event twice running is
        // queued once, until the kernel has dropped events.
        let limit = fs::read_to_string("/proc/sys/fs/inotify/max_queued_events").unwrap();
        let limit: usize = limit.trim().parse().unwrap();
        let files = [directory.join("a"), directory.join("b")];
        for index in 0..=limit {
            fs::write(&files[index % 2], "").unwrap();
        }
        // Its going and coming are among what was dropped.
        fs::remove_dir_all(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        assert!(changed(&mut watcher));
        fs::write(directory.join("config.yaml"), "").unwrap();
        assert!(changed(&mut watcher), "and it is watched again");
    }

    #[test]
    fn a_configuration_file_beside_the_directory_is_not_a_change() {
        let parent = TempDir::new().unwrap();
        let directory = parent.path().join("omorfos");
        fs::create_dir(&directory).unwrap();
        let mut watcher = Watcher::new(&directory).unwrap();
        fs::write(parent.path().join("config.yaml"), "").unwrap();
        fs::create_dir(parent.path().join("other")).unwrap();
        assert!(!changed(&mut watcher));
    }

    #[test]
    fn a_queue_overflow_counts_as_a_change() {
        assert!(is_relevant(EventMask::Q_OVERFLOW, None));
        assert!(!is_relevant(EventMask::IGNORED, None));
    }

    /// A directory in one of its own, for a test of when the watcher wakes:
    /// one made in the directory every test makes its own in would hear of
    /// theirs coming and going, as the directory a directory is in is
    /// watched for its name.
    fn quiet() -> (TempDir, TempDir) {
        let parent = TempDir::new().unwrap();
        let directory = TempDir::new_in(parent.path()).unwrap();
        (directory, parent)
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
        let (directory, _parent) = quiet();
        let mut watcher = Watcher::new(directory.path()).unwrap();
        assert!(!readable(&watcher, 0));
        fs::write(directory.path().join("config.yaml"), "").unwrap();
        assert!(readable(&watcher, 100));
        assert!(changed(&mut watcher));
        assert!(!readable(&watcher, 0), "and is quiet once drained");
    }

    #[test]
    fn it_wakes_at_the_time_it_is_told_as_though_something_had_changed() {
        let (directory, _parent) = quiet();
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
        let (directory, _parent) = quiet();
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
        let (directory, _parent) = quiet();
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
