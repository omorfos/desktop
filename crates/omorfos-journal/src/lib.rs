//! What a program the session starts writes, handed to the systemd journal
//! under the program's name, as systemd hands it what a service writes,
//! rather than to the compositor's own log.
//!
//! The journal takes it over a socket of its own: a stream that begins
//! with a few lines saying whose it is and how it is to be kept, after
//! which each line written is an entry. That stream is the program's
//! output; nothing more is done with it here. Where there is no journal,
//! as on a system without systemd, there is no stream, and a program
//! writes where it would have.

use std::io::Write;
use std::net::Shutdown;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

/// Where the journal takes streams.
pub const SOCKET: &str = "/run/systemd/journal/stdout";

/// The level each line is kept at: information, as what a service writes
/// is, unless the program says otherwise.
const INFORMATION: u8 = 6;

/// A stream into the journal at `socket` for what the program named
/// `name` writes, one entry a line; nothing if there is no journal there.
#[must_use]
pub fn stream(socket: &Path, name: &str) -> Option<OwnedFd> {
    // A name is one line of the header, so it holds no line breaks; nor
    // is it empty, which the journal would not take.
    let name: String = name
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let name = if name.is_empty() { "program" } else { &name };
    // Whose it is, its unit (none: the journal finds it), the level, and
    // that it is kept only in the journal: not parsed for levels, nor
    // sent to syslog, the kernel's log or the console.
    let header = format!("{name}\n\n{INFORMATION}\n0\n0\n0\n0\n");
    let connected = UnixStream::connect(socket).ok();
    let told = connected.filter(|stream| (&*stream).write_all(header.as_bytes()).is_ok());
    told.map(|stream| {
        // Nothing comes back; only what is written goes.
        let _ = stream.shutdown(Shutdown::Read);
        OwnedFd::from(stream)
    })
}

/// The name of the program `program` names, as the journal is to know it:
/// its file's, without the directories it is in.
#[must_use]
pub fn name(program: &std::ffi::OsStr) -> String {
    let file = Path::new(program).file_name().unwrap_or(program);
    file.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;

    use super::*;

    /// A journal of the test's own, and what it was told on the first
    /// stream: its header, and then the lines written.
    fn journal(name: &str, written: &str) -> Vec<String> {
        let directory = tempfile::TempDir::new().unwrap();
        let socket = directory.path().join("stdout");
        let listener = UnixListener::bind(&socket).unwrap();
        let stream = stream(&socket, name).unwrap();
        File::from(stream).write_all(written.as_bytes()).unwrap();
        let (heard, _) = listener.accept().unwrap();
        BufReader::new(heard).lines().map(Result::unwrap).collect()
    }

    #[test]
    fn a_program_writes_into_the_journal_under_its_name() {
        let heard = journal("foot", "one\ntwo\n");
        assert_eq!(heard, ["foot", "", "6", "0", "0", "0", "0", "one", "two"]);
    }

    #[test]
    fn a_name_is_one_line_and_never_empty() {
        assert_eq!(journal("foo\nt\r", "")[0], "foot");
        assert_eq!(journal("\n", "")[0], "program");
    }

    #[test]
    fn without_a_journal_there_is_no_stream() {
        let directory = tempfile::TempDir::new().unwrap();
        assert!(stream(&directory.path().join("stdout"), "foot").is_none());
    }

    #[test]
    fn a_program_is_known_by_its_files_name() {
        assert_eq!(name(OsStr::new("/usr/bin/foot")), "foot");
        assert_eq!(name(OsStr::new("code")), "code");
        assert_eq!(name(OsStr::new("/")), "/");
    }
}
