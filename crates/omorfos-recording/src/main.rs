//! The `omorfos-recording` executable: tells the omorfos desktop, on what
//! it writes, what records from a microphone or a camera, of the cameras
//! in `/dev`.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use omorfos_ipc::help::{About, answered};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

/// How long it waits for PipeWire to be there again once it is not.
const AGAIN: Duration = Duration::from_secs(2);

/// Whether nobody reads what is written on `out` any more, as when the
/// compositor that started this has gone: the other end of it is closed.
fn unheard(out: BorrowedFd<'_>) -> bool {
    let mut polled = [PollFd::from_borrowed_fd(out, PollFlags::empty())];
    let _ = poll(&mut polled, Some(&Timespec::default()));
    polled[0]
        .revents()
        .intersects(PollFlags::ERR | PollFlags::HUP)
}

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-recording",
    what: "Tells the omorfos desktop what records from a microphone or a camera, as PipeWire knows it.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "The omorfos compositor starts it, and reads what it writes.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    // What it says is only heard while the compositor is there, and PipeWire
    // may have nothing new to say for a long time: it looks each turn.
    let gone = || unheard(stdout.as_fd());
    loop {
        // It watches until PipeWire goes, or is not there to begin with, or
        // nobody listens.
        let _ = omorfos_recording::watch(None, Path::new("/dev"), &mut out, &gone);
        // Then nothing it knew of records. Once what it says cannot be
        // said, nobody is listening, and it is done.
        if writeln!(out, ".").and_then(|()| out.flush()).is_err() {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(AGAIN);
    }
}
