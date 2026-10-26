//! A program's connection through the relay: what the program sends is
//! read and passed on to the compositor, and what the compositor sends is
//! read and passed back, with the descriptors each message carries. What the
//! program makes is written down as it goes, so that when the compositor
//! ends the program can wait, and what it had be made again for the next
//! (ADR 61, step 2).
//!
//! Nothing waits and nothing grows without bound. Each way keeps at most
//! two of the longest messages read and not yet passed on, and 64 KiB to be
//! written; when that is full, nothing more is read from the side that
//! sends until the other side has taken some. A program that does not read
//! is so held to what the compositor does with a program that does not
//! read (ADR 45).

use std::collections::VecDeque;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use omorfos_ipc::relay::Program;
use rustix::io::Errno;
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};

use crate::journal::{self, Sending};
use crate::keep::Keeping;
use crate::objects::Objects;
use crate::wire::{self, Broken, HEADER, LONGEST};

/// How much of what is read is kept until it is passed on.
const READING: usize = 2 * LONGEST;

/// How much is kept to be written before nothing more is read for it.
const WRITING: usize = 16 * LONGEST;

/// The most descriptors kept on either side of a way.
const DESCRIPTORS: usize = 256;

/// The most descriptors sent or read at once, as libwayland sends them.
const AT_ONCE: usize = 28;

/// The display, and its event that says the program did what it may not.
const DISPLAY: u32 = 1;
const ERROR: u16 = 0;
/// And its event that says a number may be taken again.
const DELETE_ID: u16 = 1;

/// Which way a message goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// From the program to the compositor.
    Up,
    /// From the compositor to the program.
    Down,
}

/// Why a link has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// One side closed its connection, or it failed.
    Closed,
    /// One side sent what could not be passed on.
    Broken(Broken),
    /// The compositor's side closed, without having said the program did
    /// what it may not: the compositor has gone, or let the program go.
    Lost,
}

impl From<Broken> for Ended {
    fn from(broken: Broken) -> Self {
        Self::Broken(broken)
    }
}

impl From<io::Error> for Ended {
    fn from(_: io::Error) -> Self {
        Self::Closed
    }
}

/// What is on its way one way.
#[derive(Debug, Default)]
struct Passing {
    /// Read, and not yet a whole message.
    read: Vec<u8>,
    /// Read with it.
    read_fds: VecDeque<OwnedFd>,
    /// Read whole, and to be written.
    written: Vec<u8>,
    /// To be written, each with where in `written` its message starts.
    written_fds: VecDeque<(usize, Rc<OwnedFd>)>,
}

impl Passing {
    /// Puts `message` to be written, with its descriptors.
    fn push(&mut self, (bytes, fds): Sending) {
        let start = self.written.len();
        self.written.extend_from_slice(&bytes);
        self.written_fds
            .extend(fds.into_iter().map(|fd| (start, fd)));
    }
}

/// A program's connection, and the relay's to the compositor for it.
#[derive(Debug)]
pub struct Link {
    program: UnixStream,
    /// Who the program is, as the compositor is told.
    who: Program,
    compositor: Option<UnixStream>,
    objects: Objects,
    keeping: Keeping,
    /// Whether the compositor has said the program did what it may not.
    errored: bool,
    up: Passing,
    down: Passing,
}

impl Link {
    /// Joins `program`, which is `who`, to `compositor`. Neither is waited
    /// on.
    ///
    /// # Errors
    ///
    /// Returns an error if either cannot be kept from waiting.
    pub fn new(program: UnixStream, who: Program, compositor: UnixStream) -> io::Result<Self> {
        let unwaited = program.set_nonblocking(true);
        let unwaited = unwaited.and_then(|()| compositor.set_nonblocking(true));
        unwaited.map(|()| Self {
            program,
            who,
            compositor: Some(compositor),
            objects: Objects::default(),
            keeping: Keeping::default(),
            errored: false,
            up: Passing::default(),
            down: Passing::default(),
        })
    }

    /// Who the program is.
    #[must_use]
    pub const fn who(&self) -> Program {
        self.who
    }

    /// The program's connection.
    #[must_use]
    pub fn program(&self) -> BorrowedFd<'_> {
        self.program.as_fd()
    }

    /// The connection to the compositor for it, while there is one.
    #[must_use]
    pub fn compositor(&self) -> Option<BorrowedFd<'_>> {
        self.compositor.as_ref().map(AsFd::as_fd)
    }

    /// Whether what comes `way` is to be read now: not while as much is
    /// waiting to go on as is kept, nor what the program sends while it
    /// waits for the compositor.
    #[must_use]
    pub fn reads(&self, way: Way) -> bool {
        let room = self.passing(way).read.len() < READING;
        let open = match way {
            Way::Up => self.keeping.live(),
            Way::Down => self.compositor.is_some(),
        };
        room && open
    }

    /// Whether there is something to write `way`.
    #[must_use]
    pub fn writes(&self, way: Way) -> bool {
        let to = way == Way::Down || self.compositor.is_some();
        to && !self.passing(way).written.is_empty()
    }

    /// Reads what has come `way`, and passes on what it can.
    ///
    /// # Errors
    ///
    /// Returns why the link has ended, if it has: the side read closed, or
    /// sent what cannot be passed on. What was sent before it closed is
    /// passed on first, as far as it can be without waiting.
    pub fn read(&mut self, way: Way) -> Result<(), Ended> {
        let from = match way {
            Way::Up => Some(&self.program),
            Way::Down => self.compositor.as_ref(),
        };
        let passing = passing_mut(&mut self.up, &mut self.down, way);
        let open = from.map_or(Ok(true), |from| take_in(from, passing))?;
        let passed = self.pass(way);
        // What came before the end has been passed on, as far as it could
        // be without waiting.
        match (open, way) {
            (true, _) => passed,
            (false, Way::Down) if !self.errored => Err(Ended::Lost),
            (false, _) => Err(Ended::Closed),
        }
    }

    /// Writes what is waiting to go `way`, then passes on what that made
    /// room for.
    ///
    /// # Errors
    ///
    /// Returns why the link has ended, if it has.
    pub fn write(&mut self, way: Way) -> Result<(), Ended> {
        self.give_out(way)?;
        self.pass(way)
    }

    /// Takes in that the compositor has gone: the program waits for the
    /// next, told that the pointer and the keyboard have left its surfaces.
    /// Returns whether it can be kept: not if it holds what cannot be made
    /// again.
    pub fn lose(&mut self) -> bool {
        if !self.keeping.lost(&self.objects) {
            return false;
        }
        self.compositor = None;
        // What was on its way to the compositor is written down already.
        self.up.written.clear();
        self.up.written_fds.clear();
        self.down.read.clear();
        self.down.read_fds.clear();
        self.tell_program();
        let _ = self.give_out(Way::Down);
        true
    }

    /// Joins the program to a new compositor, `compositor`, and makes what
    /// it had again there.
    ///
    /// # Errors
    ///
    /// Returns why the link has ended, if it cannot be joined.
    pub fn join(&mut self, compositor: UnixStream) -> Result<(), Ended> {
        let unwaited = compositor.set_nonblocking(true);
        self.compositor = Some(compositor);
        self.errored = false;
        self.keeping.join();
        self.flush_keeping();
        unwaited
            .and_then(|()| self.give_out(Way::Up))
            .map_err(Ended::from)
    }

    /// Reads each whole message waiting `way`, keeps what it makes and
    /// destroys, moves it to be written, and writes what it can.
    fn pass(&mut self, way: Way) -> Result<(), Ended> {
        match way {
            Way::Up => self.pass_up()?,
            Way::Down => self.pass_down()?,
        }
        for freed in self.keeping.journal.freed() {
            self.down
                .push(journal::message(DISPLAY, DELETE_ID, &[freed]));
        }
        self.flush_keeping();
        self.give_out(Way::Up)?;
        self.give_out(Way::Down).map_err(Ended::from)
    }

    /// Passes on what the program has sent, while it is live.
    fn pass_up(&mut self) -> Result<(), Ended> {
        while self.keeping.live() {
            let Some(header) = wire::header(&self.up.read)? else {
                break;
            };
            if self.up.written.len() + header.size > WRITING {
                break;
            }
            let mut message = self.up.read[..header.size].to_vec();
            let read = self.objects.request(&message, header)?;
            if self.up.read_fds.len() < read.fds {
                return Err(Broken("a message without its descriptors").into());
            }
            let fds: Vec<Rc<OwnedFd>> = self.up.read_fds.drain(..read.fds).map(Rc::new).collect();
            let described = &read.interface.requests[usize::from(read.opcode)];
            self.keeping
                .journal
                .request(header, &message, &fds, &read, described);
            if self.keeping.request(&read, &mut message, described) {
                self.up.push((message, fds));
            }
            self.up.read.drain(..header.size);
        }
        Ok(())
    }

    /// Passes on what the compositor has sent, sifted while the relay
    /// catches up.
    fn pass_down(&mut self) -> Result<(), Ended> {
        while let Some(header) = wire::header(&self.down.read)? {
            if self.down.written.len() + header.size > WRITING {
                break;
            }
            let mut message = self.down.read[..header.size].to_vec();
            self.down.read.drain(..header.size);
            let header = self.keeping.before(&self.objects, header, &mut message)?;
            // What the relay is to tell the program as the compositor caught
            // up, globals new to it among them, goes before what follows.
            self.tell_program();
            let Some(header) = header else {
                continue;
            };
            let read = self.objects.event(&message, header)?;
            if self.down.read_fds.len() < read.fds {
                return Err(Broken("a message without its descriptors").into());
            }
            let fds: Vec<Rc<OwnedFd>> = self.down.read_fds.drain(..read.fds).map(Rc::new).collect();
            let body = message.get(HEADER..).unwrap_or_default();
            let kept = self.keeping.journal.event(header, body, &read);
            let passed = kept && self.keeping.after(&read, &message);
            self.errored |= header.object == DISPLAY && header.opcode == ERROR;
            self.tell_program();
            if passed {
                self.down.push((message, fds));
            }
        }
        Ok(())
    }

    /// Puts what the relay is to tell the program of its own accord to be
    /// written, in its place among what the compositor says.
    fn tell_program(&mut self) {
        for told in self.keeping.to_program.drain(..) {
            self.down.push(told);
        }
    }

    /// Puts what the relay is to send the compositor of its own accord to be
    /// written. What it is to tell the program is put so as it comes, in
    /// its place among what the compositor says.
    fn flush_keeping(&mut self) {
        for sending in self.keeping.to_compositor.drain(..) {
            self.up.push(sending);
        }
    }

    /// Writes what is waiting to go `way`, as much as is taken now.
    fn give_out(&mut self, way: Way) -> io::Result<()> {
        match way {
            Way::Up => {
                let up = &mut self.up;
                let compositor = self.compositor.as_ref();
                compositor.map_or(Ok(()), |compositor| give_out(compositor, up))
            }
            Way::Down => give_out(&self.program, &mut self.down),
        }
    }

    fn passing(&self, way: Way) -> &Passing {
        match way {
            Way::Up => &self.up,
            Way::Down => &self.down,
        }
    }
}

/// The way `way` of a link.
fn passing_mut<'a>(up: &'a mut Passing, down: &'a mut Passing, way: Way) -> &'a mut Passing {
    match way {
        Way::Up => up,
        Way::Down => down,
    }
}

/// Reads what `from` has sent into `passing`, as much as is kept. Returns
/// whether `from` is still open.
fn take_in(from: &UnixStream, passing: &mut Passing) -> Result<bool, Ended> {
    let had = passing.read.len();
    // With no room, nothing is read, which is not the other side closing.
    if had >= READING {
        return Ok(true);
    }
    passing.read.resize(READING, 0);
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(AT_ONCE))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let chunks = &mut [IoSliceMut::new(&mut passing.read[had..])];
    let flags = RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT;
    let (bytes, flags, open) = match recvmsg(from, chunks, &mut control, flags) {
        Ok(got) => (got.bytes, got.flags, got.bytes > 0),
        // Nothing yet, or it has failed, which is as good as closed.
        Err(error) => (0, ReturnFlags::empty(), error == Errno::AGAIN),
    };
    passing.read.truncate(had + bytes);
    for message in control.drain() {
        if let RecvAncillaryMessage::ScmRights(handed) = message {
            passing.read_fds.extend(handed);
        }
    }
    if flags.contains(ReturnFlags::CTRUNC) || passing.read_fds.len() > DESCRIPTORS {
        return Err(Broken("more descriptors than are kept").into());
    }
    Ok(open)
}

/// Writes what `passing` has to `to`, as much as it takes now, each
/// descriptor with or before the bytes of its message.
fn give_out(to: &UnixStream, passing: &mut Passing) -> io::Result<()> {
    while !passing.written.is_empty() {
        // Descriptors past those sent at once wait, and so do the bytes of
        // their messages.
        let waiting = passing.written_fds.get(AT_ONCE).map(|(start, _)| *start);
        let end = waiting.map_or(passing.written.len(), |start| start.max(1));
        let fds: Vec<BorrowedFd<'_>> = passing
            .written_fds
            .iter()
            .take(AT_ONCE)
            .map(|(_, fd)| fd.as_fd())
            .collect();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(AT_ONCE))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        // There is room for as many as are sent at once.
        let _ = control.push(SendAncillaryMessage::ScmRights(&fds));
        let bytes = [IoSlice::new(&passing.written[..end])];
        let flags = SendFlags::NOSIGNAL | SendFlags::DONTWAIT;
        let sent = match sendmsg(to, &bytes, &mut control, flags) {
            Ok(sent) => sent,
            Err(Errno::AGAIN) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let handed = fds.len();
        drop(fds);
        passing.written_fds.drain(..handed);
        passing.written.drain(..sent);
        for (start, _) in &mut passing.written_fds {
            *start = start.saturating_sub(sent);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::wire::tests::{message, text, words};

    /// Who the tests' programs are.
    const WHO: Program = Program {
        pid: 1,
        uid: 2,
        gid: 3,
        number: Some(4),
    };

    /// A link, with the program's end of its connection and the
    /// compositor's.
    fn linked() -> (Link, UnixStream, UnixStream) {
        let (program, relayed) = UnixStream::pair().unwrap();
        let (towards, compositor) = UnixStream::pair().unwrap();
        compositor.set_nonblocking(true).unwrap();
        program.set_nonblocking(true).unwrap();
        (
            Link::new(relayed, WHO, towards).unwrap(),
            program,
            compositor,
        )
    }

    /// Sends `bytes` with `fds` over `to`.
    fn send(to: &UnixStream, bytes: &[u8], fds: &[BorrowedFd<'_>]) {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(64))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        assert!(control.push(SendAncillaryMessage::ScmRights(fds)));
        let sent = sendmsg(to, &[IoSlice::new(bytes)], &mut control, SendFlags::empty());
        assert_eq!(sent.unwrap(), bytes.len());
    }

    /// What has come over `from`: its bytes, and how many descriptors.
    fn received(from: &UnixStream) -> (Vec<u8>, usize) {
        // Told who sends, the kernel says so beside any descriptors.
        rustix::net::sockopt::set_socket_passcred(from, true).unwrap();
        let mut bytes = Vec::new();
        let mut fds = 0;
        loop {
            let mut chunk = [0; 8192];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(64))];
            let mut control = RecvAncillaryBuffer::new(&mut space);
            let chunks = &mut [IoSliceMut::new(&mut chunk)];
            let got = recvmsg(from, chunks, &mut control, RecvFlags::DONTWAIT);
            let Some(got) = got.ok().filter(|got| got.bytes > 0) else {
                return (bytes, fds);
            };
            fds += control
                .drain()
                .map(|message| match message {
                    RecvAncillaryMessage::ScmRights(handed) => handed.count(),
                    _ => 0,
                })
                .sum::<usize>();
            bytes.extend_from_slice(&chunk[..got.bytes]);
        }
    }

    /// `wl_display.get_registry`, as the object 2.
    fn get_registry() -> Vec<u8> {
        message(1, 1, &words(&[2]))
    }

    /// Binds `wl_shm` as the object 3.
    fn bind_shm() -> Vec<u8> {
        let mut body = words(&[1]);
        body.extend(text("wl_shm"));
        body.extend(words(&[1, 3]));
        message(2, 0, &body)
    }

    #[test]
    fn what_a_program_sends_reaches_the_compositor_as_it_was_sent() {
        let (mut link, program, compositor) = linked();
        let mut sent = get_registry();
        sent.extend(bind_shm());
        send(&program, &sent, &[]);
        assert_eq!(link.read(Way::Up), Ok(()));
        assert_eq!(received(&compositor), (sent, 0));
        assert!(!link.writes(Way::Up), "all of it went");
    }

    #[test]
    fn what_the_compositor_sends_reaches_the_program() {
        let (mut link, program, compositor) = linked();
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        // wl_registry.global, of wl_shm.
        let mut body = words(&[7]);
        body.extend(text("wl_shm"));
        body.extend(words(&[2]));
        let global = message(2, 0, &body);
        send(&compositor, &global, &[]);
        assert_eq!(link.read(Way::Down), Ok(()));
        assert_eq!(received(&program).0, global);
    }

    #[test]
    fn descriptors_go_with_the_message_that_carries_them() {
        let (mut link, program, compositor) = linked();
        let mut sent = get_registry();
        sent.extend(bind_shm());
        // wl_shm.create_pool, with the memory's descriptor.
        let create_pool = message(3, 0, &words(&[4, 4096]));
        sent.extend(&create_pool);
        let memory = File::open("/dev/null").unwrap();
        send(&program, &sent, &[memory.as_fd()]);
        assert_eq!(link.read(Way::Up), Ok(()));
        assert_eq!(received(&compositor), (sent, 1));
    }

    #[test]
    fn a_message_in_pieces_is_passed_on_once_it_is_whole() {
        let (mut link, program, compositor) = linked();
        let sent = get_registry();
        send(&program, &sent[..5], &[]);
        link.read(Way::Up).unwrap();
        assert_eq!(received(&compositor).0, Vec::<u8>::new());
        send(&program, &sent[5..], &[]);
        link.read(Way::Up).unwrap();
        assert_eq!(received(&compositor).0, sent);
    }

    #[test]
    fn what_cannot_be_passed_on_ends_the_link() {
        let (mut link, program, _compositor) = linked();
        send(&program, &message(9, 0, &[]), &[]);
        let broken = |reason| Err(Ended::Broken(Broken(reason)));
        assert_eq!(link.read(Way::Up), broken("a request to no object"));

        let (mut link, program, _compositor) = linked();
        let mut missized = message(1, 1, &[]);
        missized[4..8].copy_from_slice(&(6_u32 << 16).to_ne_bytes());
        send(&program, &missized, &[]);
        assert_eq!(
            link.read(Way::Up),
            broken("a message of a size no message has")
        );

        let (mut link, program, _compositor) = linked();
        let mut sent = get_registry();
        sent.extend(bind_shm());
        sent.extend(message(3, 0, &words(&[4, 4096])));
        send(&program, &sent, &[]);
        assert_eq!(
            link.read(Way::Up),
            broken("a message without its descriptors")
        );
    }

    #[test]
    fn a_side_closing_ends_the_link_once_what_it_sent_is_passed_on() {
        let (mut link, program, compositor) = linked();
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        // wl_display.delete_id, then the compositor goes, which is not the
        // program's end.
        let deleted = message(1, 1, &words(&[5]));
        send(&compositor, &deleted, &[]);
        drop(compositor);
        link.read(Way::Down).unwrap();
        assert_eq!(link.read(Way::Down), Err(Ended::Lost));
        assert_eq!(received(&program).0, deleted);

        // Having said the program did what it may not, it is.
        let (mut link, program, compositor) = linked();
        let mut error = words(&[1, 0]);
        error.extend(text("no"));
        send(&compositor, &message(1, 0, &error), &[]);
        drop(compositor);
        link.read(Way::Down).unwrap();
        assert_eq!(link.read(Way::Down), Err(Ended::Closed));
        drop(program);

        let (mut link, program, _compositor) = linked();
        drop(program);
        assert_eq!(link.read(Way::Up), Err(Ended::Closed));
    }

    #[test]
    fn what_waits_to_be_written_is_written_when_there_is_room() {
        let (mut link, program, compositor) = linked();
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        let deleted = message(1, 1, &words(&[5]));
        send(&compositor, &deleted, &[]);
        link.read(Way::Down).unwrap();
        assert_eq!(
            (link.write(Way::Down), link.write(Way::Up)),
            (Ok(()), Ok(()))
        );
        assert_eq!(received(&program).0, deleted);

        drop(program);
        link.down.written.extend(&deleted);
        assert_eq!(link.write(Way::Down), Err(Ended::Closed), "to no one");
    }

    #[test]
    fn a_program_that_does_not_read_stops_the_compositor_being_read() {
        let (mut link, program, compositor) = linked();
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        // wl_display.delete_id, of numbers never taken, more than is kept to
        // be written while the program's end takes none.
        let deleted = message(1, 1, &words(&[999]));
        let mut sent = 0;
        while (&compositor).write(&deleted).is_ok() && sent < 40_000 {
            sent += 1;
            let _ = link.read(Way::Down);
        }
        assert!(link.writes(Way::Down));
        assert!(!link.reads(Way::Down), "nothing more read for now");
        drop(program);
    }

    #[test]
    fn a_compositor_that_sends_what_cannot_be_read_ends_the_link() {
        let broken = |reason| Err(Ended::Broken(Broken(reason)));
        let (mut link, _program, compositor) = linked();
        let mut missized = message(1, 1, &[]);
        missized[4..8].copy_from_slice(&(6_u32 << 16).to_ne_bytes());
        send(&compositor, &missized, &[]);
        assert_eq!(
            link.read(Way::Down),
            broken("a message of a size no message has")
        );

        let (mut link, _program, compositor) = linked();
        send(&compositor, &message(99, 0, &[]), &[]);
        assert_eq!(link.read(Way::Down), broken("an event to no object"));
    }

    #[test]
    fn a_program_is_kept_through_its_compositor_ending_and_joined_to_the_next() {
        let (mut link, program, compositor) = linked();
        // A registry, offered a seat, the seat bound, and its pointer.
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        let mut seat = words(&[1]);
        seat.extend(text("wl_seat"));
        seat.extend(words(&[7]));
        send(&compositor, &message(2, 0, &seat), &[]);
        link.read(Way::Down).unwrap();
        let mut bind = words(&[1]);
        bind.extend(text("wl_seat"));
        bind.extend(words(&[7, 3]));
        let mut bound = message(2, 0, &bind);
        bound.extend(message(3, 0, &words(&[4])));
        send(&program, &bound, &[]);
        link.read(Way::Up).unwrap();
        // The pointer enters a surface, and the compositor goes.
        send(&compositor, &message(4, 0, &words(&[10, 9, 0, 0])), &[]);
        link.read(Way::Down).unwrap();
        drop(compositor);
        assert_eq!(link.read(Way::Down), Err(Ended::Lost));
        received(&program);
        assert!(link.lose());
        let mut left = message(4, 1, &words(&[10, 9]));
        left.extend(message(4, 5, &[]));
        assert_eq!(received(&program).0, left, "told it left, and the frame");

        // The next makes the registry again, offers the seat under another
        // name and a shm the program has not been told of, and answers the
        // relay's sync.
        let (towards, next) = UnixStream::pair().unwrap();
        next.set_nonblocking(true).unwrap();
        link.join(towards).unwrap();
        let mut expected = get_registry();
        expected.extend(message(1, 0, &words(&[3])));
        assert_eq!(received(&next).0, expected);
        let mut global = words(&[5]);
        global.extend(text("wl_seat"));
        global.extend(words(&[7]));
        let mut shm = words(&[6]);
        shm.extend(text("wl_shm"));
        shm.extend(words(&[1]));
        let mut said = message(2, 0, &global);
        said.extend(message(2, 0, &shm));
        said.extend(message(3, 0, &words(&[0])));
        send(&next, &said, &[]);
        link.read(Way::Down).unwrap();
        assert_eq!(
            received(&program).0,
            message(2, 0, &shm),
            "the shm offered, as new to it"
        );
        let mut rebound = words(&[5]);
        rebound.extend(text("wl_seat"));
        rebound.extend(words(&[7, 3]));
        let mut remade = message(2, 0, &rebound);
        remade.extend(message(3, 0, &words(&[4])));
        remade.extend(message(1, 0, &words(&[5])));
        assert_eq!(
            received(&next).0,
            remade,
            "bound by its new name, and the rest made"
        );
    }

    #[test]
    fn an_ack_of_a_configure_never_sent_is_kept_from_the_compositor() {
        let (mut link, program, compositor) = linked();
        let mut sent = get_registry();
        let mut bind = words(&[1]);
        bind.extend(text("xdg_wm_base"));
        bind.extend(words(&[6, 3]));
        sent.extend(message(2, 0, &bind));
        let mut bind = words(&[2]);
        bind.extend(text("wl_compositor"));
        bind.extend(words(&[6, 4]));
        sent.extend(message(2, 0, &bind));
        sent.extend(message(4, 0, &words(&[5])));
        sent.extend(message(3, 2, &words(&[6, 5])));
        send(&program, &sent, &[]);
        link.read(Way::Up).unwrap();
        assert_eq!(received(&compositor).0, sent);
        send(&program, &message(6, 4, &words(&[9])), &[]);
        link.read(Way::Up).unwrap();
        assert_eq!(received(&compositor).0, Vec::<u8>::new());
    }

    #[test]
    fn a_program_whose_global_the_next_compositor_does_not_offer_is_let_go() {
        let (mut link, program, compositor) = linked();
        send(&program, &get_registry(), &[]);
        link.read(Way::Up).unwrap();
        let mut seat = words(&[1]);
        seat.extend(text("wl_seat"));
        seat.extend(words(&[7]));
        send(&compositor, &message(2, 0, &seat), &[]);
        link.read(Way::Down).unwrap();
        let mut bind = words(&[1]);
        bind.extend(text("wl_seat"));
        bind.extend(words(&[7, 3]));
        send(&program, &message(2, 0, &bind), &[]);
        link.read(Way::Up).unwrap();
        drop(compositor);
        let _ = link.read(Way::Down);
        assert!(link.lose());
        let (towards, next) = UnixStream::pair().unwrap();
        link.join(towards).unwrap();
        // No seat is offered: the relay's sync is answered at once.
        send(&next, &message(3, 0, &words(&[0])), &[]);
        let gone = Err(Ended::Broken(Broken("a global that is no longer offered")));
        assert_eq!(link.read(Way::Down), gone);
        drop(program);
    }

    #[test]
    fn an_event_without_the_descriptor_it_carries_ends_the_link() {
        let (mut link, program, compositor) = linked();
        let mut sent = get_registry();
        let mut bind = words(&[1]);
        bind.extend(text("wl_seat"));
        bind.extend(words(&[7, 3]));
        sent.extend(message(2, 0, &bind));
        sent.extend(message(3, 1, &words(&[4])));
        send(&program, &sent, &[]);
        link.read(Way::Up).unwrap();
        // wl_keyboard.keymap, its descriptor missing.
        send(&compositor, &message(4, 0, &words(&[1, 4096])), &[]);
        let missing = Err(Ended::Broken(Broken("a message without its descriptors")));
        assert_eq!(link.read(Way::Down), missing);
    }

    #[test]
    fn a_number_withheld_is_given_back_once_nothing_names_it() {
        let (mut link, program, compositor) = linked();
        let memory = File::open("/dev/null").unwrap();
        let mut sent = get_registry();
        let mut bind = words(&[1]);
        bind.extend(text("wl_compositor"));
        bind.extend(words(&[6, 3]));
        sent.extend(message(2, 0, &bind));
        sent.extend(message(3, 0, &words(&[4])));
        send(&program, &sent, &[]);
        link.read(Way::Up).unwrap();
        let mut shm = words(&[2]);
        shm.extend(text("wl_shm"));
        shm.extend(words(&[1, 5]));
        let mut sent = message(2, 0, &shm);
        sent.extend(message(5, 0, &words(&[6, 4096])));
        sent.extend(message(6, 0, &words(&[7, 0, 1, 1, 4, 0])));
        sent.extend(message(4, 1, &words(&[7, 0, 0])));
        sent.extend(message(4, 6, &[]));
        sent.extend(message(7, 0, &[]));
        send(&program, &sent, &[memory.as_fd()]);
        link.read(Way::Up).unwrap();
        // The compositor frees the buffer's number; it is still shown.
        send(&compositor, &message(1, 1, &words(&[7])), &[]);
        link.read(Way::Down).unwrap();
        let _ = received(&program);
        // Another shown, the number is given back.
        let mut sent = message(6, 0, &words(&[8, 0, 1, 1, 4, 0]));
        sent.extend(message(4, 1, &words(&[8, 0, 0])));
        sent.extend(message(4, 6, &[]));
        send(&program, &sent, &[]);
        link.read(Way::Up).unwrap();
        assert_eq!(received(&program).0, message(1, 1, &words(&[7])));
    }

    #[test]
    fn a_compositor_gone_cannot_be_written_to() {
        let (mut link, program, compositor) = linked();
        drop(compositor);
        send(&program, &get_registry(), &[]);
        assert_eq!(link.read(Way::Up), Err(Ended::Closed));
    }

    #[test]
    fn a_compositor_that_does_not_read_stops_the_program_being_read() {
        let (mut link, program, compositor) = linked();
        program.set_nonblocking(false).unwrap();
        let mut syncs = Vec::new();
        // wl_display.sync, each making a callback of its own.
        for number in 2..30_000 {
            syncs.extend(message(1, 0, &words(&[number])));
        }
        let writer = std::thread::spawn(move || {
            let _ = (&program).write_all(&syncs);
            program
        });
        while link.reads(Way::Up) {
            link.read(Way::Up).unwrap();
        }
        assert!(link.writes(Way::Up));
        // With no room, a read reads nothing, and is not taken for an end.
        assert_eq!(link.read(Way::Up), Ok(()));

        // Once the compositor reads, the program is read again.
        let mut all = Vec::new();
        let mut chunk = vec![0; 65536];
        while all.len() < 29_998 * 12 {
            if let Ok(read) = (&compositor).read(&mut chunk) {
                all.extend_from_slice(&chunk[..read]);
            } else {
                link.write(Way::Up).unwrap();
                link.read(Way::Up).unwrap();
            }
        }
        let program = writer.join().unwrap();
        drop(program);
        assert_eq!(all.len(), 29_998 * 12);
    }

    #[test]
    fn descriptors_past_those_sent_at_once_wait_with_their_messages() {
        let (towards, compositor) = UnixStream::pair().unwrap();
        compositor.set_nonblocking(true).unwrap();
        rustix::net::sockopt::set_socket_passcred(&compositor, true).unwrap();
        let memory = File::open("/dev/null").unwrap();
        let mut passing = Passing::default();
        for message in 0..30 {
            passing.written.extend(words(&[message]));
            let fd = Rc::new(memory.try_clone().unwrap().into());
            passing
                .written_fds
                .push_back((usize::try_from(message).unwrap() * 4, fd));
        }
        give_out(&towards, &mut passing).unwrap();
        assert_eq!(passing.written, b"");

        let mut chunk = [0; 1024];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(64))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let chunks = &mut [IoSliceMut::new(&mut chunk)];
        let first = recvmsg(&compositor, chunks, &mut control, RecvFlags::empty()).unwrap();
        let handed: usize = control
            .drain()
            .map(|message| match message {
                RecvAncillaryMessage::ScmRights(handed) => handed.count(),
                _ => 0,
            })
            .sum();
        assert_eq!((first.bytes, handed), (AT_ONCE * 4, AT_ONCE));
        assert_eq!(received(&compositor), (words(&[28, 29]), 2));
    }

    #[test]
    fn a_program_may_not_send_more_descriptors_than_are_kept() {
        let (mut link, program, _compositor) = linked();
        let memory = File::open("/dev/null").unwrap();
        // Far more than there is room for, which the kernel cuts short.
        let fds = vec![memory.as_fd(); 2 * AT_ONCE];
        send(&program, &[0], &fds);
        let too_many = Err(Ended::Broken(Broken("more descriptors than are kept")));
        assert_eq!(link.read(Way::Up), too_many, "at once");

        let (mut link, program, _compositor) = linked();
        let fds = vec![memory.as_fd(); AT_ONCE];
        // The start of the longest message, which never comes whole.
        let mut start = message(1, 0, &[]);
        start[4..8].copy_from_slice(&(4096_u32 << 16).to_ne_bytes());
        send(&program, &start, &fds);
        for _ in 0..DESCRIPTORS / AT_ONCE {
            send(&program, &[0], &fds);
        }
        let mut read = Ok(());
        while read.is_ok() {
            read = link.read(Way::Up);
        }
        assert_eq!(read, too_many, "in all");
    }

    #[test]
    fn who_sent_it_is_not_taken_for_a_descriptor() {
        let (program, relayed) = UnixStream::pair().unwrap();
        let (towards, compositor) = UnixStream::pair().unwrap();
        rustix::net::sockopt::set_socket_passcred(&relayed, true).unwrap();
        let mut link = Link::new(relayed, WHO, towards).unwrap();
        send(&program, &get_registry(), &[]);
        assert_eq!(link.read(Way::Up), Ok(()));
        compositor.set_nonblocking(true).unwrap();
        assert_eq!(received(&compositor).0, get_registry());
    }

    #[test]
    fn its_connections_are_there_to_be_waited_on() {
        let (link, program, compositor) = linked();
        let _ = (program, compositor);
        let compositor = link.compositor().map(|fd| fd.as_raw_fd());
        assert_ne!(Some(link.program().as_raw_fd()), compositor);
        assert_eq!(link.who(), WHO);
        assert!(link.reads(Way::Down));
        assert!(!link.writes(Way::Down));
    }
}
