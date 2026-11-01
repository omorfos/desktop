//! The seat as `omorfos-hold` keeps it (ADR 63).
//!
//! Where the holder keeps the seat, the compositor asks it for the seat's
//! devices over a connection of their own, as it would ask libseat, and is
//! told whether the seat is the session's to use. [`HeldSeat`] is that, as
//! Smithay's [`Session`], so that the graphics card and libinput take their
//! devices from it as they would from libseat.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::rc::Rc;

use omorfos_ipc::hold::{self, Heard};
use omorfos_ipc::seat::{Asked, Seated};
use smithay::backend::session::{AsErrno, Session};
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::rustix::io::Errno;

/// The seat the holder keeps, as the compositor asks it for devices.
#[derive(Debug, Clone)]
pub struct HeldSeat {
    name: String,
    link: Rc<RefCell<Link>>,
}

/// The compositor's end of the connection to the holder for the seat, and
/// what has been read on it.
#[derive(Debug)]
struct Link {
    connection: UnixStream,
    heard: Heard,
    /// Descriptors read and not yet taken: each goes with the next device
    /// said to be opened.
    handed: VecDeque<OwnedFd>,
    /// Answers read and not yet taken: a device opened, with the number it
    /// is known by, or why it was not.
    answers: VecDeque<Result<(u32, OwnedFd), i32>>,
    /// Whether the seat is the session's to use, as last said.
    active: bool,
    /// Whether it is, each time it was said, not yet taken.
    changes: VecDeque<bool>,
    /// The number each device opened is known by to the holder, by its
    /// descriptor here.
    devices: HashMap<RawFd, u32>,
}

/// Why the holder did not do what it was asked: an error number, as libseat
/// says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused(i32);

impl AsErrno for Refused {
    fn as_errno(&self) -> Option<i32> {
        Some(self.0)
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error = io::Error::from_raw_os_error(self.0);
        write!(formatter, "omorfos-hold refused: {error}")
    }
}

impl std::error::Error for Refused {}

impl From<io::Error> for Refused {
    fn from(error: io::Error) -> Self {
        Self(error.raw_os_error().unwrap_or(Errno::IO.raw_os_error()))
    }
}

impl HeldSeat {
    /// The seat called `name` the holder keeps, asked for over
    /// `connection`.
    pub(crate) fn new(name: String, connection: UnixStream) -> Self {
        let link = Link {
            connection,
            heard: Heard::default(),
            handed: VecDeque::new(),
            answers: VecDeque::new(),
            active: false,
            changes: VecDeque::new(),
            devices: HashMap::new(),
        };
        Self {
            name,
            link: Rc::new(RefCell::new(link)),
        }
    }

    /// Waits until the holder says the seat is the session's to use: a
    /// compositor started while the user is at another virtual terminal
    /// may open nothing until they are back. What was said meanwhile is
    /// not to be heard again.
    ///
    /// # Errors
    ///
    /// Returns an error if the holder has gone.
    pub fn wait_until_active(&self) -> io::Result<()> {
        let mut link = self.link.borrow_mut();
        while !link.active {
            link.receive(true)?;
        }
        link.changes.clear();
        Ok(())
    }

    /// Whether the seat is the session's to use, or not, each time the
    /// holder said so since this was last asked, reading what has come
    /// without waiting for more.
    ///
    /// # Errors
    ///
    /// Returns an error once the holder has gone.
    pub fn changes(&self) -> io::Result<Vec<bool>> {
        let mut link = self.link.borrow_mut();
        let mut read = link.receive(false);
        while read.is_ok() {
            read = link.receive(false);
        }
        let ended = read
            .err()
            .filter(|error| error.kind() != io::ErrorKind::WouldBlock);
        ended.map_or_else(|| Ok(link.changes.drain(..).collect()), Err)
    }

    /// What tells when the holder has said something, to wait on.
    ///
    /// # Errors
    ///
    /// Returns an error if it cannot be duplicated.
    pub fn connection(&self) -> io::Result<OwnedFd> {
        self.link.borrow().connection.as_fd().try_clone_to_owned()
    }

    /// Asks the holder `asked`.
    fn ask(&self, asked: &Asked) -> Result<(), Refused> {
        let request = asked.line().ok_or(Refused(Errno::INVAL.raw_os_error()))?;
        let link = self.link.borrow();
        (&link.connection).write_all(request.as_bytes())?;
        Ok(())
    }
}

impl Link {
    /// Reads what the holder has said, waiting for it if `wait`, and takes
    /// it in. Returns how much was read: none once the holder has gone.
    fn receive(&mut self, wait: bool) -> io::Result<usize> {
        let mut bytes = [0; 1024];
        let (read, handed) = if wait {
            hold::read(&self.connection, &mut bytes)
        } else {
            hold::read_now(&self.connection, &mut bytes)
        }?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.handed.extend(handed);
        let lines = self.heard.lines(&bytes[..read]);
        for seated in lines.iter().filter_map(|line| Seated::read(line)) {
            match seated {
                Seated::Opened(number) => {
                    // Said to be opened, and none handed with it, it is
                    // not to be had.
                    let device = self.handed.pop_front();
                    let device = device.ok_or(Errno::IO.raw_os_error());
                    self.answers
                        .push_back(device.map(|device| (number, device)));
                }
                Seated::Refused(error) => self.answers.push_back(Err(error)),
                Seated::Paused | Seated::Active => {
                    self.active = seated == Seated::Active;
                    self.changes.push_back(self.active);
                }
            }
        }
        Ok(read)
    }
}

impl Session for HeldSeat {
    type Error = Refused;

    /// Opens the device at `path`, as the holder opens it through libseat,
    /// whatever the `flags`, as libseat does.
    fn open(&mut self, path: &Path, _flags: OFlags) -> Result<OwnedFd, Refused> {
        self.ask(&Asked::Open(path.to_owned()))?;
        let mut link = self.link.borrow_mut();
        let (number, device) = loop {
            match link.answers.pop_front() {
                Some(answer) => break answer.map_err(Refused)?,
                None => link.receive(true)?,
            };
        };
        link.devices.insert(device.as_raw_fd(), number);
        Ok(device)
    }

    fn close(&mut self, device: OwnedFd) -> Result<(), Refused> {
        let number = self.link.borrow_mut().devices.remove(&device.as_raw_fd());
        drop(device);
        number.map_or(Ok(()), |number| self.ask(&Asked::Close(number)))
    }

    fn change_vt(&mut self, terminal: i32) -> Result<(), Refused> {
        self.ask(&Asked::Switch(terminal))
    }

    fn is_active(&self) -> bool {
        self.link.borrow().active
    }

    fn seat(&self) -> String {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;

    use super::*;

    /// A seat kept by a holder of the test's own, and the holder's end.
    fn held() -> (HeldSeat, UnixStream) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        (HeldSeat::new("seat9".to_owned(), theirs), ours)
    }

    /// The holder says `seated`, handing `device` with it.
    fn say(holder: &UnixStream, seated: Seated, device: Option<&File>) {
        let fds = Vec::from_iter(device.map(AsFd::as_fd));
        hold::give(holder, &seated.line(), &fds).unwrap();
    }

    /// What the holder has been asked since last read.
    fn asked(holder: &UnixStream) -> String {
        holder.set_nonblocking(true).unwrap();
        let mut asked = String::new();
        let _ = (&*holder).read_to_string(&mut asked);
        asked
    }

    #[test]
    fn a_compositor_waits_for_the_seat_to_be_active_and_hears_it_change_after() {
        let (seat, holder) = held();
        assert_eq!(seat.seat(), "seat9");
        assert!(!seat.is_active());
        for seated in [Seated::Paused, Seated::Active] {
            say(&holder, seated, None);
        }
        seat.wait_until_active().unwrap();
        assert!(seat.is_active());
        assert_eq!(
            seat.changes().unwrap(),
            Vec::<bool>::new(),
            "what was waited for is not heard again"
        );
        say(&holder, Seated::Paused, None);
        say(&holder, Seated::Active, None);
        assert_eq!(seat.changes().unwrap(), [false, true]);
        assert_eq!(
            seat.changes().unwrap(),
            Vec::<bool>::new(),
            "nothing more said, nothing waited for"
        );
        assert!(seat.connection().is_ok());
        drop(holder);
        assert!(seat.wait_until_active().is_ok(), "active as it went");
        assert_eq!(
            seat.changes().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof,
            "gone"
        );
    }

    #[test]
    fn a_device_asked_for_is_opened_by_the_holder_and_closed_through_it() {
        let (mut seat, holder) = held();
        let null = File::open("/dev/null").unwrap();
        // Said meanwhile, the seat paused is heard all the same.
        say(&holder, Seated::Paused, None);
        say(&holder, Seated::Opened(4), Some(&null));
        let device = seat.open(Path::new("/dev/dri/card0"), OFlags::RDWR);
        let device = device.unwrap();
        assert!(!seat.is_active());
        assert_eq!(seat.changes().unwrap(), [false]);
        seat.change_vt(3).unwrap();
        seat.close(device).unwrap();
        assert_eq!(asked(&holder), "open /dev/dri/card0\nswitch 3\nclose 4\n");
        // One it did not open is not the holder's to close.
        seat.close(OwnedFd::from(File::open("/dev/null").unwrap()))
            .unwrap();
        assert_eq!(asked(&holder), "");
    }

    #[test]
    fn a_device_the_holder_does_not_open_says_why() {
        let (mut seat, holder) = held();
        say(&holder, Seated::Refused(16), None);
        let refused = seat.open(Path::new("/dev/dri/card0"), OFlags::RDWR);
        assert_eq!(refused.unwrap_err().as_errno(), Some(16));
        // Said to be opened, but with nothing handed.
        say(&holder, Seated::Opened(1), None);
        let lost = seat.open(Path::new("/dev/dri/card0"), OFlags::RDWR);
        assert_eq!(lost.unwrap_err(), Refused(Errno::IO.raw_os_error()));
        // A path no line can carry is not asked for.
        let unsayable = seat.open(Path::new("/dev/a\nb"), OFlags::RDWR);
        assert_eq!(unsayable.unwrap_err(), Refused(Errno::INVAL.raw_os_error()));
        assert_eq!(asked(&holder), "open /dev/dri/card0\nopen /dev/dri/card0\n");

        // The holder gone, nothing is opened, and it says so.
        drop(holder);
        let gone = seat.open(Path::new("/dev/dri/card0"), OFlags::RDWR);
        let gone = gone.unwrap_err();
        assert_eq!(gone.as_errno(), Some(Errno::PIPE.raw_os_error()));
        assert!(gone.to_string().starts_with("omorfos-hold refused: "));
    }

    #[test]
    fn the_holder_gone_while_the_compositor_waits_on_it_is_an_error() {
        let (mut seat, holder) = held();
        // It reads what it is asked, and ends before answering.
        holder.shutdown(std::net::Shutdown::Write).unwrap();
        let gone = seat.open(Path::new("/dev/input/event1"), OFlags::RDWR);
        assert_eq!(gone.unwrap_err(), Refused(Errno::IO.raw_os_error()));
        assert_eq!(asked(&holder), "open /dev/input/event1\n");
        let waited = seat.wait_until_active().unwrap_err();
        assert_eq!(waited.kind(), io::ErrorKind::UnexpectedEof);
    }
}
