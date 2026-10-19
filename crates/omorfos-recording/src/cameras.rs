//! Cameras a program opens itself, and not through PipeWire: seen by which
//! processes have one among their open files, looked at only when a camera
//! has been opened or let go.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use inotify::{Inotify, WatchMask};

/// What the name of a camera starts with in the folder of devices.
const CAMERA: &str = "video";

/// How long after a camera is opened or let go the processes are looked
/// at: the kernel tells of a camera being opened a moment before it is
/// among the process's open files.
const SETTLE: Duration = Duration::from_millis(100);

/// The cameras in a folder of devices, watched for being opened and let go.
pub(crate) struct Cameras {
    folder: PathBuf,
    inotify: Inotify,
    buffer: Vec<u8>,
    /// When to look at who has a camera open, if anything has changed.
    due: Option<Instant>,
}

impl Cameras {
    /// Watches the cameras in `folder`, and those that come into it, and
    /// is due to look at who has one open at once.
    ///
    /// # Errors
    ///
    /// Returns why `folder` cannot be watched.
    pub(crate) fn watch(folder: &Path) -> io::Result<Self> {
        let watching = Inotify::init().and_then(|inotify| {
            let mask = WatchMask::CREATE | WatchMask::ONLYDIR;
            inotify.watches().add(folder, mask).map(|_| inotify)
        });
        watching.map(|inotify| {
            let mut cameras = Self {
                folder: folder.to_owned(),
                inotify,
                buffer: vec![0; 4096],
                due: Some(Instant::now()),
            };
            let there = std::fs::read_dir(folder).into_iter().flatten().flatten();
            for entry in there {
                cameras.follow(&entry.file_name());
            }
            cameras
        })
    }

    /// Watches what is called `name` in the folder for being opened and let
    /// go, if it is a camera.
    fn follow(&mut self, name: &OsStr) {
        let mask = WatchMask::OPEN | WatchMask::CLOSE;
        if is_camera(name) {
            let _ = self.inotify.watches().add(self.folder.join(name), mask);
        }
    }

    /// Whether it is time, `now`, to look at who has a camera open: a
    /// moment after one was opened, let go, or came.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        while let Ok(events) = self.inotify.read_events(&mut self.buffer) {
            let names: Vec<_> = events
                .map(|event| event.name.map(OsStr::to_owned))
                .collect();
            self.due = self.due.or(Some(now + SETTLE));
            for name in names.into_iter().flatten() {
                self.follow(&name);
            }
        }
        let due = self.due.is_some_and(|due| due <= now);
        self.due = self.due.filter(|_| !due);
        due
    }

    /// The processes that have a camera open, of those whose open files
    /// can be seen: the user's own.
    pub(crate) fn holding(&self) -> BTreeSet<u32> {
        let processes = std::fs::read_dir("/proc").into_iter().flatten().flatten();
        let pids = processes.filter_map(|entry| {
            let name = entry.file_name();
            name.to_str().and_then(|name| name.parse::<u32>().ok())
        });
        pids.filter(|pid| self.opened(*pid)).collect()
    }

    /// Whether a process has a camera open.
    fn opened(&self, pid: u32) -> bool {
        let files = std::fs::read_dir(format!("/proc/{pid}/fd")).into_iter();
        let files = files.flatten().flatten();
        let mut files = files.filter_map(|file| std::fs::read_link(file.path()).ok());
        files.any(|file| {
            file.parent() == Some(&self.folder) && file.file_name().is_some_and(is_camera)
        })
    }
}

/// Whether what is called `name` in the folder of devices is a camera.
fn is_camera(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| name.starts_with(CAMERA))
}
