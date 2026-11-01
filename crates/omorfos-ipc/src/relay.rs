//! What `omorfos-relay` is handed, and hands the compositor (ADR 61).
//!
//! The holder starts the relay with one end of a connection as what it
//! reads from, and hands it over that connection the socket programs
//! connect to, then, for each compositor it starts, the relay's end of a
//! connection to it. Over that one the relay hands the compositor each
//! program that connects, with who the program is; and the compositor
//! hands the relay each part of the session it starts with roles that is
//! to be kept as programs are, which the relay hands back with them, and
//! again to each compositor after; where each window of a program is; and
//! which window it lists by each identifier.
//!
//! Each connection carries records, not a stream: a [`Hand`] and the
//! descriptors with it arrive whole and alone. A hand not known is passed
//! over, so that hands can be added and never changed.

use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{BorrowedFd, OwnedFd};

use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg, sendmsg, socketpair,
};

use crate::hold::{LONGEST, MOST};

/// Who a program is, as the relay found it from its connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program {
    /// Its process.
    pub pid: u32,
    /// The user it runs as.
    pub uid: u32,
    /// The group it runs as.
    pub gid: u32,
    /// The number the relay gives the program, which stays the same across
    /// compositors, where the relay gives one.
    pub number: Option<u32>,
}

/// A part of the session the compositor started with roles, to be kept by
/// the relay as programs are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Started {
    /// Its process.
    pub pid: u32,
    /// The roles it has, as the compositor counts them.
    pub roles: u8,
}

/// A hand, with the descriptors it says come with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hand {
    /// From the holder: the socket programs connect to.
    Socket,
    /// From the holder: the relay's end of a connection to a compositor
    /// started anew.
    Compositor,
    /// From the relay: the compositor's end of a program's connection, and
    /// who the program is.
    Program(Program),
    /// From the compositor: the relay's end of the connection of a part of
    /// the session it started, with its roles.
    Started(Started),
    /// From the relay: the compositor's end of the connection of a part of
    /// the session, who it is, and the roles the compositor that started it
    /// gave it.
    Part(Program, u8),
    /// From the relay: the compositor's end of the connection of a program
    /// that connected through a sandbox, and who the program is. It is
    /// inside the sandbox, as one that connected there to the compositor
    /// would be.
    Sandboxed(Program),
    /// From the relay: every program it kept for this compositor has been
    /// handed over.
    Kept,
    /// From the relay: every program it kept for this compositor has had
    /// all it had made again there, and the relay has sent all of it, so
    /// that the session may be shown as it was.
    Ready,
    /// From the compositor: where a window of the program the relay
    /// numbered `number` is, its toplevel being the program's object
    /// `toplevel`, as the compositor says it, which the relay keeps for the
    /// next; and from the relay, handed back to it after the program.
    Placed {
        /// The number the relay gave the program.
        number: u32,
        /// The program's number for the window's toplevel.
        toplevel: u32,
        /// Where it is, as the compositor says it, to the end of the line.
        place: String,
    },
    /// From the compositor: where the session is as a whole, as it says it,
    /// which the relay keeps for the next; and from the relay, handed back
    /// to it first.
    Desk(String),
    /// From the compositor: a window it lists for the programs that may
    /// see the list of windows, by the identifier it gives it there, which
    /// has no space in it; and, where the window's program is the relay's,
    /// the number the relay gave that program and the program's number for
    /// the window's toplevel. Handed before the programs are told of it.
    Listed {
        /// The window's identifier in the list.
        identifier: String,
        /// The program's number and its toplevel's, where it is relayed.
        window: Option<(u32, u32)>,
    },
    /// From the compositor: the window it listed by this identifier is
    /// listed no longer.
    Unlisted(String),
}

impl Hand {
    /// The hand as it is written.
    #[must_use]
    pub fn written(&self) -> String {
        match self {
            Self::Socket => "socket".to_owned(),
            Self::Compositor => "compositor".to_owned(),
            Self::Program(program) => format!("program {}", who(program, None)),
            Self::Started(started) => format!("started {} {}", started.pid, started.roles),
            Self::Part(program, roles) => format!("part {}", who(program, Some(*roles))),
            Self::Sandboxed(program) => format!("sandboxed {}", who(program, None)),
            Self::Kept => "kept".to_owned(),
            Self::Ready => "ready".to_owned(),
            Self::Placed {
                number,
                toplevel,
                place,
            } => format!("placed {number} {toplevel} {place}"),
            Self::Desk(desk) => format!("desk {desk}"),
            Self::Listed { identifier, window } => match window {
                Some((number, toplevel)) => format!("listed {identifier} {number} {toplevel}"),
                None => format!("listed {identifier}"),
            },
            Self::Unlisted(identifier) => format!("unlisted {identifier}"),
        }
    }

    /// The hand `text` writes, if it is one.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let mut words = text.split(' ');
        match words.next() {
            Some("socket") => Some(Self::Socket),
            Some("compositor") => Some(Self::Compositor),
            Some("program") => read_who(words, false).map(|(program, _)| Self::Program(program)),
            Some("started") => {
                let mut numbers = words.map(str::parse::<u32>);
                let pid = numbers.next()?.ok()?;
                let roles = u8::try_from(numbers.next()?.ok()?).ok()?;
                let whole = numbers.next().is_none();
                whole.then_some(Self::Started(Started { pid, roles }))
            }
            Some("part") => {
                let read = read_who(words, true);
                read.map(|(program, roles)| Self::Part(program, roles))
            }
            Some("sandboxed") => {
                read_who(words, false).map(|(program, _)| Self::Sandboxed(program))
            }
            Some("kept") => words.next().is_none().then_some(Self::Kept),
            Some("ready") => words.next().is_none().then_some(Self::Ready),
            Some("placed") => {
                let mut said = text.splitn(4, ' ').skip(1);
                let number = said.next()?.parse().ok()?;
                let toplevel = said.next()?.parse().ok()?;
                let place = said.next()?.to_owned();
                Some(Self::Placed {
                    number,
                    toplevel,
                    place,
                })
            }
            Some("desk") => text
                .split_once(' ')
                .map(|(_, desk)| Self::Desk(desk.to_owned())),
            Some("listed") => {
                let identifier = words.next().filter(|identifier| !identifier.is_empty())?;
                let mut numbers = words.map(str::parse::<u32>);
                let window = match (numbers.next(), numbers.next()) {
                    (None, _) => None,
                    (Some(number), Some(toplevel)) => Some((number.ok()?, toplevel.ok()?)),
                    (Some(_), None) => return None,
                };
                numbers.next().is_none().then(|| Self::Listed {
                    identifier: identifier.to_owned(),
                    window,
                })
            }
            Some("unlisted") => {
                let identifier = words.next().filter(|identifier| !identifier.is_empty())?;
                let whole = words.next().is_none();
                whole.then(|| Self::Unlisted(identifier.to_owned()))
            }
            _ => None,
        }
    }
}

/// Who `program` is as a hand writes it, with `roles` before its number
/// where there are some.
fn who(program: &Program, roles: Option<u8>) -> String {
    let roles = roles.map(|roles| format!(" {roles}")).unwrap_or_default();
    let number = program.number.map(|number| format!(" {number}"));
    let number = number.unwrap_or_default();
    format!(
        "{} {} {}{roles}{number}",
        program.pid, program.uid, program.gid
    )
}

/// Who a program is, as [`who`] writes it in `words`, and its roles, if it
/// is written `with_roles`: none otherwise.
fn read_who<'a>(words: impl Iterator<Item = &'a str>, with_roles: bool) -> Option<(Program, u8)> {
    let mut numbers = words.map(str::parse::<u32>);
    let mut number = || numbers.next().and_then(Result::ok);
    let (pid, uid, gid) = (number()?, number()?, number()?);
    let roles = if with_roles {
        u8::try_from(number()?).ok()?
    } else {
        0
    };
    let given = numbers.next().map(Result::ok);
    let program = Program {
        pid,
        uid,
        gid,
        number: given.flatten(),
    };
    let whole = given.is_none_or(|number| number.is_some()) && numbers.next().is_none();
    whole.then_some((program, roles))
}

/// What came over a connection: a hand, unless it was one not known, and
/// the descriptors with it.
#[derive(Debug)]
pub struct Received {
    /// The hand, if it is one known.
    pub hand: Option<Hand>,
    /// The descriptors that came with it.
    pub fds: Vec<OwnedFd>,
}

/// A new connection that carries records, both of its ends.
///
/// # Errors
///
/// Returns an error if the system will not make one.
pub fn pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let flags = SocketFlags::CLOEXEC;
    let made = socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, flags, None);
    made.map_err(io::Error::from)
}

/// Hands `hand` and the descriptors `fds` over `to`, as one record.
///
/// # Errors
///
/// Returns an error if there are more than [`MOST`] descriptors, or the
/// connection cannot be written to, as when the other end has closed.
pub fn send(to: BorrowedFd<'_>, hand: &Hand, fds: &[BorrowedFd<'_>]) -> io::Result<()> {
    if fds.len() > MOST {
        let too_many = "too many descriptors to hand over at once";
        return Err(io::Error::new(io::ErrorKind::InvalidInput, too_many));
    }
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MOST))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    // There is room for as many as may be handed.
    let _ = control.push(SendAncillaryMessage::ScmRights(fds));
    let text = hand.written();
    let bytes = [IoSlice::new(text.as_bytes())];
    sendmsg(to, &bytes, &mut control, SendFlags::NOSIGNAL)?;
    Ok(())
}

/// Takes the next record from `from`, without waiting: nothing if none has
/// come.
///
/// # Errors
///
/// Returns an error if the other end has closed, or the connection cannot
/// be read.
pub fn receive(from: BorrowedFd<'_>) -> io::Result<Option<Received>> {
    let mut space =
        [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MOST), ScmCredentials(1))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let mut bytes = [0; LONGEST];
    let chunks = &mut [IoSliceMut::new(&mut bytes)];
    let flags = RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT;
    let got = match recvmsg(from, chunks, &mut control, flags) {
        Ok(got) => got,
        Err(rustix::io::Errno::AGAIN) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut fds = Vec::new();
    for message in control.drain() {
        if let RecvAncillaryMessage::ScmRights(handed) = message {
            fds.extend(handed);
        }
    }
    if got.bytes == 0 && fds.is_empty() {
        let closed = "the other end has closed";
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, closed));
    }
    // A record cut short, being longer than any hand, is not one known.
    let whole = !got.flags.contains(rustix::net::ReturnFlags::TRUNC);
    let text = std::str::from_utf8(&bytes[..got.bytes]).ok();
    let hand = text.filter(|_| whole).and_then(Hand::read);
    Ok(Some(Received { hand, fds }))
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::fd::AsFd;

    use super::*;

    const PROGRAM: Hand = Hand::Program(Program {
        pid: 4321,
        uid: 1000,
        gid: 100,
        number: Some(7),
    });

    /// One from a relay that gave programs no number.
    const UNNUMBERED: Hand = Hand::Program(Program {
        pid: 4321,
        uid: 1000,
        gid: 100,
        number: None,
    });

    /// A part of the session started with the `clipboard` role, as the
    /// compositor hands it to the relay, and as the relay hands it on.
    const STARTED: Hand = Hand::Started(Started {
        pid: 4321,
        roles: 16,
    });
    const PART: Hand = Hand::Part(
        Program {
            pid: 4321,
            uid: 1000,
            gid: 100,
            number: Some(7),
        },
        16,
    );

    /// The program numbered 7, which connected through a sandbox.
    const SANDBOXED: Hand = Hand::Sandboxed(Program {
        pid: 4321,
        uid: 1000,
        gid: 100,
        number: Some(7),
    });

    /// Where a window of the program numbered 7, its toplevel 12, is, and
    /// where the session is, as the compositor says them.
    fn placed() -> Hand {
        Hand::Placed {
            number: 7,
            toplevel: 12,
            place: "1 10 20 640 480 m 0 eDP-1".to_owned(),
        }
    }

    fn desk() -> Hand {
        Hand::Desk("400 300 eDP-1 eDP-1:0".to_owned())
    }

    /// A window of the program numbered 7, its toplevel 12, listed, one of
    /// a program not relayed listed, and the first listed no longer.
    fn listed() -> [Hand; 3] {
        [
            Hand::Listed {
                identifier: "aB3".to_owned(),
                window: Some((7, 12)),
            },
            Hand::Listed {
                identifier: "cD4".to_owned(),
                window: None,
            },
            Hand::Unlisted("aB3".to_owned()),
        ]
    }

    #[test]
    fn each_hand_is_read_as_it_is_written() {
        let hands = [Hand::Socket, Hand::Compositor, PROGRAM, UNNUMBERED];
        let hands = hands
            .into_iter()
            .chain([STARTED, PART, SANDBOXED, Hand::Kept, Hand::Ready]);
        for hand in hands.chain([placed(), desk()]).chain(listed()) {
            assert_eq!(Hand::read(&hand.written()), Some(hand.clone()));
        }
        let written = listed().map(|hand| hand.written());
        assert_eq!(written, ["listed aB3 7 12", "listed cD4", "unlisted aB3"]);
        assert_eq!(placed().written(), "placed 7 12 1 10 20 640 480 m 0 eDP-1");
        assert_eq!(desk().written(), "desk 400 300 eDP-1 eDP-1:0");
        assert_eq!(PROGRAM.written(), "program 4321 1000 100 7");
        assert_eq!(UNNUMBERED.written(), "program 4321 1000 100");
        assert_eq!(STARTED.written(), "started 4321 16");
        assert_eq!(PART.written(), "part 4321 1000 100 16 7");
        assert_eq!(SANDBOXED.written(), "sandboxed 4321 1000 100 7");
        assert_eq!(Hand::Kept.written(), "kept");
        assert_eq!(Hand::Ready.written(), "ready");
    }

    #[test]
    fn a_hand_not_known_or_not_whole_is_not_read() {
        for text in [
            "",
            "socks",
            "program",
            "program 1",
            "program 1 2",
            "program 1 2 x",
            "program 1 2 3 x",
            "program 1 2 3 4 5",
            "program -1 2 3",
            "started",
            "started 1",
            "started x 1",
            "started 1 x",
            "started 1 256",
            "started 1 2 3",
            "part 1 2 3",
            "part 1 2 3 256 4",
            "part 1 2 3 4 5 6",
            "kept 1",
            "ready 1",
            "sandboxed",
            "sandboxed 1 2 3 4 5",
            "placed",
            "placed 1",
            "placed x 2 3",
            "placed 1 x 3",
            "placed 1 2",
            "desk",
            "listed",
            "listed  7 12",
            "listed aB3 7",
            "listed aB3 x 12",
            "listed aB3 7 x",
            "listed aB3 7 12 13",
            "unlisted",
            "unlisted aB3 7",
        ] {
            assert_eq!(Hand::read(text), None, "{text:?}");
        }
    }

    #[test]
    fn hands_arrive_one_at_a_time_with_their_descriptors() {
        let (ours, theirs) = pair().unwrap();
        let file = File::open("/dev/null").unwrap();
        send(ours.as_fd(), &PROGRAM, &[file.as_fd()]).unwrap();
        send(ours.as_fd(), &Hand::Compositor, &[]).unwrap();

        let first = receive(theirs.as_fd()).unwrap().unwrap();
        assert_eq!((first.hand, first.fds.len()), (Some(PROGRAM), 1));
        let second = receive(theirs.as_fd()).unwrap().unwrap();
        assert_eq!((second.hand, second.fds.len()), (Some(Hand::Compositor), 0));
        assert!(
            receive(theirs.as_fd()).unwrap().is_none(),
            "nothing more has come"
        );
    }

    #[test]
    fn a_record_not_known_is_passed_over_with_its_descriptors_kept() {
        let (ours, theirs) = pair().unwrap();
        let file = File::open("/dev/null").unwrap();
        let unknown = [IoSlice::new(b"newer")];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let fds = [file.as_fd()];
        assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
        sendmsg(&ours, &unknown, &mut control, SendFlags::empty()).unwrap();
        let received = receive(theirs.as_fd()).unwrap().unwrap();
        assert_eq!((received.hand, received.fds.len()), (None, 1));

        let long = vec![b'p'; LONGEST + 1];
        let mut control = SendAncillaryBuffer::default();
        sendmsg(
            &ours,
            &[IoSlice::new(&long)],
            &mut control,
            SendFlags::empty(),
        )
        .unwrap();
        assert!(
            receive(theirs.as_fd()).unwrap().unwrap().hand.is_none(),
            "cut short"
        );
    }

    #[test]
    fn who_handed_it_is_not_taken_for_a_descriptor() {
        let (ours, theirs) = pair().unwrap();
        // Told who sends, the kernel says so with everything sent.
        rustix::net::sockopt::set_socket_passcred(&theirs, true).unwrap();
        let file = File::open("/dev/null").unwrap();
        send(ours.as_fd(), &Hand::Socket, &[file.as_fd()]).unwrap();
        let received = receive(theirs.as_fd()).unwrap().unwrap();
        assert_eq!((received.hand, received.fds.len()), (Some(Hand::Socket), 1));
    }

    #[test]
    fn too_many_descriptors_are_not_handed() {
        let (ours, _theirs) = pair().unwrap();
        let file = File::open("/dev/null").unwrap();
        let fds = vec![file.as_fd(); MOST + 1];
        let refused = send(ours.as_fd(), &Hand::Socket, &fds).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn the_other_end_closing_is_said() {
        let (ours, theirs) = pair().unwrap();
        drop(ours);
        let closed = receive(theirs.as_fd()).unwrap_err();
        assert_eq!(closed.kind(), io::ErrorKind::UnexpectedEof);
        assert!(
            send(theirs.as_fd(), &Hand::Socket, &[]).is_err(),
            "nothing to hand to"
        );
    }

    #[test]
    fn what_is_not_a_connection_cannot_be_read() {
        let file = File::open("/dev/null").unwrap();
        assert!(receive(file.as_fd()).is_err());
    }
}
