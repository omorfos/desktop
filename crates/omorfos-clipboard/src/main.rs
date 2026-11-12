//! The `omorfos-clipboard` executable: keeps what is copied in the omorfos
//! desktop after the program it was copied from closes.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-clipboard",
    what: "Keeps what is copied in the omorfos desktop after the program it was copied from closes.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "The omorfos compositor starts it, with the role that lets it keep the clipboard.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    // It runs until the session ends, so it only ever returns with the
    // reason it could not go on.
    let failure = omorfos_clipboard::start(&|name| env::var_os(name));
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "omorfos-clipboard: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
