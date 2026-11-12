//! The `omorfos-pinentry` executable: what `gpg-agent` starts to ask for a
//! passphrase, named in `gpg-agent.conf` as its `pinentry-program`. It asks
//! in the sealed prompt, in words of omorfos's alone (ADR 58).

use std::ffi::OsString;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-pinentry",
    what: "What gpg-agent starts to ask for a passphrase, in the sealed prompt of the omorfos desktop.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "gpg-agent starts it where `pinentry-program` in gpg-agent.conf names it, and\n\
            speaks with it on its standard input and output.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return;
    }
    let runtime = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let door = runtime.map(|runtime| runtime.join(omorfos_ask::DOOR));
    let stdin = io::stdin();
    // What gpg-agent says is read from, watched while the user is asked.
    let agent = Some(stdin.as_fd());
    omorfos_ask::pinentry::serve(stdin.lock(), io::stdout().lock(), (door.as_deref(), agent));
}
