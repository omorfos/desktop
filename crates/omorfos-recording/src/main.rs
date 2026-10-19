//! The `omorfos-recording` executable: tells the omorfos desktop, on what
//! it writes, what records from a microphone or a camera, of the cameras
//! in `/dev`.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

/// How long it waits for PipeWire to be there again once it is not.
const AGAIN: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    let mut out = io::stdout().lock();
    loop {
        // It watches until PipeWire goes, or is not there to begin with.
        let _ = omorfos_recording::watch(None, Path::new("/dev"), &mut out, &|| false);
        // Then nothing it knew of records. Once what it says cannot be
        // said, nobody is listening, and it is done.
        if writeln!(out, ".").and_then(|()| out.flush()).is_err() {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(AGAIN);
    }
}
