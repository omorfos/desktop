//! The `omorfos-relay` executable: stands between Wayland programs and the
//! omorfos compositor (ADR 60 and ADR 61).

use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::process::ExitCode;

use omorfos_relay::Relay;

const USAGE: &str = "\
Stands between Wayland programs and the omorfos compositor, so that the
compositor can be started again without closing them.

Usage: omorfos-relay [OPTIONS]

It is started by omorfos-hold, which hands it what it serves: there is no
need to start it by hand.

Options:
  -h, --help     Print this help
  -V, --version  Print the version
";

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let (mut out, mut err) = (io::stdout(), io::stderr());
    match arguments.as_slice() {
        [] => relay(&mut err),
        ["-h" | "--help"] => {
            let _ = out.write_all(USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        ["-V" | "--version"] => {
            let _ = writeln!(out, "omorfos-relay {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [unexpected, ..] => {
            let _ = writeln!(
                err,
                "omorfos-relay: unexpected argument `{unexpected}`\n\n{USAGE}"
            );
            ExitCode::from(2)
        }
    }
}

/// Relays until the holder has gone, which is when the session ends.
fn relay(err: &mut dyn Write) -> ExitCode {
    // The holder's connection is what it was started reading from, which is
    // then put to nothing, so that nothing the relay starts, were it to
    // start anything, could speak to the holder in its name. Started with
    // nothing to read from, it has no holder, and fails.
    let holder = io::stdin().as_fd().try_clone_to_owned();
    let nothing = File::open("/dev/null");
    let taken = holder.and_then(|holder| {
        nothing
            .and_then(|nothing| rustix::stdio::dup2_stdin(&nothing).map_err(io::Error::from))
            .map(|()| holder)
    });
    let relayed = taken.map(|holder| {
        let mut relay = Relay::new(holder);
        while relay.step(None, err) {}
        ExitCode::SUCCESS
    });
    relayed.unwrap_or(ExitCode::FAILURE)
}
