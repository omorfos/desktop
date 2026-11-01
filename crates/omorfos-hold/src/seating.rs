//! The seat the holder keeps for the compositors it starts (ADR 63).
//!
//! Each compositor is handed a connection to ask for the seat's devices
//! over, as it would ask libseat, and is told whether the seat is the
//! session's to use as that changes. What is opened for a compositor is
//! closed as it ends, so that the next can open it: the virtual terminal
//! stays as it is, the session staying the holder's throughout.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use omorfos_ipc::hold::{self, Heard};
use omorfos_ipc::seat::{Asked, Seated};
use smithay::reexports::calloop::PostAction;

/// What the holder needs of a seat.
pub(crate) trait Seat: std::fmt::Debug {
    /// What the seat is called.
    fn name(&self) -> String;
    /// Opens the device at `path`, or says why it cannot, as an error
    /// number.
    fn open(&mut self, path: &Path) -> Result<OwnedFd, i32>;
    /// Closes `device`, opened by [`Seat::open`].
    fn close(&mut self, device: OwnedFd);
    /// Asks for the virtual terminal `terminal` to be shown.
    fn switch(&mut self, terminal: i32);
}

/// Where graphics cards are, whose pictures stay on the screens while the
/// card is open.
const CARDS: &str = "/dev/dri/card";

/// The seat the holder keeps, and what it has opened on it.
#[derive(Debug)]
pub(crate) struct Seating {
    seat: Box<dyn Seat>,
    /// Whether the seat is the session's to use.
    active: bool,
    /// The holder's end of the connection to the compositor running, which
    /// it is told on.
    link: Option<UnixStream>,
    /// What the compositor running has asked, as far as it has been read.
    asking: Heard,
    /// What has been opened for the compositor running, by the number it
    /// knows each by, with where each is.
    opened: BTreeMap<u32, (PathBuf, OwnedFd)>,
    /// The number the next device opened is known by.
    next: u32,
}

impl Seating {
    /// Keeps `seat`, which is the session's to use if `active`.
    pub(crate) fn new(seat: Box<dyn Seat>, active: bool) -> Self {
        Self {
            seat,
            active,
            link: None,
            asking: Heard::default(),
            opened: BTreeMap::new(),
            next: 0,
        }
    }

    /// What the seat is called.
    pub(crate) fn name(&self) -> String {
        self.seat.name()
    }

    /// A connection for the compositor about to be started to ask for the
    /// seat's devices over, which is told first whether the seat is the
    /// session's to use: the compositor's end, and one to read what it asks
    /// from.
    pub(crate) fn link(&mut self) -> io::Result<(UnixStream, UnixStream)> {
        let state = if self.active {
            Seated::Active
        } else {
            Seated::Paused
        };
        let told = |(ours, theirs): (UnixStream, UnixStream)| {
            (&ours)
                .write_all(state.line().as_bytes())
                .map(|()| (ours, theirs))
        };
        let read = |(ours, theirs): (UnixStream, UnixStream)| {
            ours.try_clone().map(|reading| (ours, theirs, reading))
        };
        let linked = UnixStream::pair().and_then(told).and_then(read);
        linked.map(|(ours, theirs, reading)| {
            self.link = Some(ours);
            (theirs, reading)
        })
    }

    /// Answers what the compositor running has asked over `from`, as far
    /// as there is anything to read; says whether to go on reading it,
    /// which is not once it has gone.
    pub(crate) fn asked(&mut self, from: &UnixStream) -> PostAction {
        let mut bytes = [0; 1024];
        let Ok((read @ 1.., _)) = hold::read(from, &mut bytes) else {
            return PostAction::Remove;
        };
        let lines = self.asking.lines(&bytes[..read]);
        for asked in lines.iter().filter_map(|line| Asked::read(line)) {
            self.answer(asked);
        }
        PostAction::Continue
    }

    /// Does what `asked` asks, and answers it where it is to be answered.
    /// One that cannot be told has gone, and what it had is closed as the
    /// holder hears it end.
    fn answer(&mut self, asked: Asked) {
        match asked {
            Asked::Open(path) => match self.seat.open(&path) {
                Ok(device) => {
                    let number = self.next;
                    self.next = self.next.wrapping_add(1);
                    let _ = self.tell(Seated::Opened(number), Some(device.as_fd()));
                    self.opened.insert(number, (path, device));
                }
                Err(error) => {
                    let _ = self.tell(Seated::Refused(error), None);
                }
            },
            Asked::Close(number) => {
                let opened = self.opened.remove(&number);
                let _ = opened.map(|(_, device)| self.seat.close(device));
            }
            Asked::Switch(terminal) => self.seat.switch(terminal),
        }
    }

    /// Tells the compositor running `seated`, handing `device` with it.
    fn tell(&self, seated: Seated, device: Option<std::os::fd::BorrowedFd<'_>>) -> io::Result<()> {
        let link = self.link.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        hold::give(link, &seated.line(), &Vec::from_iter(device))
    }

    /// Takes in that the seat is the session's to use, or not, telling the
    /// compositor running, if there is one.
    pub(crate) fn changed(&mut self, active: bool) {
        self.active = active;
        let seated = if active {
            Seated::Active
        } else {
            Seated::Paused
        };
        let _ = self.tell(seated, None);
    }

    /// Closes everything opened for the compositor, which has ended, so
    /// that the next may open it. Returns the first graphics card it had,
    /// still open, so that what it drew stays on the screens until the next
    /// draws (ADR 62).
    pub(crate) fn ended(&mut self) -> Option<OwnedFd> {
        self.link = None;
        self.asking = Heard::default();
        let mut card = None;
        for (path, device) in std::mem::take(&mut self.opened).into_values() {
            if card.is_none() && path.to_string_lossy().starts_with(CARDS) {
                card = device.try_clone().ok();
            }
            self.seat.close(device);
        }
        card
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::fs::File;
    use std::rc::Rc;

    use super::*;

    /// What a seat of the tests' own was asked, in order.
    pub(crate) type Log = Rc<RefCell<Vec<String>>>;

    /// A seat that opens what it is asked to as a file, but for what is
    /// under `/refused`, and notes down everything it is asked.
    #[derive(Debug)]
    pub(crate) struct Fake(pub(crate) Log);

    impl Seat for Fake {
        fn name(&self) -> String {
            "seat9".to_owned()
        }

        fn open(&mut self, path: &Path) -> Result<OwnedFd, i32> {
            self.0.borrow_mut().push(format!("open {}", path.display()));
            if path.starts_with("/refused") {
                return Err(16);
            }
            Ok(OwnedFd::from(File::open("/dev/null").unwrap()))
        }

        fn close(&mut self, _: OwnedFd) {
            self.0.borrow_mut().push("close".to_owned());
        }

        fn switch(&mut self, terminal: i32) {
            self.0.borrow_mut().push(format!("switch {terminal}"));
        }
    }

    /// What the compositor's end of `link` has been told since last asked,
    /// and how many descriptors came with it.
    fn told(link: &UnixStream) -> (Vec<Seated>, usize) {
        link.set_nonblocking(true).unwrap();
        let mut bytes = [0; 1024];
        let (mut said, mut fds) = (Vec::new(), 0);
        let mut heard = Heard::default();
        while let Ok((read @ 1.., handed)) = hold::read(link, &mut bytes) {
            let lines = heard.lines(&bytes[..read]);
            said.extend(lines.iter().filter_map(|line| Seated::read(line)));
            fds += handed.len();
        }
        (said, fds)
    }

    /// Writes each of `lines` to `link`, a line each.
    fn ask(link: &UnixStream, lines: &[&str]) {
        for line in lines {
            writeln!(&mut &*link, "{line}").unwrap();
        }
    }

    #[test]
    fn a_compositor_is_told_whether_the_seat_is_active_and_as_that_changes() {
        let log = Log::default();
        let mut seating = Seating::new(Box::new(Fake(log)), false);
        assert_eq!(seating.name(), "seat9");
        seating.changed(true);
        let (theirs, _) = seating.link().unwrap();
        assert_eq!(told(&theirs), (vec![Seated::Active], 0));
        seating.changed(false);
        seating.changed(true);
        assert_eq!(told(&theirs), (vec![Seated::Paused, Seated::Active], 0));

        // Paused with none running, the next is told it is.
        seating.ended();
        seating.changed(false);
        let (next, _) = seating.link().unwrap();
        assert_eq!(told(&next), (vec![Seated::Paused], 0));
    }

    #[test]
    fn what_a_compositor_asks_for_is_opened_answered_and_closed_as_it_ends() {
        let log = Log::default();
        let mut seating = Seating::new(Box::new(Fake(Rc::clone(&log))), true);
        let (theirs, reading) = seating.link().unwrap();
        told(&theirs);
        ask(
            &theirs,
            &[
                "open /dev/dri/card0",
                "open /refused",
                "open /dev/input/event3",
                "something new",
                "open /dev/dri/card1",
                "close 1",
                "close 9",
                "switch 4",
            ],
        );
        assert_eq!(seating.asked(&reading), PostAction::Continue);
        assert_eq!(
            told(&theirs),
            (
                vec![
                    Seated::Opened(0),
                    Seated::Refused(16),
                    Seated::Opened(1),
                    Seated::Opened(2)
                ],
                3
            )
        );
        // The input device closed, and nothing for one never opened.
        assert_eq!(
            log.borrow().iter().skip(4).collect::<Vec<_>>(),
            ["close", "switch 4"]
        );

        // Gone: both cards closed, the first kept open for its picture.
        drop(theirs);
        assert_eq!(seating.asked(&reading), PostAction::Remove);
        assert!(seating.ended().is_some());
        assert_eq!(
            log.borrow().iter().filter(|said| *said == "close").count(),
            3
        );
        // The next starts counting where the last left off, and is told
        // nothing of the last.
        let (next, reading) = seating.link().unwrap();
        ask(&next, &["open /dev/input/event3"]);
        seating.asked(&reading);
        assert_eq!(told(&next), (vec![Seated::Active, Seated::Opened(3)], 1));
        assert!(seating.ended().is_none(), "no card");
    }

    #[test]
    fn what_is_asked_of_a_compositor_gone_is_done_and_not_answered() {
        let log = Log::default();
        let mut seating = Seating::new(Box::new(Fake(Rc::clone(&log))), true);
        let (theirs, reading) = seating.link().unwrap();
        ask(&theirs, &["open /dev/input/event1"]);
        drop(theirs);
        assert_eq!(seating.asked(&reading), PostAction::Continue);
        assert_eq!(*log.borrow(), ["open /dev/input/event1"]);
        // Asked of with none running, as nothing is.
        seating.ended();
        seating.answer(Asked::Open(PathBuf::from("/dev/input/event2")));
        assert_eq!(seating.opened.len(), 1, "closed as the next one ends");
    }
}
