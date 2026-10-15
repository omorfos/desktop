//! The `omorfos-clipboard` executable: keeps what is copied in the omorfos
//! desktop after the program it was copied from closes.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    // It runs until the session ends, so it only ever returns with the
    // reason it could not go on.
    let failure = omorfos_clipboard::start(&|name| env::var_os(name));
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(io::stderr(), "omorfos-clipboard: {failure}");
    ExitCode::FAILURE
}
