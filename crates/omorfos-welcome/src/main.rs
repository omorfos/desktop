//! The `omorfos-welcome` executable: the welcome card of the omorfos
//! desktop.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::answered;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&omorfos_welcome::ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let first = arguments.iter().any(|argument| argument == "--first");
    let outcome = omorfos_welcome::start(&|name| env::var_os(name), first);
    // The session ending is how it is told to stop, and nothing to say.
    let outcome = outcome.or_else(omorfos_shell::ended::quietly);
    let failure = outcome.as_ref().err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure.map(|failure| writeln!(io::stderr(), "omorfos-welcome: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
