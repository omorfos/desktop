//! The `omorfos-hold` executable: holds an omorfos session while its
//! compositor is started again (ADR 60).

use std::env;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use omorfos_hold::renew::{self, CHECK, TAKING, TAKING_BACK};

const USAGE: &str = "\
Holds an omorfos session while its compositor is started again.

Usage: omorfos-hold [OPTIONS]

It is started by omorfos itself, from the login screen, and starts omorfos
beside it in turn: there is no need to start it by hand.

Options:
  -h, --help     Print this help
  -V, --version  Print the version
";

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let (mut out, mut err) = (io::stdout(), io::stderr());
    match arguments.as_slice() {
        [] => hold(&mut err),
        ["-h" | "--help"] => {
            let _ = out.write_all(USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        // What a version of the holder about to become this one asks.
        ["-V" | "--version" | CHECK] => {
            let _ = writeln!(out, "omorfos-hold {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [TAKING] => taking(false, &mut err),
        [TAKING_BACK] => taking(true, &mut err),
        [unexpected, ..] => {
            let _ = writeln!(
                err,
                "omorfos-hold: unexpected argument `{unexpected}`\n\n{USAGE}"
            );
            ExitCode::from(2)
        }
    }
}

/// Holds the session from the start, as the login screen's `omorfos`
/// starts it.
fn hold(err: &mut dyn Write) -> ExitCode {
    let Some(runtime) = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) else {
        let _ = writeln!(err, "omorfos-hold: XDG_RUNTIME_DIR is not set");
        return ExitCode::FAILURE;
    };
    let own = env::current_exe().unwrap_or_default();
    match omorfos_hold::hold(&runtime, own, err) {
        Ok(failed) => ended(failed),
        Err(error) => {
            let _ = writeln!(err, "omorfos-hold: cannot hold the session: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Holds the session as another version of the holder did, with what it
/// handed over as what this reads from; or, being that version taken
/// `back`, as it did before.
fn taking(back: bool, err: &mut dyn Write) -> ExitCode {
    let handed = io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map(UnixStream::from);
    let handed = handed.and_then(|handed| omorfos_ipc::hold::take(&handed));
    let (text, fds) = match handed {
        Ok(handed) => handed,
        Err(error) => {
            let _ = writeln!(err, "omorfos-hold: nothing was handed over: {error}");
            return ExitCode::FAILURE;
        }
    };
    match renew::take(&text, fds) {
        Ok((mut hold, which)) => ended(hold.run_from(which, err)),
        Err((text, fds)) if !back => {
            let said = "cannot take what the version before handed over; going back to it";
            let _ = writeln!(err, "omorfos-hold: {said}");
            let error = omorfos_hold::give_back(&text, &fds);
            let _ = writeln!(
                err,
                "omorfos-hold: cannot go back to the version before: {error}"
            );
            ExitCode::FAILURE
        }
        Err(_) => {
            let _ = writeln!(err, "omorfos-hold: cannot take back what was handed back");
            ExitCode::FAILURE
        }
    }
}

/// How the holder ends, as the session did.
fn ended(failed: bool) -> ExitCode {
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
