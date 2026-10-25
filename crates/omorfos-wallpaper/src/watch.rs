//! Hearing of pictures put in, changed in or taken out of the directories
//! the pictures shown are in, so that what is shown follows at once and
//! not at the next turn.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};

/// The directories watched, and what was heard of them. Nothing here
/// blocks: [`Watch::as_fd`] is readable when there is something to hear.
#[derive(Debug)]
pub(crate) struct Watch {
    inotify: Inotify,
    watched: Vec<(WatchDescriptor, PathBuf)>,
    buffer: Vec<u8>,
}

impl Watch {
    /// A watch on nothing yet.
    ///
    /// # Errors
    ///
    /// If the kernel gives no more watches.
    pub(crate) fn new() -> io::Result<Self> {
        Self::with_buffer(vec![0; 4096])
    }

    fn with_buffer(buffer: Vec<u8>) -> io::Result<Self> {
        Inotify::init().map(|inotify| Self {
            inotify,
            watched: Vec::new(),
            buffer,
        })
    }

    /// Watches `places`, and nothing else. One that cannot be watched, as
    /// one that is not there, is not, until it is asked for again.
    pub(crate) fn follow<'a>(&mut self, places: impl Iterator<Item = &'a Path>) {
        let places: Vec<&Path> = places.collect();
        let (kept, dropped) = std::mem::take(&mut self.watched)
            .into_iter()
            .partition(|(_, place)| places.contains(&place.as_path()));
        self.watched = kept;
        for (descriptor, _) in dropped {
            // One already gone with its directory cannot be taken away
            // again, harmlessly.
            let _ = self.inotify.watches().remove(descriptor);
        }
        let mask = WatchMask::CLOSE_WRITE
            | WatchMask::DELETE
            | WatchMask::MOVED_FROM
            | WatchMask::MOVED_TO
            | WatchMask::ONLYDIR;
        for place in places {
            let known = self.watched.iter().any(|(_, watched)| watched == place);
            let added = (!known).then(|| self.inotify.watches().add(place, mask).ok());
            self.watched
                .extend(added.flatten().map(|added| (added, place.to_owned())));
        }
    }

    /// The files heard to have been put in, changed or taken out since
    /// this was last asked. After more changes than the kernel could keep,
    /// the directories themselves, each to be looked at again.
    ///
    /// # Errors
    ///
    /// If what was heard cannot be read.
    pub(crate) fn heard(&mut self) -> io::Result<Vec<PathBuf>> {
        let mut heard = Vec::new();
        loop {
            match self.inotify.read_events(&mut self.buffer) {
                Ok(events) => {
                    for event in events {
                        if event.mask.contains(EventMask::IGNORED) {
                            // Its directory is gone, and with it the watch.
                            self.watched.retain(|(watched, _)| *watched != event.wd);
                        } else if event.mask.contains(EventMask::Q_OVERFLOW) {
                            heard.extend(self.watched.iter().map(|(_, place)| place.clone()));
                        } else {
                            let mut places = self.watched.iter();
                            let place = places.find(|(watched, _)| *watched == event.wd);
                            let place = place.map(|(_, place)| place);
                            let file = place.zip(event.name).map(|(place, name)| place.join(name));
                            heard.extend(file);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(heard),
                Err(error) => return Err(error),
            }
        }
    }
}

impl AsFd for Watch {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inotify.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn what_is_put_in_changed_in_or_taken_out_of_a_place_watched_is_heard() {
        let one = tempfile::TempDir::new().unwrap();
        let other = tempfile::TempDir::new().unwrap();
        let mut watch = Watch::new().unwrap();
        watch.follow([one.path(), other.path()].into_iter());
        assert_eq!(watch.heard().unwrap(), [] as [PathBuf; 0]);
        let picture = one.path().join("hills.png");
        fs::write(&picture, "").unwrap();
        fs::rename(&picture, other.path().join("hills.png")).unwrap();
        fs::remove_file(other.path().join("hills.png")).unwrap();
        assert_eq!(
            watch.heard().unwrap(),
            [
                picture.clone(),
                picture,
                other.path().join("hills.png"),
                other.path().join("hills.png"),
            ]
        );
    }

    #[test]
    fn only_the_places_followed_are_watched_and_each_once() {
        let one = tempfile::TempDir::new().unwrap();
        let other = tempfile::TempDir::new().unwrap();
        let mut watch = Watch::new().unwrap();
        watch.follow([one.path(), one.path()].into_iter());
        watch.follow([other.path(), Path::new("/nowhere")].into_iter());
        fs::write(one.path().join("a.png"), "").unwrap();
        fs::write(other.path().join("b.png"), "").unwrap();
        assert_eq!(watch.heard().unwrap(), [other.path().join("b.png")]);
    }

    #[test]
    fn a_place_taken_away_is_not_watched_until_it_is_followed_again() {
        let parent = tempfile::TempDir::new().unwrap();
        let place = parent.path().join("pictures");
        fs::create_dir(&place).unwrap();
        let mut watch = Watch::new().unwrap();
        watch.follow([place.as_path()].into_iter());
        fs::remove_dir(&place).unwrap();
        assert_eq!(watch.heard().unwrap(), [] as [PathBuf; 0]);
        // Made again, it is watched once it is followed.
        fs::create_dir(&place).unwrap();
        watch.follow([place.as_path()].into_iter());
        fs::write(place.join("new.png"), "").unwrap();
        assert_eq!(watch.heard().unwrap(), [place.join("new.png")]);
    }

    #[test]
    fn after_more_changes_than_were_kept_each_place_is_heard_of_itself() {
        let place = tempfile::TempDir::new().unwrap();
        let mut watch = Watch::new().unwrap();
        watch.follow([place.path()].into_iter());
        // More than the kernel keeps for a watch by default, 16384.
        let max = fs::read_to_string("/proc/sys/fs/inotify/max_queued_events").unwrap();
        let max: usize = max.trim().parse().unwrap();
        // Each of its own, as the kernel makes one of the same change again
        // and again.
        for at in 0..=max {
            fs::write(place.path().join(format!("{at}.png")), "").unwrap();
        }
        let heard = watch.heard().unwrap();
        assert!(heard.contains(&place.path().to_owned()));
    }

    #[test]
    fn what_cannot_be_read_is_said() {
        let place = tempfile::TempDir::new().unwrap();
        // A buffer too small for even one event makes the kernel refuse the
        // read.
        let mut watch = Watch::with_buffer(vec![0; 1]).unwrap();
        watch.follow([place.path()].into_iter());
        fs::write(place.path().join("a.png"), "").unwrap();
        assert!(watch.heard().is_err());
    }
}
