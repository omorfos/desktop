//! What a compositor and `omorfos-hold`, which keeps the seat, say to each
//! other about it (ADR 63).
//!
//! The compositor asks the holder for the devices of the seat, as it would
//! ask libseat, over a connection of their own, a line each; the holder
//! answers each, and says, of its own accord, whether the seat is the
//! session's to use. The holder and the compositor are of different
//! versions whenever one of them is installed: lines are added, never
//! changed, and either passes over one it does not know.

use std::path::PathBuf;

/// What a compositor asks of the seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// To open the device at this path, which is answered with
    /// [`Seated::Opened`] and the device, or [`Seated::Refused`].
    Open(PathBuf),
    /// To close the device opened under this number.
    Close(u32),
    /// To switch to this virtual terminal.
    Switch(i32),
}

/// What the holder says of the seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seated {
    /// The device asked for is open, under this number, and handed with
    /// this line.
    Opened(u32),
    /// The device asked for could not be opened, for this error number.
    Refused(i32),
    /// The seat is not the session's to use for now, as the user is at
    /// another virtual terminal: what was opened on it is not to be used.
    Paused,
    /// The seat is the session's to use.
    Active,
}

impl Asked {
    /// The line the compositor writes, or nothing for a path no line can
    /// carry.
    #[must_use]
    pub fn line(&self) -> Option<String> {
        match self {
            Self::Open(path) => {
                let path = path.to_str().filter(|path| !path.contains('\n'));
                path.map(|path| format!("open {path}\n"))
            }
            Self::Close(device) => Some(format!("close {device}\n")),
            Self::Switch(terminal) => Some(format!("switch {terminal}\n")),
        }
    }

    /// What `line` asks, if it is a request known here. A path to open is
    /// whole: one that is not, from the root, is none.
    #[must_use]
    pub fn read(line: &str) -> Option<Self> {
        let open = line.strip_prefix("open ").map(PathBuf::from);
        let open = open.filter(|path| path.is_absolute()).map(Self::Open);
        let close = number(line, "close ").map(Self::Close);
        let switch = number(line, "switch ").map(Self::Switch);
        open.or(close).or(switch)
    }
}

impl Seated {
    /// The line the holder writes.
    #[must_use]
    pub fn line(self) -> String {
        match self {
            Self::Opened(device) => format!("opened {device}\n"),
            Self::Refused(error) => format!("refused {error}\n"),
            Self::Paused => "paused\n".to_owned(),
            Self::Active => "active\n".to_owned(),
        }
    }

    /// What `line` says, if it is one known here.
    #[must_use]
    pub fn read(line: &str) -> Option<Self> {
        match line {
            "paused" => Some(Self::Paused),
            "active" => Some(Self::Active),
            _ => {
                let opened = number(line, "opened ").map(Self::Opened);
                opened.or_else(|| number(line, "refused ").map(Self::Refused))
            }
        }
    }
}

/// The number `line` gives after `before`, if it is one.
fn number<T: std::str::FromStr>(line: &str, before: &str) -> Option<T> {
    line.strip_prefix(before)
        .and_then(|number| number.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_request_is_read_back_from_its_line() {
        for asked in [
            Asked::Open(PathBuf::from("/dev/dri/card0")),
            Asked::Open(PathBuf::from("/dev/input/event 3")),
            Asked::Close(7),
            Asked::Switch(3),
        ] {
            let line = asked.line().unwrap();
            assert!(line.ends_with('\n'), "{line:?}");
            assert_eq!(Asked::read(line.trim_end()), Some(asked), "{line:?}");
        }
        assert_eq!(Asked::Close(2).line().as_deref(), Some("close 2\n"));
    }

    #[test]
    fn a_path_no_line_can_carry_is_not_asked_for() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(Asked::Open(PathBuf::from("/dev/a\nclose 1")).line(), None);
        let not_text = std::ffi::OsStr::from_bytes(&[b'/', 0xff]);
        assert_eq!(Asked::Open(PathBuf::from(not_text)).line(), None);
    }

    #[test]
    fn a_request_not_known_here_is_none() {
        for line in [
            "",
            "open",
            "open dev/dri/card0",
            "close",
            "close -1",
            "close x",
            "switch",
            "switch 1 2",
            "rename 1",
        ] {
            assert_eq!(Asked::read(line), None, "{line:?}");
        }
    }

    #[test]
    fn each_thing_said_of_the_seat_is_read_back_from_its_line() {
        for seated in [
            Seated::Opened(0),
            Seated::Opened(12),
            Seated::Refused(13),
            Seated::Paused,
            Seated::Active,
        ] {
            let line = seated.line();
            assert!(line.ends_with('\n'), "{line:?}");
            assert_eq!(Seated::read(line.trim_end()), Some(seated), "{line:?}");
        }
        for line in ["", "opened", "opened -1", "refused x", "active!", "pause"] {
            assert_eq!(Seated::read(line), None, "{line:?}");
        }
    }
}
