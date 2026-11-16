//! Sandboxes made through the relay (ADR 61, step 3d).
//!
//! A program such as Flatpak makes a sandbox by handing over a socket it
//! has made and listens on, with a descriptor whose closing ends the
//! sandbox (`wp_security_context_v1`): whatever connects at that socket is
//! inside the sandbox. Handed to the compositor, the socket would be the
//! compositor's to serve, and every program that connected there would end
//! with it. So the relay answers for the sandbox being made, and the
//! compositor is never told of it: the relay listens on the socket itself,
//! and each program that connects there is relayed like any other, kept
//! through a change of compositor, and handed to each compositor as one
//! inside a sandbox. A program inside one is not offered the way to make
//! one, so it cannot make itself another.
//!
//! What a sandbox is said to be, its engine, its program and its instance,
//! is not kept: the compositor takes nothing from it, and a program inside
//! is held to the same whatever it is.
//!
//! Each sandbox's socket is listened on by a thread of its own, which hands
//! the relay what connects. Whether taking a program from a socket waits is
//! the socket's to say, and the socket is the program's that made it as
//! much as the relay's: told not to wait, it can be told again to, and a
//! connection the relay was woken for can be taken by that program first.
//! Either would have the relay wait in `accept` for good, and every program
//! with it, as a pipe written to would (ADR 90). The thread waits instead,
//! and the relay is woken only once there is a program to take.

use std::cell::Cell;
use std::collections::HashMap;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::{Errno, read, retry_on_intr, write};
use rustix::net::{
    AddressFamily, Shutdown, SocketFlags, SocketType, accept_with, shutdown, socketpair,
};
use wayland_backend::protocol::MessageDesc;

use crate::AGAIN;
use crate::objects::Read;

/// How many programs that came in through one sandbox wait for the relay
/// to take them; past that, they wait to be taken from the socket.
const ROOM: usize = 4;

/// What makes a sandbox, and the sandbox being made.
const MANAGER: &str = "wp_security_context_manager_v1";
const CONTEXT: &str = "wp_security_context_v1";

/// A sandbox: the socket listened on, and the descriptor whose closing
/// ends it.
#[derive(Debug)]
pub struct Sandbox {
    listening: Rc<OwnedFd>,
    closing: Rc<OwnedFd>,
}

impl Sandbox {
    /// Listens on the sandbox, on a thread of its own.
    ///
    /// # Errors
    ///
    /// Returns an error if no descriptor or thread is left for it.
    pub fn listen(self) -> io::Result<Listened> {
        let (sent, came) = mpsc::sync_channel(ROOM);
        let theirs = self.listening.try_clone();
        let flags = SocketFlags::CLOEXEC | SocketFlags::NONBLOCK;
        let pair = socketpair(AddressFamily::UNIX, SocketType::STREAM, flags, None);
        let woken = theirs.and_then(|theirs| {
            pair.map_err(io::Error::from).and_then(|(wake, woken)| {
                let thread = thread::Builder::new().name("omorfos-relay: sandbox".to_owned());
                let started = thread.spawn(move || listen(&theirs, &woken, &sent));
                started.map(|_| wake)
            })
        });
        woken.map(|wake| Listened {
            listening: self.listening,
            closing: self.closing,
            wake,
            came,
        })
    }
}

/// What came in at a sandbox's socket: a program, or why one could not
/// be taken.
#[derive(Debug)]
enum Came {
    Program(UnixStream),
    Failed(Errno),
}

/// Takes each program that connects at `listening` and hands it on through
/// `came`, waking the relay at `woken`, until the relay hangs up there.
fn listen(listening: &OwnedFd, woken: &OwnedFd, came: &SyncSender<Came>) {
    // The relay never writes to its end, so it is read from only once it
    // has hung up.
    while !matches!(read(woken, &mut [0]), Ok(0)) {
        let mut polled = [
            PollFd::new(listening, PollFlags::IN),
            PollFd::new(woken, PollFlags::IN),
        ];
        let _ = retry_on_intr(|| poll(&mut polled, None));
        // Taken without waiting, if the socket says so; if it does not, and
        // the program took the one there first, until the next connects or
        // the relay shuts the socket.
        let taken = accept_with(listening, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
        let failed = taken.as_ref().err().copied();
        // One that could not be taken, as when no descriptor is left, is
        // still there: tried again at once, it would be again and again.
        let _ = failed.map(|_| thread::sleep(AGAIN));
        let came = came.send(taken.map_or_else(Came::Failed, |program| {
            Came::Program(UnixStream::from(program))
        }));
        // The relay is woken with a byte; one that has bytes left to read
        // will be woken without another.
        let _ = came.map(|()| write(woken, &[0]));
    }
}

/// A sandbox listened on, by a thread that stops when it is let go of.
#[derive(Debug)]
pub struct Listened {
    listening: Rc<OwnedFd>,
    closing: Rc<OwnedFd>,
    /// Readable once a program has come in; hung up, it stops the thread.
    wake: OwnedFd,
    came: Receiver<Came>,
}

impl Listened {
    /// The descriptor that is readable once a program has come in through
    /// the sandbox.
    #[must_use]
    pub fn woken(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }

    /// The descriptor that is readable, or hung up, once the sandbox has
    /// ended.
    #[must_use]
    pub fn closing(&self) -> BorrowedFd<'_> {
        self.closing.as_fd()
    }

    /// Each program that has come in through the sandbox, without waiting
    /// for one; `failed` is set if one waiting could not be taken, as when
    /// no descriptor is left.
    pub fn waiting<'a>(&'a self, failed: &'a Cell<bool>) -> impl Iterator<Item = UnixStream> + 'a {
        // What woke the relay is read first, so that what comes after it
        // wakes it again.
        while matches!(read(&self.wake, &mut [0; 64]), Ok(1..)) {}
        self.came.try_iter().filter_map(|came| match came {
            Came::Program(program) => Some(program),
            Came::Failed(error) => {
                failed.set(error != Errno::AGAIN);
                None
            }
        })
    }
}

impl Drop for Listened {
    fn drop(&mut self) {
        // The thread is told by its end hung up, and woken by the socket
        // shut if it waits to take a program; it is not waited for. Shut,
        // the socket takes no one more, which an ended sandbox does not.
        let _ = shutdown(&self.wake, Shutdown::Both);
        let _ = shutdown(&*self.listening, Shutdown::Both);
    }
}

/// The sandboxes a program is making, and those it has made since they were
/// last taken.
#[derive(Debug, Default)]
pub struct Sandboxes {
    /// By the program's number for the sandbox being made.
    making: HashMap<u32, Sandbox>,
    made: Vec<Sandbox>,
}

impl Sandboxes {
    /// Takes in `read`, the program's request `described` to `object`,
    /// with `fds`, if it is one of making a sandbox. Returns whether it
    /// was: one is the relay's to answer, and not passed on.
    pub fn request(
        &mut self,
        object: u32,
        read: &Read,
        described: &MessageDesc,
        fds: &[Rc<OwnedFd>],
    ) -> bool {
        match (read.interface.name, described.name) {
            (MANAGER, "create_listener") => {
                let made = read.made.map(|(made, ..)| made);
                let given = fds.first().zip(fds.get(1));
                let making = made.zip(given).map(|(made, (listening, closing))| {
                    let sandbox = Sandbox {
                        listening: Rc::clone(listening),
                        closing: Rc::clone(closing),
                    };
                    self.making.insert(made, sandbox);
                });
                let _ = making;
                true
            }
            (CONTEXT, "commit") => {
                // Made once; what is asked of it after that is passed over.
                let made = self.making.remove(&object);
                self.made.extend(made);
                true
            }
            (CONTEXT, _) => {
                if read.ends {
                    self.making.remove(&object);
                }
                true
            }
            _ => false,
        }
    }

    /// The sandboxes made since they were last taken.
    pub fn made(&mut self) -> Vec<Sandbox> {
        std::mem::take(&mut self.made)
    }
}
