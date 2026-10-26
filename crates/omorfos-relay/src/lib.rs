//! Stands between Wayland programs and the omorfos compositor (ADR 60 and
//! ADR 61).
//!
//! Programs connect to the relay, at the socket the holder hands it. For
//! each, the relay makes a connection to the compositor, hands the
//! compositor its end with who the program is, and passes every message
//! on, each read as it goes. When the compositor ends, a program whose
//! every object can be made again waits for the next, and has them made
//! again there; one that cannot is let go.

pub mod journal;
pub mod keep;
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
    /// The number the next program is given.
    next: u32,
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
            next: 1,
        }
    }

    /// How many programs are connected through the relay, waiting for a
    /// compositor or not.
    #[must_use]
    pub fn programs(&self) -> usize {
        self.links.len()
    }

    /// Waits up to `timeout`, or until something happens, and does what it
    /// says; says on `log` why a program's connection was ended, if it was
    /// for what it sent. Returns whether the relay is to go on: not once the
    /// holder has gone.
    pub fn step(&mut self, timeout: Option<Duration>, log: &mut dyn Write) -> bool {
        let joined: Vec<bool> = self
            .links
            .iter()
            .map(|link| link.compositor().is_some())
            .collect();
        let ready = self.wait(timeout);
        let mut ready = ready.into_iter();
        // What the holder hands changes what the rest are; they are read
        // again once that is taken.
        if ready
            .next()
            .is_some_and(|holder| holder.intersects(READABLE))
        {
            return self.hear_holder(log);
        }
        let readable =
            |ready: Option<PollFlags>| ready.is_some_and(|ready| ready.intersects(READABLE));
        let compositor_ready = self.compositor.as_ref().map(|_| readable(ready.next()));
        let socket_ready = compositor_ready
            .and(self.socket.as_ref())
            .map(|_| readable(ready.next()));
        // Programs first: the compositor gone closes each program's
        // connection to it as well as its own to the relay, and in whichever
        // order they are seen, a program losing it is then kept.
        let mut ended = Vec::new();
        let mut lost = false;
        for ((index, link), joined) in self.links.iter_mut().enumerate().zip(joined) {
            let program = ready.next().unwrap_or(PollFlags::empty());
            let compositor = joined.then(|| ready.next()).flatten();
            match serve(link, program, compositor) {
                Ok(()) => {}
                Err(Ended::Lost) => {
                    lost = true;
                    ended.push((index, Ended::Lost));
                }
                Err(why) => ended.push((index, why)),
            }
        }
        let gone = (lost || compositor_ready == Some(true)) && self.compositor_gone();
        if gone {
            // One that lost it is kept with the rest; one let go alone, by
            // a compositor still there, is not.
            ended.retain(|(_, why)| *why != Ended::Lost);
        }
        remove(&mut self.links, ended, log);
        if gone {
            self.lose_compositor();
        } else if socket_ready == Some(true) {
            let both = self.socket.as_ref().zip(self.compositor.as_ref());
            let (links, next) = (&mut self.links, &mut self.next);
            let admitted = both.map(|(socket, compositor)| {
                admit(socket, compositor.as_fd(), links, next, log);
            });
            let _ = admitted;
        }
        true
    }

    /// Waits up to `timeout` for any connection to be ready, and says how
    /// each is: the holder's, the compositor's and the socket while there
    /// is one, then each program's and, while it has one, its connection to
    /// the compositor.
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
            polled.push(PollFd::from_borrowed_fd(link.program(), program));
            if let Some(towards) = link.compositor() {
                let compositor = wanted(link.reads(Way::Down), link.writes(Way::Up));
                polled.push(PollFd::from_borrowed_fd(towards, compositor));
            }
        }
        let timeout = timeout.and_then(|timeout| Timespec::try_from(timeout).ok());
        // Interrupted, nothing is ready, and the caller steps again.
        let _ = poll(&mut polled, timeout.as_ref());
        polled.iter().map(PollFd::revents).collect()
    }

    /// Whether the compositor has gone: there is none, or its connection to
    /// the relay has closed.
    fn compositor_gone(&self) -> bool {
        self.compositor.as_ref().is_none_or(|compositor| {
            let mut polled = [PollFd::new(compositor, PollFlags::IN)];
            let _ = poll(&mut polled, Some(&Timespec::default()));
            let readable = polled
                .iter()
                .any(|polled| polled.revents().intersects(READABLE));
            readable && relay::receive(compositor.as_fd()).is_err()
        })
    }

    /// Takes in that the compositor has gone: each program that can be kept
    /// waits for the next; the rest are let go.
    fn lose_compositor(&mut self) {
        self.compositor = None;
        self.links
            .retain_mut(|link| link.compositor().is_none() || link.lose());
    }

    /// Takes what the holder has handed. Returns whether the holder is
    /// still there.
    fn hear_holder(&mut self, log: &mut dyn Write) -> bool {
        loop {
            match relay::receive(self.holder.as_fd()) {
                Ok(Some(received)) => self.take(received, log),
                Ok(None) => return true,
                Err(_) => return false,
            }
        }
    }

    /// Takes a hand from the holder.
    fn take(&mut self, received: Received, log: &mut dyn Write) {
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
                // One not yet seen to go has gone, as the holder starts
                // another only then.
                self.lose_compositor();
                let towards = compositor.as_fd();
                self.links.retain_mut(|link| rejoin(link, towards, log));
                self.compositor = Some(compositor);
            }
            _ => {}
        }
    }
}

/// Lets go of each link in `ended`, saying on `log` why where it was for
/// what the program sent.
fn remove(links: &mut Vec<Link>, ended: Vec<(usize, Ended)>, log: &mut dyn Write) {
    for (index, why) in ended.into_iter().rev() {
        links.remove(index);
        if let Ended::Broken(broken) = why {
            let _ = writeln!(log, "omorfos-relay: a program was let go, for {}", broken.0);
        }
    }
}

/// Takes each program waiting at `socket`, numbering each from `next`, and
/// hands `compositor` a connection for it, keeping each joined to its
/// connection in `links`.
fn admit(
    socket: &UnixListener,
    compositor: BorrowedFd<'_>,
    links: &mut Vec<Link>,
    next: &mut u32,
    log: &mut dyn Write,
) {
    while let Ok((program, _)) = socket.accept() {
        match link(program, *next, compositor) {
            Ok(link) => {
                links.push(link);
                *next = next.wrapping_add(1);
            }
            Err(error) => {
                let _ = writeln!(log, "omorfos-relay: a program could not be let in: {error}");
            }
        }
    }
}

/// Hands `compositor` a connection for `program`, numbered `number`, with who
/// the program is, and joins the two.
fn link(program: UnixStream, number: u32, compositor: BorrowedFd<'_>) -> io::Result<Link> {
    let found = rustix::net::sockopt::socket_peercred(&program).map_err(io::Error::from);
    let made = found.and_then(|found| UnixStream::pair().map(|pair| (found, pair)));
    made.and_then(|(found, (ours, theirs))| {
        let who = Program {
            pid: found.pid.as_raw_nonzero().get().unsigned_abs(),
            uid: found.uid.as_raw(),
            gid: found.gid.as_raw(),
            number: Some(number),
        };
        relay::send(compositor, &Hand::Program(who), &[theirs.as_fd()])
            .and_then(|()| Link::new(program, who, ours))
    })
}

/// Hands `compositor` a connection for the program `link` kept, and makes
/// what it had again there. Says, on `log` if not, whether it could be.
fn rejoin(link: &mut Link, compositor: BorrowedFd<'_>, log: &mut dyn Write) -> bool {
    let made = UnixStream::pair();
    let handed = made.and_then(|(ours, theirs)| {
        relay::send(compositor, &Hand::Program(link.who()), &[theirs.as_fd()]).map(|()| ours)
    });
    let joined = handed.map_err(Ended::from).and_then(|ours| link.join(ours));
    if joined.is_err() {
        let _ = writeln!(
            log,
            "omorfos-relay: a program could not be kept for the next compositor"
        );
    }
    joined.is_ok()
}

/// Does what `link`'s connections being as they are, `program` and, while it
/// has one, `compositor`, says to.
fn serve(link: &mut Link, program: PollFlags, compositor: Option<PollFlags>) -> Result<(), Ended> {
    // The program gone, with no room to read what it left, is gone.
    let gone = PollFlags::HUP | PollFlags::ERR;
    if program.intersects(gone) && !link.reads(Way::Up) {
        return Err(Ended::Closed);
    }
    let ways = [
        (Some(program), Way::Up, Way::Down),
        (compositor, Way::Down, Way::Up),
    ];
    for (ready, from, to) in ways {
        let Some(ready) = ready else {
            continue;
        };
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
