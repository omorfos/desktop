//! What `omorfos-relay` is handed, and hands the compositor (ADR 61).
//!
//! The holder starts the relay with one end of a connection as what it
//! reads from, and hands it over that connection the socket programs
//! connect to, then, for each compositor it starts, the relay's end of a
//! connection to it. Over that one the relay hands the compositor each
//! program that connects, with who the program is.
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
}

/// A hand, with the descriptors it says come with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    /// From the holder: the socket programs connect to.
    Socket,
    /// From the holder: the relay's end of a connection to a compositor
    /// started anew.
    Compositor,
    /// From the relay: the compositor's end of a program's connection, and
    /// who the program is.
    Program(Program),
}

impl Hand {
    /// The hand as it is written.
    #[must_use]
    pub fn written(&self) -> String {
        match self {
            Self::Socket => "socket".to_owned(),
            Self::Compositor => "compositor".to_owned(),
            Self::Program(program) => {
                format!("program {} {} {}", program.pid, program.uid, program.gid)
            }
        }
    }

    /// The hand `text` writes, if it is one.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let mut words = text.split(' ');
        match words.next() {
            Some("socket") => Some(Self::Socket),
            Some("compositor") => Some(Self::Compositor),
            Some("program") => {
                let mut number = || words.next().and_then(|word| word.parse().ok());
                let (pid, uid, gid) = (number(), number(), number());
                let program = Program {
                    pid: pid?,
                    uid: uid?,
                    gid: gid?,
                };
                words.next().is_none().then_some(Self::Program(program))
            }
            _ => None,
        }
    }
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
    });

    #[test]
    fn each_hand_is_read_as_it_is_written() {
        for hand in [Hand::Socket, Hand::Compositor, PROGRAM] {
            assert_eq!(Hand::read(&hand.written()), Some(hand));
        }
        assert_eq!(PROGRAM.written(), "program 4321 1000 100");
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
            "program 1 2 3 4",
            "program -1 2 3",
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
