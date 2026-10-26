//! Stands between Wayland programs and the omorfos compositor (ADR 60 and
//! ADR 61).
//!
//! Programs connect to the relay, at the socket the holder hands it. For
//! each, the relay makes a connection to the compositor, hands the
//! compositor its end with who the program is, and passes every message
//! on, each read as it goes. When the compositor ends, so does every
//! program's connection, until the relay can make what a program had
//! again for the compositor started anew.

mod known;
pub mod link;
pub mod objects;
pub mod wire;

use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use omorfos_ipc::relay::{self, Hand, Program, Received};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::link::{Ended, Link, Way};

/// What a connection being ready to read is, or its other end gone.
const READABLE: PollFlags = PollFlags::IN.union(PollFlags::HUP).union(PollFlags::ERR);

/// The relay, and every program's connection through it.
#[derive(Debug)]
pub struct Relay {
    /// The connection to the holder, which hands the relay what it serves.
    holder: OwnedFd,
    /// The socket programs connect to, once the holder has handed it.
    socket: Option<UnixListener>,
    /// The connection to the compositor, while there is one.
    compositor: Option<OwnedFd>,
    links: Vec<Link>,
}

impl Relay {
    /// A relay that the holder hands what it serves over `holder`.
    #[must_use]
    pub const fn new(holder: OwnedFd) -> Self {
        Self {
            holder,
            socket: None,
            compositor: None,
            links: Vec::new(),
        }
    }

    /// How many programs are connected through the relay.
    #[must_use]
    pub fn programs(&self) -> usize {
        self.links.len()
    }

    /// Waits up to `timeout`, or until something happens, and does what it
    /// says; says on `log` why a program's connection was ended, if it was
    /// for what it sent. Returns whether the relay is to go on: not once the
    /// holder has gone.
    pub fn step(&mut self, timeout: Option<Duration>, log: &mut dyn Write) -> bool {
        let ready = self.wait(timeout);
        let mut ready = ready.into_iter();
        // What the holder hands changes what the rest are; they are read
        // again once that is taken.
        if ready
            .next()
            .is_some_and(|holder| holder.intersects(READABLE))
        {
            return self.hear_holder();
        }
        if let Some(compositor) = self.compositor.take() {
            let gone = ready.next().is_some_and(|ready| ready.intersects(READABLE));
            let gone = gone && relay::receive(compositor.as_fd()).is_err();
            if gone {
                // Its programs cannot be kept yet (ADR 61, step 1).
                self.links.clear();
                return true;
            }
            if let Some(socket) = &self.socket {
                let waiting = ready.next().is_some_and(|ready| ready.intersects(READABLE));
                if waiting {
                    admit(socket, compositor.as_fd(), &mut self.links, log);
                }
            }
            self.compositor = Some(compositor);
        }
        let mut ended = Vec::new();
        for (index, link) in self.links.iter_mut().enumerate() {
            let program = ready.next().unwrap_or(PollFlags::empty());
            let compositor = ready.next().unwrap_or(PollFlags::empty());
            if let Err(why) = serve(link, program, compositor) {
                ended.push((index, why));
            }
        }
        for (index, why) in ended.into_iter().rev() {
            self.links.remove(index);
            if let Ended::Broken(broken) = why {
                let _ = writeln!(
                    log,
                    "omorfos-relay: a program was let go: it sent {}",
                    broken.0
                );
            }
        }
        true
    }

    /// Waits up to `timeout` for any connection to be ready, and says how
    /// each is: the holder's, the compositor's and the socket while there
    /// is one, then each program's and its connection to the compositor.
    fn wait(&self, timeout: Option<Duration>) -> Vec<PollFlags> {
        let mut polled = vec![PollFd::new(&self.holder, PollFlags::IN)];
        if let Some(compositor) = &self.compositor {
            polled.push(PollFd::new(compositor, PollFlags::IN));
            polled.extend(
                self.socket
                    .as_ref()
                    .map(|socket| PollFd::new(socket, PollFlags::IN)),
            );
        }
        for link in &self.links {
            let wanted = |reads: bool, writes: bool| {
                let reads = if reads {
                    PollFlags::IN
                } else {
                    PollFlags::empty()
                };
                let writes = if writes {
                    PollFlags::OUT
                } else {
                    PollFlags::empty()
                };
                reads | writes
            };
            let program = wanted(link.reads(Way::Up), link.writes(Way::Down));
            let compositor = wanted(link.reads(Way::Down), link.writes(Way::Up));
            polled.push(PollFd::from_borrowed_fd(link.program(), program));
            polled.push(PollFd::from_borrowed_fd(link.compositor(), compositor));
        }
        let timeout = timeout.and_then(|timeout| Timespec::try_from(timeout).ok());
        // Interrupted, nothing is ready, and the caller steps again.
        let _ = poll(&mut polled, timeout.as_ref());
        polled.iter().map(PollFd::revents).collect()
    }

    /// Takes what the holder has handed. Returns whether the holder is
    /// still there.
    fn hear_holder(&mut self) -> bool {
        loop {
            match relay::receive(self.holder.as_fd()) {
                Ok(Some(received)) => self.take(received),
                Ok(None) => return true,
                Err(_) => return false,
            }
        }
    }

    /// Takes a hand from the holder.
    fn take(&mut self, received: Received) {
        let Received { hand, fds } = received;
        let fd = fds.into_iter().next();
        match (hand, fd) {
            (Some(Hand::Socket), Some(socket)) => {
                let socket = UnixListener::from(socket);
                // One that cannot be kept from waiting is not served, and
                // the compositor started next is handed the socket itself.
                self.socket = socket.set_nonblocking(true).map(|()| socket).ok();
            }
            (Some(Hand::Compositor), Some(compositor)) => {
                // Programs of the compositor before cannot be kept yet.
                self.links.clear();
                self.compositor = Some(compositor);
            }
            _ => {}
        }
    }
}

/// Takes each program waiting at `socket`, and hands `compositor` a
/// connection for it, keeping each joined to its connection in `links`.
fn admit(
    socket: &UnixListener,
    compositor: BorrowedFd<'_>,
    links: &mut Vec<Link>,
    log: &mut dyn Write,
) {
    while let Ok((program, _)) = socket.accept() {
        match link(program, compositor) {
            Ok(link) => links.push(link),
            Err(error) => {
                let _ = writeln!(log, "omorfos-relay: a program could not be let in: {error}");
            }
        }
    }
}

/// Hands `compositor` a connection for `program`, with who the program is,
/// and joins the two.
fn link(program: UnixStream, compositor: BorrowedFd<'_>) -> io::Result<Link> {
    let found = rustix::net::sockopt::socket_peercred(&program).map_err(io::Error::from);
    let made = found.and_then(|found| UnixStream::pair().map(|pair| (found, pair)));
    made.and_then(|(found, (ours, theirs))| {
        let who = Program {
            pid: found.pid.as_raw_nonzero().get().unsigned_abs(),
            uid: found.uid.as_raw(),
            gid: found.gid.as_raw(),
        };
        relay::send(compositor, &Hand::Program(who), &[theirs.as_fd()])
            .and_then(|()| Link::new(program, ours))
    })
}

/// Does what `link`'s connections being as they are, `program` and
/// `compositor`, says to.
fn serve(link: &mut Link, program: PollFlags, compositor: PollFlags) -> Result<(), Ended> {
    for (ready, from, to) in [
        (program, Way::Up, Way::Down),
        (compositor, Way::Down, Way::Up),
    ] {
        // One gone, with no room to read what it left, is gone.
        let gone = PollFlags::HUP | PollFlags::ERR;
        if ready.intersects(gone) && !link.reads(from) {
            return Err(Ended::Closed);
        }
        let read = if ready.intersects(READABLE) {
            link.read(from)
        } else {
            Ok(())
        };
        read.and_then(|()| {
            if ready.contains(PollFlags::OUT) {
                link.write(to)
            } else {
                Ok(())
            }
        })?;
    }
    Ok(())
}
