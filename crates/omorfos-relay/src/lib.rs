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
pub mod sandbox;
pub mod serials;
pub mod wire;

use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos_ipc::help::About;
use omorfos_ipc::relay::{self, Hand, Program, Received, Started};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::link::{Ended, Link, Way};
use crate::sandbox::Listened;

/// What it says of itself, when asked and in its manual page.
pub const ABOUT: About = About {
    name: "omorfos-relay",
    what: "Stands between Wayland programs and the omorfos compositor, so that the compositor \
           can be started again without closing them.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "It is started by omorfos-hold, which hands it what it serves: there is no need to start \
            it by hand.",
    version: env!("CARGO_PKG_VERSION"),
};

/// What a connection being ready to read is, or its other end gone.
const READABLE: PollFlags = PollFlags::IN.union(PollFlags::HUP).union(PollFlags::ERR);

/// How long the compositor is given to be seen gone once a program's
/// connection to it has closed with no error said. A compositor ending
/// closes its connections one at a time, and another process may hold a
/// copy of one for a moment, as one being started does. The program is
/// kept aside meanwhile, and the rest go on: a compositor can let a program
/// go while it goes on, and a program can have that happen again and
/// again.
const GRACE: Duration = Duration::from_millis(500);

/// The most sandboxes listened on at once: one for each program Flatpak has
/// running, which is far fewer. Each holds two descriptors, and a program
/// may make as many as it likes, so past this one made is let go.
const SANDBOXES: usize = 64;

/// How long the socket is not listened to once a program waiting at it
/// could not be taken, as when no descriptor is left: it stays ready, and
/// listened to it would be woken for at once, again and again.
pub(crate) const AGAIN: Duration = Duration::from_millis(100);

/// The relay, and every program's connection through it.
#[derive(Debug)]
pub struct Relay {
    /// The connection to the holder, which hands the relay what it serves.
    holder: OwnedFd,
    /// The socket programs connect to, once the holder has handed it.
    socket: Option<UnixListener>,
    /// The sandboxes programs have made, whose sockets the relay listens on.
    sandboxes: Vec<Listened>,
    /// The connection to the compositor, while there is one.
    compositor: Option<OwnedFd>,
    links: Vec<Link>,
    /// The number the next program is given.
    next: u32,
    /// Where the session is as a whole, as the compositor last said, for the
    /// next compositor.
    desk: Option<String>,
    /// The windows the compositor lists, by the identifier it gives each:
    /// which it is, where it is relayed.
    listed: BTreeMap<String, Option<(u32, u32)>>,
    /// Whether the compositor is still to be told that what was kept for it
    /// is ready to be shown.
    readying: bool,
    /// Whether the compositor has said it is going: each program's
    /// connection it closes from then is it going, and the program kept.
    going: bool,
    /// Until when the socket is not listened to, as a program waiting at
    /// it could not be taken; and whether that has been said since a
    /// program last was.
    paused: (Option<Instant>, bool),
    /// The programs whose connection to the compositor closed while it
    /// was still there, each until when it is waited for to be seen gone:
    /// kept with the rest if it is, let go if it is not.
    doubtful: Vec<(Link, Instant)>,
}

impl Relay {
    /// A relay that the holder hands what it serves over `holder`.
    #[must_use]
    pub const fn new(holder: OwnedFd) -> Self {
        Self {
            holder,
            socket: None,
            sandboxes: Vec::new(),
            compositor: None,
            links: Vec::new(),
            next: 1,
            desk: None,
            listed: BTreeMap::new(),
            readying: false,
            going: false,
            paused: (None, false),
            doubtful: Vec::new(),
        }
    }

    /// How many programs are connected through the relay, waiting for a
    /// compositor or not.
    #[must_use]
    pub fn programs(&self) -> usize {
        self.links.len() + self.doubtful.len()
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
        // What the compositor hands is taken first: which window it lists by
        // each identifier is what a program's list may wait on.
        let went = compositor_ready == Some(true) && !self.hear(log);
        let socket_ready = compositor_ready
            .and(self.socket.as_ref())
            .map(|_| readable(ready.next()));
        // Whether each sandbox has ended, and while there is a compositor,
        // whether a program waits to come in through it.
        let through: Vec<(bool, bool)> = self
            .sandboxes
            .iter()
            .map(|_| {
                let ended = readable(ready.next());
                let waiting = compositor_ready.is_some() && readable(ready.next());
                (ended, waiting)
            })
            .collect();
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
        let doubting = lost || !self.doubtful.is_empty();
        let gone = went || (doubting && self.going);
        if gone {
            // One that lost it is kept with the rest.
            ended.retain(|(_, why)| *why != Ended::Lost);
        }
        let until = Instant::now() + GRACE;
        let doubted = remove(&mut self.links, ended, log);
        self.doubtful
            .extend(doubted.into_iter().map(|link| (link, until)));
        self.doubt(gone, log);
        // Pictures of a window whose program has gone, or closed it, stop.
        if self.links.iter().any(Link::awaits_pictures) {
            let present: HashSet<(u32, u32)> = self.links.iter().flat_map(Link::windows).collect();
            for link in &mut self.links {
                link.windows_gone(&present);
            }
        }
        if gone {
            self.lose_compositor(log);
        } else {
            // A window shared anew is imported again by each program that
            // imported it.
            let renamed: Vec<_> = self.links.iter_mut().flat_map(Link::renamed).collect();
            for (before, now) in renamed {
                for link in &mut self.links {
                    link.exported(&before, &now);
                }
            }
            let (links, next) = (&mut self.links, &mut self.next);
            let compositor = self.compositor.as_ref().map(AsFd::as_fd);
            let socket = self.socket.as_ref().filter(|_| socket_ready == Some(true));
            let failed = Cell::new(false);
            let programs = socket
                .into_iter()
                .flat_map(|socket| waiting(socket, &failed));
            let waiting_inside = self.sandboxes.iter().zip(&through);
            let waiting_inside = waiting_inside.filter(|(_, (_, waiting))| *waiting);
            let inside = waiting_inside.flat_map(|(sandbox, _)| sandbox.waiting(&failed));
            let _ = compositor.map(|compositor| {
                admit(programs, false, compositor, links, next, log);
                admit(inside, true, compositor, links, next, log);
            });
            let tried = socket_ready == Some(true) || through.iter().any(|(_, waiting)| *waiting);
            self.pause(tried, failed.get(), log);
        }
        // A sandbox ended lets nothing more in; what came in stays.
        let mut ended = through.iter().map(|(ended, _)| *ended);
        self.sandboxes.retain(|_| !ended.next().unwrap_or_default());
        self.take_sandboxes(log);
        self.tell_ready();
        true
    }

    /// Stops listening to the socket, and to the sandboxes', for a moment
    /// if, `tried`, a program waiting at one could not be taken, `failed`,
    /// as no descriptor is left for any; and says so once, until they are
    /// tried and all waiting are taken.
    fn pause(&mut self, tried: bool, failed: bool, log: &mut dyn Write) {
        if !tried {
            return;
        }
        if failed && !self.paused.1 {
            let _ = writeln!(
                log,
                "omorfos-relay: a program could not be let in, as no descriptor is left; it is \
                 let in once one is free"
            );
        }
        self.paused = (failed.then(|| Instant::now() + AGAIN), failed);
    }

    /// Tells the compositor, once, that every program kept for it is ready
    /// to be shown, as soon as each is.
    fn tell_ready(&mut self) {
        if self.readying && self.links.iter().all(Link::ready) {
            self.readying = false;
            let compositor = self.compositor.as_ref();
            // A compositor that cannot be told goes, and is seen to.
            let _ = compositor.map(|towards| relay::send(towards.as_fd(), &Hand::Ready, &[]));
        }
    }

    /// Listens on each sandbox a program has made, as far as there is room
    /// for it; says on `log` of one there is not.
    fn take_sandboxes(&mut self, log: &mut dyn Write) {
        let made = self.links.iter_mut().flat_map(Link::sandboxes);
        for sandbox in made.collect::<Vec<_>>() {
            if self.sandboxes.len() < SANDBOXES {
                match sandbox.listen() {
                    Ok(listened) => self.sandboxes.push(listened),
                    Err(error) => {
                        let _ = writeln!(
                            log,
                            "omorfos-relay: a sandbox was let go, as it could not be listened on: \
                             {error}"
                        );
                    }
                }
            } else {
                let _ = writeln!(
                    log,
                    "omorfos-relay: a sandbox was let go, as {SANDBOXES} are listened on already"
                );
            }
        }
    }

    /// Waits up to `timeout` for any connection to be ready, and says how
    /// each is: the holder's, the compositor's and the socket while there
    /// is one, then each sandbox's end and, while there is a compositor, its
    /// socket, then each program's and, while it has one, its connection to
    /// the compositor.
    fn wait(&self, timeout: Option<Duration>) -> Vec<PollFlags> {
        let mut polled = vec![PollFd::new(&self.holder, PollFlags::IN)];
        // A socket not listened to is still among what is polled, asking
        // for nothing, so that each is where it is expected to be.
        let left = self
            .paused
            .0
            .map(|until| until.saturating_duration_since(Instant::now()));
        let paused = left.filter(|left| !left.is_zero());
        let socket = if paused.is_some() {
            PollFlags::empty()
        } else {
            PollFlags::IN
        };
        if let Some(compositor) = &self.compositor {
            polled.push(PollFd::new(compositor, PollFlags::IN));
            polled.extend(
                self.socket
                    .as_ref()
                    .map(|listening| PollFd::new(listening, socket)),
            );
        }
        // Woken too when the first program kept aside is to be decided on.
        let doubted = self.doubtful.iter().map(|(_, until)| *until).min();
        let doubted = doubted.map(|until| until.saturating_duration_since(Instant::now()));
        let paused = paused.into_iter().chain(doubted).min();
        let timeout = match (timeout, paused) {
            (Some(timeout), Some(paused)) => Some(timeout.min(paused)),
            (timeout, paused) => timeout.or(paused),
        };
        for sandbox in &self.sandboxes {
            polled.push(PollFd::from_borrowed_fd(sandbox.closing(), PollFlags::IN));
            if self.compositor.is_some() {
                // Not listened to while the socket is not: what keeps a
                // program from being taken is the relay's, not the socket's.
                polled.push(PollFd::from_borrowed_fd(sandbox.woken(), socket));
            }
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

    /// Decides on the programs kept aside: kept with the rest if the
    /// compositor has `gone`, as one ending closes its connections one at a
    /// time; let go, and said on `log` to be, once it has been there for as
    /// long as it is given to be seen gone. One that said it is going has
    /// said so before it closed any program's connection, and is heard to
    /// have, as what it hands is taken first in each step.
    fn doubt(&mut self, gone: bool, log: &mut dyn Write) {
        let now = Instant::now();
        for (link, until) in std::mem::take(&mut self.doubtful) {
            if gone {
                self.links.push(link);
            } else if until <= now {
                said_lost(&link, log);
            } else {
                self.doubtful.push((link, until));
            }
        }
    }

    /// Takes what the compositor has handed. Returns whether it is still
    /// there: not if there is none.
    fn hear(&mut self, log: &mut dyn Write) -> bool {
        let kept = Kept {
            links: &mut self.links,
            next: &mut self.next,
            desk: &mut self.desk,
            listed: &mut self.listed,
            going: &mut self.going,
        };
        let compositor = self.compositor.as_ref();
        compositor.is_some_and(|compositor| hear_compositor(compositor.as_fd(), kept, log))
    }

    /// Takes in that the compositor has gone: each program that can be kept
    /// waits for the next; the rest are let go, and said on `log` to be.
    fn lose_compositor(&mut self, log: &mut dyn Write) {
        self.compositor = None;
        self.going = false;
        let mut losing = Vec::new();
        self.links.retain_mut(|link| {
            let lost = link.compositor().is_some().then(|| link.lose());
            if let Some(Err(unkept)) = &lost {
                let _ = writeln!(
                    log,
                    "omorfos-relay: {} was let go, as it held {}, which cannot be made again yet",
                    named(link.who()),
                    unkept.join(", ")
                );
            }
            let kept = !matches!(lost, Some(Err(_)));
            if kept {
                losing.push(lost.is_some());
            }
            kept
        });
        // A window listed is listed again by the next compositor where it is
        // kept, and closed where it is not; one imported is imported again
        // once the next shares it, where it is kept.
        let kept: HashSet<(u32, u32)> = self.links.iter().flat_map(Link::windows).collect();
        let shared = self.links.iter().flat_map(Link::exports);
        let shared: HashSet<Vec<u8>> = shared.map(<[u8]>::to_vec).collect();
        for (link, losing) in self.links.iter_mut().zip(losing) {
            if losing {
                link.unlist(&self.listed, &kept);
                link.unimport(&shared);
            }
        }
        self.listed.clear();
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
                // another only then: a program kept aside as it went is
                // kept with the rest.
                self.doubt(true, log);
                self.lose_compositor(log);
                let towards = compositor.as_fd();
                // Where the session was first, then each program, and where
                // its windows were.
                let desk = self.desk.clone().map(Hand::Desk);
                let _ = desk.map(|desk| relay::send(towards, &desk, &[]));
                self.links.retain_mut(|link| rejoin(link, towards, log));
                // A compositor that cannot be told goes, and is seen to.
                let _ = relay::send(towards, &Hand::Kept, &[]);
                self.compositor = Some(compositor);
                self.readying = true;
                self.tell_ready();
            }
            _ => {}
        }
    }
}

/// What the relay keeps that the compositor hands it.
struct Kept<'a> {
    /// Every program's link.
    links: &'a mut Vec<Link>,
    /// The number the next program is given.
    next: &'a mut u32,
    /// Where the session is as a whole.
    desk: &'a mut Option<String>,
    /// The windows the compositor lists.
    listed: &'a mut BTreeMap<String, Option<(u32, u32)>>,
    /// Whether the compositor has said it is going.
    going: &'a mut bool,
}

/// Takes what `compositor` has handed into what the relay keeps, `kept`:
/// each part of the session it started is kept as programs are, and said on
/// `log` if it cannot be; where a window or the session is, kept for the
/// next. Returns whether the compositor is still there.
fn hear_compositor(compositor: BorrowedFd<'_>, kept: Kept<'_>, log: &mut dyn Write) -> bool {
    let Kept {
        links,
        next,
        desk,
        listed,
        going,
    } = kept;
    loop {
        match relay::receive(compositor) {
            Ok(Some(Received {
                hand: Some(Hand::Started(started)),
                fds,
            })) => {
                let part = fds.into_iter().next().map(UnixStream::from);
                let parted = part.map(|part| relayed(part, started, *next, compositor));
                match parted {
                    Some(Ok(link)) => {
                        links.push(link);
                        *next = next.wrapping_add(1);
                    }
                    Some(Err(error)) => {
                        let _ = writeln!(
                            log,
                            "omorfos-relay: a part of the session could not be let in: {error}"
                        );
                    }
                    None => {}
                }
            }
            Ok(Some(Received {
                hand:
                    Some(Hand::Placed {
                        number,
                        toplevel,
                        place,
                    }),
                ..
            })) => {
                let mut program = links.iter_mut();
                let program = program.find(|link| link.who().number == Some(number));
                let _ = program.map(|link| link.place(toplevel, place));
            }
            Ok(Some(Received {
                hand: Some(Hand::Desk(said)),
                ..
            })) => *desk = Some(said),
            Ok(Some(Received {
                hand: Some(Hand::Going),
                ..
            })) => *going = true,
            Ok(Some(Received {
                hand: Some(Hand::Listed { identifier, window }),
                ..
            })) => {
                for link in links.iter_mut() {
                    link.listed(&identifier, window);
                }
                listed.insert(identifier, window);
            }
            Ok(Some(Received {
                hand: Some(Hand::Unlisted(identifier)),
                ..
            })) => {
                for link in links.iter_mut() {
                    link.unlisted(&identifier);
                }
                listed.remove(&identifier);
            }
            // What is not known, or not whole, is passed over.
            Ok(Some(_)) => {}
            Ok(None) => return true,
            Err(_) => return false,
        }
    }
}

/// How the log names `who`: by the file its process runs, and its number,
/// as the compositor names programs. The name is only to tell the user
/// which it was, and anything in it that is not plain text is escaped.
fn named(who: Program) -> String {
    let file = std::fs::read_link(format!("/proc/{}/exe", who.pid)).ok();
    let name = file.as_deref().and_then(Path::file_name);
    let name = name.map(|name| name.to_string_lossy().escape_debug().to_string());
    format!(
        "{}, process {}",
        name.as_deref().unwrap_or("a program"),
        who.pid
    )
}

/// Lets go of each link in `ended`, saying on `log` why where it was for
/// what the program sent; returns those the compositor let go, to be
/// decided on.
fn remove(links: &mut Vec<Link>, ended: Vec<(usize, Ended)>, log: &mut dyn Write) -> Vec<Link> {
    let mut lost = Vec::new();
    for (index, why) in ended.into_iter().rev() {
        let link = links.remove(index);
        match why {
            Ended::Broken(broken) => {
                let _ = writeln!(
                    log,
                    "omorfos-relay: {} was let go, for {}",
                    named(link.who()),
                    broken.0
                );
            }
            Ended::Lost => lost.push(link),
            Ended::Closed => {}
        }
    }
    lost
}

/// Says on `log` that the program of `link` was let go, as the compositor
/// closed its connection while it went on.
fn said_lost(link: &Link, log: &mut dyn Write) {
    let _ = writeln!(
        log,
        "omorfos-relay: {} was let go, as the compositor closed its connection and went on, {}",
        named(link.who()),
        link.lost()
    );
}

/// Each program waiting at `socket`, without waiting for one; `failed` is
/// set if one waiting could not be taken.
fn waiting<'a>(
    socket: &'a UnixListener,
    failed: &'a Cell<bool>,
) -> impl Iterator<Item = UnixStream> + 'a {
    std::iter::from_fn(move || match socket.accept() {
        Ok((program, _)) => Some(program),
        Err(error) => {
            failed.set(error.kind() != io::ErrorKind::WouldBlock);
            None
        }
    })
}

/// Takes each of `programs`, numbering each from `next`, and hands
/// `compositor` a connection for it, as one inside a sandbox where they
/// came through one, `sandboxed`, keeping each joined to its connection in
/// `links`.
fn admit(
    programs: impl Iterator<Item = UnixStream>,
    sandboxed: bool,
    compositor: BorrowedFd<'_>,
    links: &mut Vec<Link>,
    next: &mut u32,
    log: &mut dyn Write,
) {
    for program in programs {
        match link(program, *next, sandboxed, compositor) {
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
/// the program is and whether it is inside a sandbox, `sandboxed`, and joins
/// the two.
fn link(
    program: UnixStream,
    number: u32,
    sandboxed: bool,
    compositor: BorrowedFd<'_>,
) -> io::Result<Link> {
    let found = rustix::net::sockopt::socket_peercred(&program).map_err(io::Error::from);
    let made = found.and_then(|found| UnixStream::pair().map(|pair| (found, pair)));
    made.and_then(|(found, (ours, theirs))| {
        let who = Program {
            pid: found.pid.as_raw_nonzero().get().unsigned_abs(),
            uid: found.uid.as_raw(),
            gid: found.gid.as_raw(),
            number: Some(number),
        };
        let hand = if sandboxed {
            Hand::Sandboxed(who)
        } else {
            Hand::Program(who)
        };
        let link = Link::new(program, who, ours);
        let link = link.map(|link| if sandboxed { link.in_sandbox() } else { link });
        relay::send(compositor, &hand, &[theirs.as_fd()]).and(link)
    })
}

/// Hands `compositor` a connection for the part of the session it `started`,
/// whose own is `part`, numbered `number`, with who it is and its roles, and
/// joins the two.
fn relayed(
    part: UnixStream,
    started: Started,
    number: u32,
    compositor: BorrowedFd<'_>,
) -> io::Result<Link> {
    let found = rustix::net::sockopt::socket_peercred(&part).map_err(io::Error::from);
    let made = found.and_then(|found| UnixStream::pair().map(|pair| (found, pair)));
    made.and_then(|(found, (ours, theirs))| {
        // Its process is what the compositor said: the connection was made
        // by the compositor, which started it.
        let who = Program {
            pid: started.pid,
            uid: found.uid.as_raw(),
            gid: found.gid.as_raw(),
            number: Some(number),
        };
        let hand = Hand::Part(who, started.roles);
        relay::send(compositor, &hand, &[theirs.as_fd()])
            .and_then(|()| Link::new(part, who, ours))
            .map(|link| link.with_roles(started.roles))
    })
}

/// Hands `compositor` a connection for the program `link` kept, and makes
/// what it had again there. Says, on `log` if not, whether it could be.
fn rejoin(link: &mut Link, compositor: BorrowedFd<'_>, log: &mut dyn Write) -> bool {
    let made = UnixStream::pair();
    let handed = made.and_then(|(ours, theirs)| {
        relay::send(compositor, &link.hand(), &[theirs.as_fd()]).map(|()| ours)
    });
    let joined = handed.map_err(Ended::from).and_then(|ours| link.join(ours));
    // Where its windows were, once it is there.
    for placed in link.placed() {
        let _ = relay::send(compositor, &placed, &[]);
    }
    if joined.is_err() {
        let _ = writeln!(
            log,
            "omorfos-relay: {} could not be kept for the next compositor",
            named(link.who())
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
    // What the compositor said and was held back is read again as if it had
    // just come.
    let ways = [
        (Some(program), Way::Up, Way::Down),
        (
            compositor.map(|ready| ready | link.again()),
            Way::Down,
            Way::Up,
        ),
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
