//! The socket programs connect to, held for the whole login.
//!
//! It is named and locked as every Wayland compositor names and locks its
//! socket, `wayland-N` beside `wayland-N.lock` in the runtime directory, so
//! that two sessions never take the same name.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};

/// The names tried, as `wayland-N`, the first free one taken.
const NUMBERS: std::ops::Range<u32> = 1..33;

/// The socket, its name and the lock that keeps it.
#[derive(Debug)]
pub struct Socket {
    listener: UnixListener,
    path: PathBuf,
    /// Held as long as the socket is: the name is taken while it is.
    lock: File,
    lock_path: PathBuf,
}

impl Socket {
    /// Binds the first name free in `runtime`.
    ///
    /// # Errors
    ///
    /// Returns an error if no name is free, or none can be bound.
    pub fn bind(runtime: &Path) -> io::Result<Self> {
        let mut last = io::Error::new(io::ErrorKind::AddrInUse, "no name is free");
        for number in NUMBERS {
            match Self::bind_as(runtime, &format!("wayland-{number}")) {
                Ok(socket) => return Ok(socket),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    /// Binds `name` in `runtime`, if nobody holds it.
    fn bind_as(runtime: &Path, name: &str) -> io::Result<Self> {
        let lock_path = runtime.join(format!("{name}.lock"));
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o660)
            .open(&lock_path)?;
        flock(&lock, FlockOperation::NonBlockingLockExclusive)?;
        // Holding the lock, a socket left by whoever held it last is theirs
        // no longer.
        let path = runtime.join(name);
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        Ok(Self {
            listener,
            path,
            lock,
            lock_path,
        })
    }

    /// The socket `listener` at `path`, kept by `lock`, as another version
    /// of the holder held it.
    #[must_use]
    pub fn from_parts(listener: UnixListener, path: PathBuf, lock: File) -> Self {
        let mut lock_path = path.clone().into_os_string();
        lock_path.push(".lock");
        Self {
            listener,
            path,
            lock,
            lock_path: lock_path.into(),
        }
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The lock that keeps its name, to hand to another version.
    #[must_use]
    pub fn lock(&self) -> BorrowedFd<'_> {
        self.lock.as_fd()
    }
}

impl AsFd for Socket {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;

    use super::*;

    #[test]
    fn the_first_free_name_is_taken_and_given_back_at_the_end() {
        let runtime = tempfile::TempDir::new().unwrap();
        let first = Socket::bind(runtime.path()).unwrap();
        assert_eq!(first.path(), runtime.path().join("wayland-1"));
        assert!(runtime.path().join("wayland-1.lock").exists());
        // Programs can connect to it.
        UnixStream::connect(first.path()).unwrap();
        let second = Socket::bind(runtime.path()).unwrap();
        assert_eq!(second.path(), runtime.path().join("wayland-2"));
        drop(first);
        assert!(!runtime.path().join("wayland-1").exists());
        assert!(!runtime.path().join("wayland-1.lock").exists());
        let again = Socket::bind(runtime.path()).unwrap();
        assert_eq!(again.path(), runtime.path().join("wayland-1"));
        drop(second);
    }

    #[test]
    fn a_socket_left_behind_by_whoever_held_the_name_is_taken_over() {
        let runtime = tempfile::TempDir::new().unwrap();
        let left = UnixListener::bind(runtime.path().join("wayland-1")).unwrap();
        let socket = Socket::bind(runtime.path()).unwrap();
        assert_eq!(socket.path(), runtime.path().join("wayland-1"));
        drop(left);
    }

    #[test]
    fn with_every_name_taken_none_is_bound() {
        let runtime = tempfile::TempDir::new().unwrap();
        let all: Vec<Socket> = NUMBERS
            .map(|_| Socket::bind(runtime.path()).unwrap())
            .collect();
        let none = Socket::bind(runtime.path()).unwrap_err();
        assert_eq!(none.kind(), io::ErrorKind::WouldBlock, "{none}");
        drop(all);
    }

    #[test]
    fn where_nothing_can_be_made_nothing_is_bound() {
        let nowhere = Path::new("/nonexistent/omorfos-hold");
        assert!(Socket::bind(nowhere).is_err());
        // A place whose path is too long for a socket's name.
        let runtime = tempfile::TempDir::new().unwrap();
        let long = runtime.path().join("x".repeat(120));
        fs::create_dir(&long).unwrap();
        let error = Socket::bind(&long).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{error}");
    }
}
