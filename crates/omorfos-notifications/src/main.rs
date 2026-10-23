//! The `omorfos-notifications` executable: notifications for the omorfos
//! desktop.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    // It runs until the session ends, so it only ever returns with the
    // reason it could not go on.
    let failure = omorfos_notifications::start(&|name| env::var_os(name));
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "omorfos-notifications: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
