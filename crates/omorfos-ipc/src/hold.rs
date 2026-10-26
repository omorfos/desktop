//! What `omorfos-hold` and the compositor say to each other (ADR 60).
//!
//! The holder starts the compositor with one end of a connection as what
//! it reads from. Over it the holder hands, once, the socket programs
//! connect to, or the compositor's connection to the relay with it (ADR
//! 61), and what it has been [`Told`] by the compositors before;
//! back along it the compositor says [`Word`]s as things change.
//!
//! A holder and a compositor of different versions meet whenever either is
//! installed, so both sides pass over what they do not know: words are
//! added, and never changed.

use std::fmt::Write as _;
use std::io::{self, IoSlice, IoSliceMut, Write as _};
use std::mem::MaybeUninit;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;

use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};

/// The most a hand, or a word, may be, in bytes.
pub const LONGEST: usize = 1024;

/// The most descriptors one hand carries.
pub const MOST: usize = 8;

/// What the holder tells a compositor it starts, of the compositors before
/// it in the same login.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a line of its own, told or not, and none depends on another"
)]
pub struct Told {
    /// Another compositor has been started in this login before, which
    /// started what starts with the session: it is not to be started twice.
    pub again: bool,
    /// The session was locked when the last compositor ended. It is to
    /// come up locked.
    pub locked: bool,
    /// The number the session's X server had, which the next is to have.
    pub display: Option<u32>,
    /// The compositor is a version older than the one installed, which
    /// would not come up: the user is to be told that a fix meant to be
    /// in is not.
    pub older: bool,
    /// Programs reach the compositor through the relay (ADR 61): what is
    /// handed with this is the compositor's end of its connection to the
    /// relay, and then the socket, whose name alone it is to take.
    pub relayed: bool,
}

/// What a compositor says to the holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Word {
    /// It is serving the socket.
    Up,
    /// The session is locked.
    Locked,
    /// The session is no longer locked.
    Unlocked,
    /// The session's X server has this number.
    Display(u32),
    /// It is about to end, to be started again, as a new version is.
    SteppingDown,
}

impl Told {
    /// What the holder writes, a line for each thing told and an empty
    /// line after them.
    #[must_use]
    pub fn written(&self) -> String {
        let mut text = String::new();
        if self.again {
            text.push_str("again\n");
        }
        if self.locked {
            text.push_str("locked\n");
        }
        if let Some(display) = self.display {
            let _ = writeln!(text, "display {display}");
        }
        if self.older {
            text.push_str("older\n");
        }
        if self.relayed {
            text.push_str("relayed\n");
        }
        text.push('\n');
        text
    }

    /// What `text` tells, passing over any line it does not know.
    #[must_use]
    pub fn read(text: &str) -> Self {
        text.lines().fold(Self::default(), |told, line| match line {
            "again" => Self {
                again: true,
                ..told
            },
            "locked" => Self {
                locked: true,
                ..told
            },
            "older" => Self {
                older: true,
                ..told
            },
            "relayed" => Self {
                relayed: true,
                ..told
            },
            _ => Self {
                display: number(line, "display ").or(told.display),
                ..told
            },
        })
    }
}

impl Word {
    /// The line the compositor writes.
    #[must_use]
    pub fn line(self) -> String {
        match self {
            Self::Up => "up\n".to_owned(),
            Self::Locked => "locked\n".to_owned(),
            Self::Unlocked => "unlocked\n".to_owned(),
            Self::Display(display) => format!("display {display}\n"),
            Self::SteppingDown => "stepping down\n".to_owned(),
        }
    }

    /// The word `line` says, if it is one known here.
    #[must_use]
    pub fn read(line: &str) -> Option<Self> {
        match line {
            "up" => Some(Self::Up),
            "locked" => Some(Self::Locked),
            "unlocked" => Some(Self::Unlocked),
            "stepping down" => Some(Self::SteppingDown),
            _ => number(line, "display ").map(Self::Display),
        }
    }
}

/// The number `line` gives after `before`, if it is one.
fn number(line: &str, before: &str) -> Option<u32> {
    line.strip_prefix(before)
        .and_then(|number| number.parse().ok())
}

/// Hands `text` and the descriptors `fds` over `to`, at once.
///
/// # Errors
///
/// Returns an error if `text` is longer than [`LONGEST`], there are more
/// than [`MOST`] descriptors, or the connection cannot be written to.
pub fn give(to: &UnixStream, text: &str, fds: &[BorrowedFd<'_>]) -> io::Result<()> {
    if text.len() > LONGEST || fds.len() > MOST {
        let too_much = "too much to hand over at once";
        return Err(io::Error::new(io::ErrorKind::InvalidInput, too_much));
    }
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MOST))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    // There is room for as many as may be handed.
    let _ = control.push(SendAncillaryMessage::ScmRights(fds));
    let bytes = text.as_bytes();
    let sent = sendmsg(
        to,
        &[IoSlice::new(bytes)],
        &mut control,
        SendFlags::NOSIGNAL,
    )?;
    // The descriptors went with the first of it; what did not fit goes
    // after.
    (&mut &*to).write_all(&bytes[sent..])
}

/// Takes what was handed over `from`: the text, up to the empty line that
/// ends it, and the descriptors with it.
///
/// # Errors
///
/// Returns an error if the connection cannot be read, ends before the
/// empty line, or carries more than [`LONGEST`] bytes before it.
pub fn take(from: &UnixStream) -> io::Result<(String, Vec<OwnedFd>)> {
    let mut text = Vec::new();
    let mut fds = Vec::new();
    let mut space =
        [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MOST), ScmCredentials(1))];
    let mut chunk = [0; LONGEST + 1];
    loop {
        if let Some(end) = ended(&text) {
            text.truncate(end);
            return Ok((String::from_utf8_lossy(&text).into_owned(), fds));
        }
        if text.len() > LONGEST {
            let long = "what was handed over runs on past its end";
            return Err(io::Error::new(io::ErrorKind::InvalidData, long));
        }
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let chunks = &mut [IoSliceMut::new(&mut chunk)];
        let got = recvmsg(from, chunks, &mut control, RecvFlags::CMSG_CLOEXEC)?;
        // Whatever else comes with it, such as who sent it, is not asked
        // for.
        for message in control.drain() {
            if let RecvAncillaryMessage::ScmRights(handed) = message {
                fds.extend(handed);
            }
        }
        if got.bytes == 0 {
            let early = "the connection closed before all of it was handed over";
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, early));
        }
        text.extend_from_slice(&chunk[..got.bytes]);
    }
}

/// Where the text handed over ends, after the empty line, if it has.
fn ended(text: &[u8]) -> Option<usize> {
    if text.first() == Some(&b'\n') {
        return Some(1);
    }
    let pair = text.windows(2).position(|pair| pair == b"\n\n");
    pair.map(|at| at + 2)
}

/// What has been heard from a compositor, in words.
#[derive(Debug, Default)]
pub struct Heard {
    /// What has been read of the line not yet ended.
    partial: Vec<u8>,
    /// Whether the line not yet ended is too long to be a word, and is
    /// being passed over to its end.
    skipping: bool,
}

impl Heard {
    /// The words `bytes` end, after what was heard before them.
    pub fn hear(&mut self, bytes: &[u8]) -> Vec<Word> {
        let mut words = Vec::new();
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            let (line, ends) = match line.split_last() {
                Some((b'\n', line)) => (line, true),
                _ => (line, false),
            };
            let skipping = self.skipping || self.partial.len() + line.len() > LONGEST;
            if !skipping {
                self.partial.extend_from_slice(line);
            }
            self.skipping = skipping && !ends;
            if ends {
                let whole = std::mem::take(&mut self.partial);
                let word = String::from_utf8(whole).ok();
                let word = word
                    .and_then(|word| Word::read(&word))
                    .filter(|_| !skipping);
                words.extend(word);
            }
        }
        words
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd;

    use super::*;

    #[test]
    fn what_is_told_is_read_back_as_it_was() {
        for told in [
            Told::default(),
            Told {
                again: true,
                locked: true,
                display: Some(3),
                older: true,
                relayed: true,
            },
            Told {
                again: true,
                locked: false,
                display: None,
                older: false,
                relayed: false,
            },
        ] {
            let written = told.written();
            assert!(written.ends_with("\n\n") || written == "\n", "{written:?}");
            assert_eq!(Told::read(&written), told);
        }
        assert_eq!(Told::default().written(), "\n", "nothing to tell");
    }

    #[test]
    fn what_a_holder_does_not_know_it_was_told_is_passed_over() {
        let told = Told::read("again\nsomething new\ndisplay x\ndisplay 2\nlocked\n\n");
        let expected = Told {
            again: true,
            locked: true,
            display: Some(2),
            older: false,
            relayed: false,
        };
        assert_eq!(told, expected);
    }

    #[test]
    fn each_word_is_read_back_from_its_line() {
        let words = [
            Word::Up,
            Word::Locked,
            Word::Unlocked,
            Word::Display(0),
            Word::Display(12),
            Word::SteppingDown,
        ];
        for word in words {
            let line = word.line();
            assert!(line.ends_with('\n'), "{line:?}");
            assert_eq!(Word::read(line.trim_end()), Some(word), "{line:?}");
        }
        assert_eq!(Word::Display(7).line(), "display 7\n");
        assert_eq!(Word::SteppingDown.line(), "stepping down\n");
    }

    #[test]
    fn a_word_not_known_here_is_none() {
        for line in ["", "down", "display", "display -1", "display 1 2", "up!"] {
            assert_eq!(Word::read(line), None, "{line:?}");
        }
    }

    #[test]
    fn words_are_heard_however_what_carries_them_is_cut() {
        let mut heard = Heard::default();
        assert_eq!(heard.hear(b"u"), []);
        assert_eq!(heard.hear(b"p\nlocked\ndisp"), [Word::Up, Word::Locked]);
        assert_eq!(heard.hear(b"lay 4\nwhat\n\n"), [Word::Display(4)]);
        assert_eq!(heard.hear(b"unlocked\n"), [Word::Unlocked]);
    }

    #[test]
    fn a_line_too_long_to_be_a_word_is_passed_over_to_its_end() {
        let mut heard = Heard::default();
        let long = vec![b'x'; LONGEST + 1];
        assert_eq!(heard.hear(&long), []);
        assert_eq!(heard.hear(b"up"), [], "still the long line");
        assert_eq!(heard.hear(b"\nlocked\n"), [Word::Locked]);
        // Long, but ended in the same breath.
        let mut long = vec![b'y'; LONGEST * 2];
        long.extend_from_slice(b"\nup\n");
        assert_eq!(heard.hear(&long), [Word::Up]);
    }

    #[test]
    fn a_hand_carries_its_text_and_descriptors() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let (one, two) = UnixStream::pair().unwrap();
        give(&ours, "again\nlocked\n\n", &[one.as_fd(), two.as_fd()]).unwrap();
        let (text, fds) = take(&theirs).unwrap();
        assert_eq!(text, "again\nlocked\n\n");
        assert_eq!(fds.len(), 2);
        // What came over is the same connection as was handed.
        let one = UnixStream::from(fds.into_iter().next().unwrap());
        drop(two);
        let mut byte = [0];
        assert_eq!(io::Read::read(&mut &one, &mut byte).unwrap(), 0, "closed");
    }

    #[test]
    fn who_handed_it_is_not_taken_for_a_descriptor() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        // Told who sends, the kernel says so with everything sent.
        rustix::net::sockopt::set_socket_passcred(&theirs, true).unwrap();
        let (one, _two) = UnixStream::pair().unwrap();
        give(&ours, "\n", &[one.as_fd()]).unwrap();
        let (text, fds) = take(&theirs).unwrap();
        assert_eq!((text.as_str(), fds.len()), ("\n", 1));
    }

    #[test]
    fn a_hand_may_come_in_pieces_and_with_no_descriptors() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        io::Write::write_all(&mut &ours, b"display 1\n").unwrap();
        io::Write::write_all(&mut &ours, b"\nafter").unwrap();
        let (text, fds) = take(&theirs).unwrap();
        assert_eq!((text.as_str(), fds.len()), ("display 1\n\n", 0));
    }

    #[test]
    fn a_hand_that_ends_early_or_runs_on_is_refused() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        io::Write::write_all(&mut &ours, b"again\n").unwrap();
        drop(ours);
        let ended = take(&theirs).unwrap_err();
        assert_eq!(ended.kind(), io::ErrorKind::UnexpectedEof);

        let (ours, theirs) = UnixStream::pair().unwrap();
        io::Write::write_all(&mut &ours, &[b'x'; LONGEST + 1]).unwrap();
        let long = take(&theirs).unwrap_err();
        assert_eq!(long.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn too_much_to_hand_is_refused_before_anything_is_sent() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let long = "x".repeat(LONGEST + 1);
        assert_eq!(
            give(&ours, &long, &[]).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        let many = vec![ours.as_fd(); MOST + 1];
        assert_eq!(
            give(&ours, "\n", &many).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        drop(ours);
        let mut rest = Vec::new();
        io::Read::read_to_end(&mut &theirs, &mut rest).unwrap();
        assert_eq!(rest, b"");
    }

    #[test]
    fn a_hand_that_cannot_be_given_or_taken_says_why() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        drop(theirs);
        assert!(give(&ours, "\n", &[]).is_err(), "nobody to take it");
        // Nothing handed yet, to one that will not wait for it.
        let (_ours, theirs) = UnixStream::pair().unwrap();
        theirs.set_nonblocking(true).unwrap();
        let waiting = take(&theirs).unwrap_err();
        assert_eq!(waiting.kind(), io::ErrorKind::WouldBlock);
    }
}
