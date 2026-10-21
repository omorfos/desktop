//! The `omorfos-ask` executable: polkit's agent for the session, which the
//! compositor starts with the `ask` role.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let failure = omorfos_ask::start(&|name| env::var_os(name));
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(io::stderr(), "omorfos-ask: {failure}");
    ExitCode::FAILURE
}
