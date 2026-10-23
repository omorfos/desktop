//! The `omorfos-ask` executable: polkit's agent for the session, which the
//! compositor starts with the `ask` role.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let failure = omorfos_ask::start(&|name| env::var_os(name));
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "omorfos-ask: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
