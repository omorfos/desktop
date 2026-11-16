//! Bytes handed to a program that asked for them, as what was copied is to
//! a program that pastes it.
//!
//! The program gives a pipe to be written into, and reads from its other
//! end when it likes, or never. Whoever hands something over by writing
//! into it and waiting is stopped for as long as the program pleases: the
//! compositor with every screen it draws, if it is the compositor. Three
//! programs hand what was copied over, the compositor its own text,
//! `omorfos-clipboard` what it keeps and `omorfos-screenshot` its picture,
//! and each does it through [`Handing`], so that it is right in one place
//! (ADR 90).
//!
//! Each thing handed over is on a thread of its own, so that nothing of
//! whoever hands it over touches the program's pipe at all. The thread
//! does not wait on the program either. Whether a write to a pipe waits is
//! the pipe's to say, and the pipe is the program's as much as ours: asked
//! not to wait, it can be asked again to, by the program, before it is
//! written to. So what is handed over is written into a pipe of our own,
//! which nothing else can ask anything of, and moved from there into the
//! program's (`splice`), which is told at each move not to wait. A program
//! whose pipe has no room is waited for by the clock, and given up on
//! once its time is up.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{FileType, fstat};
use rustix::io::{Errno, retry_on_intr, write};
use rustix::pipe::{PipeFlags, SpliceFlags, pipe_with, splice};

/// The most that is handed over at once: more than any program asks for
/// together, and few enough that one that takes nothing costs little.
pub const MOST: usize = 16;

/// How long a program is given to take what it is handed, all told. One
/// that pastes reads at once; one that has not read in this long is not
/// going to.
const PATIENCE: Duration = Duration::from_secs(30);

/// What is being handed over to programs that asked for it.
#[derive(Debug)]
pub struct Handing {
    /// How long each program is given.
    patience: Duration,
    /// The threads handing something over, and those that have just done.
    writers: Vec<JoinHandle<()>>,
}

impl Default for Handing {
    fn default() -> Self {
        Self::patient(PATIENCE)
    }
}

impl Handing {
    /// Nothing being handed over yet, each program to be given `patience`
    /// to take what it is handed: half a minute, but in the tests of this,
    /// which do not wait that long.
    const fn patient(patience: Duration) -> Self {
        Self {
            patience,
            writers: Vec::new(),
        }
    }

    /// Hands `bytes` to a program through `to`, the pipe it gave for them,
    /// without waiting on it: at once if the pipe has room, as the program
    /// reads if it has not, and not at all once the program has had its
    /// time. The pipe is closed when it is done with, which is how the
    /// program knows it has everything.
    ///
    /// Returns whether it is being handed over. It is not if `to` is not a
    /// pipe, which is all a program pastes through and all that can be
    /// written into without waiting; nor while [`MOST`] are being handed
    /// over already. The program then reads nothing, as from one that had
    /// nothing to give.
    pub fn hand(&mut self, bytes: Arc<[u8]>, to: OwnedFd) -> bool {
        let pipe = fstat(&to).is_ok_and(|is| FileType::from_raw_mode(is.st_mode) == FileType::Fifo);
        let room = self.busy() < MOST;
        let patience = self.patience;
        // A thread that cannot be started hands nothing over.
        let writer = (pipe && room)
            .then(|| thread::Builder::new().spawn(move || pour(&bytes, &to, patience)))
            .and_then(Result::ok);
        let handed = writer.is_some();
        self.writers.extend(writer);
        handed
    }

    /// How many are being handed over now.
    pub fn busy(&mut self) -> usize {
        self.writers.retain(|writer| !writer.is_finished());
        self.writers.len()
    }

    /// Waits until everything being handed over has been, or been given up
    /// on: no longer than a program is given. For a program that ends once
    /// it has handed over what it had, so that what a program is reading
    /// as it ends is not cut short.
    pub fn finish(&mut self) {
        for writer in self.writers.drain(..) {
            // One that failed handed over what it could.
            let _ = writer.join();
        }
    }
}

/// Writes `bytes` into `to`, a pipe of a program's, as the program makes
/// room for them, for no longer than `patience`; and stops once the
/// program has stopped reading.
fn pour(bytes: &[u8], to: &OwnedFd, patience: Duration) {
    let until = Instant::now() + patience;
    // A pipe of our own, which does not wait because we made it so, and
    // which nothing else can make wait again.
    let ours = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).ok();
    let _ = ours.map(|(staged, staging)| {
        let (mut written, mut moved) = (0, 0);
        while moved < bytes.len() {
            // As much more as our own pipe takes: none, if it is full.
            written += write(&staging, &bytes[written..]).unwrap_or(0);
            let waiting = written - moved;
            match splice(&staged, None, to, None, waiting, SpliceFlags::NONBLOCK) {
                Ok(some) if some > 0 => moved += some,
                Err(Errno::AGAIN) if room(to, until) => {}
                // The program has stopped reading, or has had its time.
                _ => break,
            }
        }
    });
}

/// Waits for `to` to have room, until `until` at the latest, and returns
/// whether it is worth trying it again: not once the time is up, nor if
/// it cannot be waited for at all.
fn room(to: &OwnedFd, until: Instant) -> bool {
    let left = until.saturating_duration_since(Instant::now());
    let mut waiting = [PollFd::new(to, PollFlags::OUT)];
    // A wait that was interrupted is begun again.
    let waited =
        Timespec::try_from(left).map(|left| retry_on_intr(|| poll(&mut waiting, Some(&left))));
    !left.is_zero() && matches!(waited, Ok(Ok(1..)))
}

#[cfg(test)]
mod tests {
    use std::io::{PipeReader, PipeWriter, Read, Write};
    use std::os::unix::net::UnixStream;

    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    use super::*;

    /// A pipe as a program that pastes makes one: what it reads from, and
    /// what it hands over to be written into.
    fn pipe() -> (PipeReader, OwnedFd) {
        let (reader, writer) = std::io::pipe().unwrap();
        (reader, OwnedFd::from(writer))
    }

    /// A pipe with no room left in it, which waits when written to, as a
    /// program that means to stop whoever writes to it would make one:
    /// what it reads from, what it hands over, and how much is in it.
    fn full() -> (PipeReader, OwnedFd, usize) {
        let (reader, writer) = std::io::pipe().unwrap();
        let mut writer: PipeWriter = writer;
        fcntl_setfl(&writer, OFlags::NONBLOCK).unwrap();
        let mut filled = 0;
        while let Ok(more) = writer.write(&[b'.'; 4096]) {
            filled += more;
        }
        fcntl_setfl(&writer, OFlags::empty()).unwrap();
        (reader, OwnedFd::from(writer), filled)
    }

    /// Waits until nothing is being handed over, for a few seconds at most.
    fn idle(handing: &mut Handing) -> bool {
        let until = Instant::now() + Duration::from_secs(10);
        while handing.busy() > 0 && Instant::now() < until {
            thread::sleep(Duration::from_millis(2));
        }
        handing.busy() == 0
    }

    fn all(mut reader: PipeReader) -> Vec<u8> {
        let mut read = Vec::new();
        reader.read_to_end(&mut read).unwrap();
        read
    }

    #[test]
    fn what_is_handed_over_arrives_whole_however_much_it_is() {
        let mut handing = Handing::default();
        // Many times what a pipe holds, so that it is read as it comes.
        let much: Vec<u8> = (0..3_000_000_u32).map(|at| at.to_le_bytes()[0]).collect();
        let (reader, to) = pipe();
        assert!(handing.hand(Arc::from(much.clone()), to));
        assert_eq!(all(reader), much);
        // And nothing at all, which is the end at once.
        let (reader, to) = pipe();
        assert!(handing.hand(Arc::from(Vec::new()), to));
        assert_eq!(all(reader), b"");
        assert!(idle(&mut handing));
    }

    #[test]
    fn a_program_that_does_not_read_keeps_nobody_waiting_and_is_given_up_on() {
        let mut handing = Handing::patient(Duration::from_millis(100));
        let (reader, to, filled) = full();
        // The program keeps hold of what it handed over, as one that means
        // to change what it says of waiting would.
        let theirs = to.try_clone().unwrap();
        // Whoever hands it over goes on at once, the pipe waiting or not.
        let begun = Instant::now();
        assert!(handing.hand(Arc::from(*b"copied"), to));
        assert!(begun.elapsed() < Duration::from_secs(5));
        // Its time up, it is given up on, with the pipe still as the
        // program made it: nothing rests on what the pipe says of waiting,
        // which the program could unsay.
        assert!(idle(&mut handing));
        assert!(begun.elapsed() >= Duration::from_millis(100));
        assert!(!fcntl_getfl(&theirs).unwrap().contains(OFlags::NONBLOCK));
        // What it reads then is only what it put there itself.
        drop(theirs);
        assert_eq!(all(reader).len(), filled);
    }

    #[test]
    fn a_program_that_reads_late_but_in_time_is_given_it_still() {
        let mut handing = Handing::default();
        let (mut reader, to, filled) = full();
        assert!(handing.hand(Arc::from(*b"copied"), to));
        thread::sleep(Duration::from_millis(20));
        assert_eq!(handing.busy(), 1, "waiting for room");
        let mut read = vec![0; filled];
        reader.read_exact(&mut read).unwrap();
        assert_eq!(all(reader), b"copied");
        assert!(idle(&mut handing));
    }

    #[test]
    fn what_is_being_handed_over_can_be_waited_for() {
        let mut handing = Handing::default();
        let (mut reader, to, filled) = full();
        assert!(handing.hand(Arc::from(*b"copied"), to));
        // The program reads in its own time, on a thread of its own.
        let program = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let mut read = vec![0; filled];
            reader.read_exact(&mut read).unwrap();
            all(reader)
        });
        handing.finish();
        assert_eq!(handing.busy(), 0);
        assert_eq!(program.join().unwrap(), b"copied");
    }

    #[test]
    fn no_more_are_handed_over_at_once_than_the_most_there_may_be() {
        let mut handing = Handing::default();
        let stuck: Vec<_> = (0..MOST).map(|_| full()).collect();
        let stuck = stuck.into_iter().map(|(reader, to, _)| {
            assert!(handing.hand(Arc::from(*b"copied"), to));
            reader
        });
        let mut stuck: Vec<PipeReader> = stuck.collect();
        assert_eq!(handing.busy(), MOST);
        // One more is given nothing.
        let (reader, to) = pipe();
        assert!(!handing.hand(Arc::from(*b"copied"), to));
        assert_eq!(all(reader), b"");
        // Once a program that was not reading has gone, there is room for
        // another.
        drop(stuck.pop());
        // However soon the thread that was writing notices: each part of the
        // wait runs at least once, so what is reached does not depend on it.
        let waited = std::iter::repeat_with(|| {
            thread::sleep(Duration::from_millis(2));
            handing.busy()
        });
        let freed = waited.take(5000).find(|busy| *busy < MOST);
        assert!(freed.is_some(), "room again within ten seconds");
        let (reader, to) = pipe();
        assert!(handing.hand(Arc::from(*b"copied"), to));
        assert_eq!(all(reader), b"copied");
        // The rest go as their programs do.
        drop(stuck);
        assert!(idle(&mut handing));
    }

    #[test]
    fn what_is_not_a_pipe_is_written_nothing() {
        let mut handing = Handing::default();
        // A file, which a write may wait on for as long as what it is kept
        // on likes.
        let file = tempfile::tempfile().unwrap();
        let kept = file.try_clone().unwrap();
        assert!(!handing.hand(Arc::from(*b"copied"), OwnedFd::from(file)));
        assert_eq!(kept.metadata().unwrap().len(), 0);
        // A socket, which whether to wait is its own to say.
        let (ours, theirs) = UnixStream::pair().unwrap();
        assert!(!handing.hand(Arc::from(*b"copied"), OwnedFd::from(ours)));
        let mut read = Vec::new();
        (&theirs).read_to_end(&mut read).unwrap();
        assert_eq!(read, b"");
        assert_eq!(handing.busy(), 0);
    }
}
