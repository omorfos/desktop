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
//! What is handed over is poured by a thread of [`Handing`]'s own, so that
//! nothing of whoever hands it over touches the program's pipe at all, into
//! every pipe it has at once. The thread does not wait on any program
//! either. Whether a write to a pipe waits is the pipe's to say, and the
//! pipe is the program's as much as ours: asked not to wait, it can be
//! asked again to, by the program, before it is written to. So what is
//! handed over is written into a pipe of our own, which nothing else can
//! ask anything of, and moved from there into the program's (`splice`),
//! which is told at each move not to wait. A program whose pipe has no room
//! is waited for by the clock, and given up on once its time is up.
//!
//! No more than [`MOST`] are poured at once. One more lets go of the one
//! that has waited longest: a paste is always the newest, and a program
//! asking without end through pipes it never reads cannot keep anything
//! from the one that does.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{FileType, fstat};
use rustix::io::{Errno, read, retry_on_intr, write};
use rustix::pipe::{PipeFlags, SpliceFlags, pipe_with, splice};

/// The most that is poured at once: more than any program asks for
/// together, and few enough that one that takes nothing costs little.
pub const MOST: usize = 16;

/// How long a program is given to take what it is handed, all told. One
/// that pastes reads at once; one that has not read in this long is not
/// going to.
const PATIENCE: Duration = Duration::from_secs(30);

/// What is being handed over to programs that asked for it.
pub struct Handing {
    /// How long each program is given.
    patience: Duration,
    /// How many are being poured, or are on their way to be.
    busy: Arc<AtomicUsize>,
    /// The thread that pours, while there is one, with what it is handed
    /// and what wakes it.
    pourer: Option<Pourer>,
}

/// The thread that pours, and what it is handed through.
struct Pourer {
    handed: Sender<Pour>,
    wake: OwnedFd,
    thread: JoinHandle<()>,
}

impl std::fmt::Debug for Handing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let busy = self.busy.load(Ordering::SeqCst);
        f.debug_struct("Handing")
            .field("busy", &busy)
            .finish_non_exhaustive()
    }
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
    fn patient(patience: Duration) -> Self {
        Self {
            patience,
            busy: Arc::default(),
            pourer: None,
        }
    }

    /// Hands `bytes` to a program through `to`, the pipe it gave for them,
    /// without waiting on it: at once if the pipe has room, as the program
    /// reads if it has not, and not at all once the program has had its
    /// time, or [`MOST`] newer have been handed over since. The pipe is
    /// closed when it is done with, which is how the program knows it has
    /// everything.
    ///
    /// Returns whether it is being handed over. It is not if `to` is not a
    /// pipe, which is all a program pastes through and all that can be
    /// written into without waiting, or if what it is poured through cannot
    /// be made. The program then reads nothing, as from one that had
    /// nothing to give.
    pub fn hand(&mut self, bytes: Arc<[u8]>, to: OwnedFd) -> bool {
        let pipe = fstat(&to).is_ok_and(|is| FileType::from_raw_mode(is.st_mode) == FileType::Fifo);
        let until = Instant::now() + self.patience;
        let pour = pipe.then(|| Pour::new(bytes, to, until)).flatten();
        let busy = Arc::clone(&self.busy);
        let pourer = pour.and_then(|pour| self.pourer().map(|pourer| (pourer, pour)));
        pourer.is_some_and(|(pourer, pour)| {
            busy.fetch_add(1, Ordering::SeqCst);
            let sent = pourer.handed.send(pour).is_ok();
            // One that cannot be sent never reaches the thread to be counted
            // done.
            busy.fetch_sub(usize::from(!sent), Ordering::SeqCst);
            let _ = write(&pourer.wake, &[0]);
            sent
        })
    }

    /// How many are being handed over now.
    pub fn busy(&mut self) -> usize {
        self.busy.load(Ordering::SeqCst)
    }

    /// Waits until everything being handed over has been, or been given up
    /// on: no longer than a program is given. For a program that ends once
    /// it has handed over what it had, so that what a program is reading
    /// as it ends is not cut short.
    pub fn finish(&mut self) {
        let pourer = self.pourer.take();
        let _ = pourer.map(
            |Pourer {
                 handed,
                 wake,
                 thread,
             }| {
                // Told there is no more, the thread ends once it has poured
                // what it has.
                drop(handed);
                let _ = write(&wake, &[0]);
                // One that failed poured what it could.
                thread.join()
            },
        );
    }

    /// The thread that pours, started if there is none: none if it cannot
    /// be.
    fn pourer(&mut self) -> Option<&Pourer> {
        if self.pourer.is_none() {
            let (handed, taken) = mpsc::channel();
            let woken = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).ok();
            let busy = Arc::clone(&self.busy);
            self.pourer = woken.and_then(|(woken, wake)| {
                let pouring = move || pour_all(&taken, &woken, &busy);
                let thread = thread::Builder::new().spawn(pouring).ok();
                thread.map(|thread| Pourer {
                    handed,
                    wake,
                    thread,
                })
            });
        }
        self.pourer.as_ref()
    }
}

/// Something being poured into a program's pipe, through a pipe of our own.
struct Pour {
    bytes: Arc<[u8]>,
    /// The program's pipe.
    to: OwnedFd,
    /// Our own, read from and written to.
    staged: OwnedFd,
    staging: OwnedFd,
    /// How much has been written into ours, and moved into the program's.
    written: usize,
    moved: usize,
    /// When the program's time is up.
    until: Instant,
}

/// How far pouring has got.
#[derive(PartialEq, Eq)]
enum Poured {
    /// All of it is in the program's pipe.
    Done,
    /// The program's pipe has no room for now.
    Waiting,
    /// The program has stopped reading.
    Gone,
}

impl Pour {
    /// `bytes` to be poured into `to` until `until`, through a pipe of our
    /// own, which does not wait because we made it so and which nothing
    /// else can make wait again; nothing if that pipe cannot be made.
    fn new(bytes: Arc<[u8]>, to: OwnedFd, until: Instant) -> Option<Self> {
        let ours = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).ok();
        ours.map(|(staged, staging)| Self {
            bytes,
            to,
            staged,
            staging,
            written: 0,
            moved: 0,
            until,
        })
    }

    /// Pours as much as the program's pipe takes now.
    fn pour(&mut self) -> Poured {
        while self.moved < self.bytes.len() {
            // As much more as our own pipe takes: none, if it is full.
            self.written += write(&self.staging, &self.bytes[self.written..]).unwrap_or(0);
            let waiting = self.written - self.moved;
            match splice(
                &self.staged,
                None,
                &self.to,
                None,
                waiting,
                SpliceFlags::NONBLOCK,
            ) {
                Ok(some) if some > 0 => self.moved += some,
                Err(Errno::AGAIN) => return Poured::Waiting,
                _ => return Poured::Gone,
            }
        }
        Poured::Done
    }
}

/// What the thread that pours does: takes what it is handed from `taken`,
/// woken through `woken`, and pours each, no more than [`MOST`] at once,
/// until it is told there is no more and has poured what it had. `busy`
/// is told of each that is done with.
fn pour_all(taken: &Receiver<Pour>, woken: &OwnedFd, busy: &AtomicUsize) {
    let mut pours: Vec<Pour> = Vec::new();
    let mut open = true;
    while open || !pours.is_empty() {
        // What has been handed over since; past the most, the one that has
        // waited longest is let go of.
        loop {
            match taken.try_recv() {
                Ok(pour) => pours.push(pour),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        let past = pours.len().saturating_sub(MOST);
        let now = Instant::now();
        let before = pours.len();
        pours.drain(..past);
        pours.retain_mut(|pour| pour.until > now && pour.pour() == Poured::Waiting);
        busy.fetch_sub(before - pours.len(), Ordering::SeqCst);
        // Until a pipe has room, or more is handed over, or the soonest
        // time is up.
        let soonest = pours.iter().map(|pour| pour.until).min();
        let left = soonest.map(|until| until.saturating_duration_since(now));
        let mut waiting = vec![PollFd::new(woken, PollFlags::IN)];
        waiting.extend(
            pours
                .iter()
                .map(|pour| PollFd::new(&pour.to, PollFlags::OUT)),
        );
        let left = left.and_then(|left| Timespec::try_from(left).ok());
        // Interrupted or not, it looks again at what it has.
        let _ = (open || !pours.is_empty())
            .then(|| retry_on_intr(|| poll(&mut waiting, left.as_ref())));
        let mut drained = [0; 64];
        while read(woken, &mut drained).is_ok_and(|read| read > 0) {}
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "a test ends with its threads, and one that cannot be started fails it"
)]
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
    fn what_is_being_handed_over_is_said_by_how_many_not_what() {
        let mut handing = Handing::default();
        let (_reader, to, _) = full();
        assert!(handing.hand(Arc::from(*b"secret"), to));
        let said = format!("{handing:?}");
        assert_eq!(said, "Handing { busy: 1, .. }");
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
    fn past_the_most_the_one_that_has_waited_longest_is_let_go_of() {
        let mut handing = Handing::default();
        // As many as may be poured at once, to programs that never read,
        // as one that means to keep a paste from going through would ask.
        let stuck: Vec<_> = (0..MOST).map(|_| full()).collect();
        let mut stuck: Vec<_> = stuck
            .into_iter()
            .map(|(reader, to, filled)| {
                assert!(handing.hand(Arc::from(*b"held up"), to));
                (reader, filled)
            })
            .collect();
        // A paste, the newest, goes through whatever they ask.
        let (reader, to) = pipe();
        assert!(handing.hand(Arc::from(*b"copied"), to));
        assert_eq!(all(reader), b"copied");
        // And the one that had waited longest was let go of already, not
        // half a minute from now: its program reads only what it put there
        // itself, and then the end at once.
        let (mut oldest, filled) = stuck.remove(0);
        let mut read = vec![0; filled];
        oldest.read_exact(&mut read).unwrap();
        fcntl_setfl(&oldest, OFlags::NONBLOCK).unwrap();
        assert_eq!(oldest.read(&mut [0; 16]).unwrap(), 0, "the end");
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
