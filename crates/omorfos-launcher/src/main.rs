//! The `omorfos-launcher` executable: the launcher of the omorfos desktop.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let outcome = omorfos_launcher::start(&|name| env::var_os(name));
    let failure = outcome.as_ref().err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure.map(|failure| writeln!(io::stderr(), "omorfos-launcher: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
