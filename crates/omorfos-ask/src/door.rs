//! Where `omorfos-pinentry` asks for a passphrase to be asked in the sealed
//! prompt: a socket in the runtime directory that only the user can open,
//! taken from only by the installed pinentry, started by `gpg-agent`
//! (ADR 58).
//!
//! What the kernel says of who is at the other end is checked: which
//! program it is, and which program started it. That keeps any program of
//! the user's from putting up the prompt by writing to the socket; it is
//! not what keeps it safe, which is that the prompt asks in words of
//! omorfos's alone.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::assuan::{LONGEST, escaped};
use crate::pinentry::{Wanted, requested};

/// What the socket is called in the runtime directory.
pub const NAME: &str = "omorfos-ask";

/// The program that may start the pinentry, by its name.
const AGENT: &str = "gpg-agent";

/// How long a pinentry has to say what it asks, once it has connected.
const SAY: Duration = Duration::from_secs(2);

/// The socket, listened on.
#[derive(Debug)]
pub(crate) struct Door {
    listener: UnixListener,
    /// The pinentry that may ask, where it is installed.
    pinentry: PathBuf,
}

/// A passphrase a pinentry asks for, and where to give it.
#[derive(Debug)]
pub(crate) struct Passing {
    stream: UnixStream,
    /// What is asked.
    pub(crate) wanted: Wanted,
}

impl Door {
    /// Listens at `path`, which only the user may open, for the pinentry
    /// at `pinentry`. One left behind by an earlier run is replaced.
    ///
    /// # Errors
    ///
    /// If it cannot be listened at.
    pub(crate) fn open(path: &Path, pinentry: PathBuf) -> io::Result<Self> {
        // If there is none to remove, the removal fails, harmlessly.
        let _ = fs::remove_file(path);
        let owner_only = fs::Permissions::from_mode(0o600);
        UnixListener::bind(path)
            .and_then(|listener| fs::set_permissions(path, owner_only).map(|()| listener))
            .and_then(|listener| listener.set_nonblocking(true).map(|()| listener))
            .map(|listener| Self { listener, pinentry })
    }

    /// What the pinentries that have knocked ask, of those that may ask;
    /// the rest are turned away, and the log says so.
    pub(crate) fn knocked(&self) -> Vec<Passing> {
        let mut asked = Vec::new();
        while let Ok((stream, _)) = self.listener.accept() {
            if !self.may_ask(&stream) {
                note(
                    "a program that is not omorfos-pinentry, started by gpg-agent, was turned away",
                );
                continue;
            }
            // One that says nothing in time, or nothing understood, is let go.
            asked.extend(said(&stream).map(|wanted| {
                note(&format!(
                    "GnuPG asks for a passphrase, and says: {}",
                    wanted.said
                ));
                Passing { stream, wanted }
            }));
        }
        asked
    }

    /// Whether the program at the other end of `stream` is the installed
    /// pinentry, started by `gpg-agent`, as the kernel says.
    fn may_ask(&self, stream: &UnixStream) -> bool {
        let peer = rustix::net::sockopt::socket_peercred(stream).ok();
        let pid = peer.map(|peer| peer.pid.as_raw_nonzero().get());
        pid.is_some_and(|pid| {
            let ours = fs::canonicalize(&self.pinentry).ok();
            let running = program(pid);
            let agent = parent(pid).and_then(name);
            ours.is_some() && running == ours && agent.as_deref() == Some(AGENT)
        })
    }
}

impl AsFd for Door {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }
}

/// The program process `pid` is running, as the kernel says.
fn program(pid: i32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// What process `pid` is called: the name of the program it runs, as the
/// kernel says; or, where that is not to be read, the name the kernel keeps
/// of it. `gpg-agent` makes itself so that what it runs cannot be read, to
/// keep what it holds from being looked into, and is known by that name.
fn name(pid: i32) -> Option<String> {
    let named = program(pid).and_then(|program| program.file_name().map(ToOwned::to_owned));
    let named = named.and_then(|named| named.into_string().ok());
    named.or_else(|| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok();
        let after = stat.as_deref().and_then(|stat| stat.split_once(" ("));
        let called = after.and_then(|(_, after)| after.rsplit_once(") "));
        called.map(|(called, _)| called.to_owned())
    })
}

/// The process that started process `pid`, as the kernel says.
fn parent(pid: i32) -> Option<i32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok();
    // The program's name, in brackets, may hold anything; what follows the
    // last bracket does not: the state, and then the parent.
    let after = stat.as_deref().and_then(|stat| stat.rsplit_once(") "));
    let parent = after.and_then(|(_, after)| after.split(' ').nth(1));
    parent.and_then(|parent| parent.parse().ok())
}

/// What a pinentry that has just connected asks, if it says it in time.
fn said(stream: &UnixStream) -> Option<Wanted> {
    let _ = stream.set_read_timeout(Some(SAY));
    let mut line = String::new();
    let longest = u64::try_from(LONGEST * 4).unwrap_or(u64::MAX);
    let mut reader = BufReader::new(Read::take(stream, longest));
    let read = reader.read_line(&mut line).is_ok();
    requested(&line).filter(|_| read)
}

impl Passing {
    /// Gives the pinentry what was typed, or says that nothing was. What
    /// was typed is wiped once it is on its way.
    pub(crate) fn give(mut self, typed: Option<String>) {
        let said = typed.map_or_else(
            || "CANCEL\n".to_owned(),
            |typed| {
                let said = format!("D {}\n", escaped(&typed));
                typed.into_bytes().fill(0);
                said
            },
        );
        // A pinentry that has gone has nobody left to tell.
        let _ = self.stream.write_all(said.as_bytes());
        said.into_bytes().fill(0);
    }

    /// Whether the pinentry has gone, as one `gpg-agent` has given up on
    /// has.
    #[must_use]
    pub(crate) fn gone(&self) -> bool {
        let mut waiting = [PollFd::new(&self.stream, PollFlags::IN)];
        let now = Timespec::default();
        let woken = poll(&mut waiting, Some(&now)).unwrap_or(0) > 0;
        // Woken with nothing to read is at its end: a pinentry says
        // nothing after what it asks.
        let mut peek = [0; 1];
        let flags = rustix::net::RecvFlags::PEEK | rustix::net::RecvFlags::DONTWAIT;
        let read = rustix::net::recv(&self.stream, &mut peek, flags);
        woken && read.is_ok_and(|(read, _)| read == 0)
    }
}

impl AsFd for Passing {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

/// Says in the session's log what became of something asked.
fn note(said: &str) {
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(io::stderr(), "omorfos-ask: {said}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_is_known_by_the_program_it_runs_and_the_one_that_started_it() {
        let me = std::process::id().cast_signed();
        assert_eq!(program(me), std::env::current_exe().ok());
        let started_by = parent(me).unwrap();
        assert!(started_by > 0, "{started_by}");
        assert_eq!(program(0), None);
        assert_eq!(parent(0), None);
        let own = std::env::current_exe().unwrap();
        let own = own.file_name().unwrap().to_str().unwrap().to_owned();
        assert_eq!(name(me), Some(own));
        assert_eq!(name(0), None);
        // The first process's program is the kernel's alone to read; its name
        // is not.
        assert!(name(1).is_some_and(|name| !name.is_empty()));
    }

    #[test]
    fn only_the_pinentry_started_by_gpg_agent_may_ask_and_others_are_turned_away() {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join(NAME);
        let door = Door::open(&path, PathBuf::from("/nowhere/omorfos-pinentry")).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the user's alone");

        // This test is neither the pinentry nor started by `gpg-agent`.
        let mut knocking = UnixStream::connect(&path).unwrap();
        knocking.write_all(b"ASK passphrase first - - +\n").unwrap();
        assert!(door.knocked().is_empty());
        let mut answer = String::new();
        // Let go of with what it said unread, its end of it may be reset.
        let read = BufReader::new(&knocking).read_line(&mut answer);
        assert_eq!(read.unwrap_or(0), 0, "and is told nothing: {answer}");
    }
}
