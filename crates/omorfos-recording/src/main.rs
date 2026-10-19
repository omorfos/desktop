//! The `omorfos-recording` executable: tells the omorfos desktop, on what
//! it writes, what records from a microphone or a camera, of the cameras
//! in `/dev`.

use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

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

fn main() -> ExitCode {
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
