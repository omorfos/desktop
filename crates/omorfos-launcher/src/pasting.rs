//! What is pasted into the launcher, as it arrives from the program that
//! copied it.
//!
//! The launcher has the keyboard while it is open, so it is offered the
//! clipboard as any program with the keyboard is, through Wayland's own
//! (ADR 82). What it asks for is written into a pipe by the program that
//! copied it, whenever that program gets round to it; the launcher reads
//! it as it comes, never waiting on it.

use std::io::{PipeReader, Read};
use std::os::fd::{AsFd, BorrowedFd};

use rustix::event::{PollFd, PollFlags, Timespec, poll};

/// The forms of text the launcher asks for, the one it likes best first.
pub(crate) const TEXTS: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];

/// The most of what is pasted that is read, in bytes: far more than the
/// launcher holds, and a program that gave more would keep it reading.
const LONGEST: usize = 4096;

/// What is pasted, as far as it has arrived.
#[derive(Debug)]
pub(crate) struct Pasting {
    from: PipeReader,
    got: Vec<u8>,
}

impl Pasting {
    /// What arrives through `from`.
    pub(crate) const fn new(from: PipeReader) -> Self {
        Self {
            from,
            got: Vec::new(),
        }
    }

    /// What is waited on for more of it.
    pub(crate) fn source(&self) -> BorrowedFd<'_> {
        self.from.as_fd()
    }

    /// Takes in what has arrived, if anything has, without waiting.
    /// Returns what was pasted once all of it has arrived, or as much as
    /// is read of it: its first line, without what is not to be typed.
    pub(crate) fn take(&mut self) -> Option<String> {
        let mut ready = [PollFd::new(&self.from, PollFlags::IN)];
        let now = Timespec::default();
        // Something to read, or the end of it.
        if !poll(&mut ready, Some(&now)).is_ok_and(|ready| ready > 0) {
            return None;
        }
        let mut chunk = [0; 1024];
        // One that fails ends it as one that has ended does.
        let read = self.from.read(&mut chunk).unwrap_or(0);
        self.got.extend_from_slice(&chunk[..read]);
        (read == 0 || self.got.len() >= LONGEST).then(|| typed(&self.got))
    }
}

/// What is typed of `pasted`: its first line, without control characters.
fn typed(pasted: &[u8]) -> String {
    let pasted = String::from_utf8_lossy(pasted);
    let line = pasted.lines().next().unwrap_or_default();
    line.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn what_is_pasted_arrives_as_it_is_written_and_is_typed_once_it_ends() {
        let (from, mut to) = std::io::pipe().unwrap();
        let mut pasting = Pasting::new(from);
        assert_eq!(pasting.take(), None, "nothing yet");
        to.write_all(b"fire").unwrap();
        assert_eq!(pasting.take(), None, "more may come");
        to.write_all(b"fox\tlater\nnot this").unwrap();
        drop(to);
        let typed = std::iter::from_fn(|| Some(pasting.take())).flatten().next();
        assert_eq!(typed.as_deref(), Some("firefoxlater"));
    }

    #[test]
    fn what_is_waited_on_wakes_once_something_arrives() {
        let (from, mut to) = std::io::pipe().unwrap();
        let pasting = Pasting::new(from);
        let waited = |pasting: &Pasting| {
            let source = pasting.source();
            let mut ready = [PollFd::new(&source, PollFlags::IN)];
            poll(&mut ready, Some(&Timespec::default())).unwrap()
        };
        assert_eq!(waited(&pasting), 0, "asleep");
        to.write_all(b"files").unwrap();
        assert_eq!(waited(&pasting), 1, "woken");
    }

    #[test]
    fn of_what_never_ends_only_so_much_is_read() {
        let (from, mut to) = std::io::pipe().unwrap();
        let mut pasting = Pasting::new(from);
        // More than is read of it, and the writer still there: it has not
        // ended.
        to.write_all(&[b'a'; 5000]).unwrap();
        let typed = (0..8).find_map(|_| pasting.take()).unwrap();
        assert_eq!(typed.len(), LONGEST);
    }

    #[test]
    fn what_is_not_text_is_typed_as_far_as_it_can_be() {
        assert_eq!(typed(b"na\xc3\xafve \xff!"), "naïve \u{fffd}!");
        assert_eq!(typed(b""), "");
        assert_eq!(typed(b"\r\nsecond"), "");
    }
}
