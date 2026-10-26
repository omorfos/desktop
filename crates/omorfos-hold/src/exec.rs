//! Becoming another program, in this process.
//!
//! A program that becomes another ends without writing down what coverage
//! it counted, so this file is excluded from the coverage gate (see
//! `docs/adr/0002-coverage-policy.md`). It decides nothing: what to become,
//! and with what, is worked out in `renew`, which is tested.

use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use crate::renew::{self, TAKING, TAKING_BACK};
use crate::{Hold, Which};

/// Becomes the holder's own new version, if there is one to become. Says
/// on `log` why not, if it could not be.
pub(crate) fn renew(hold: &mut Hold, which: Which, log: &mut dyn Write) {
    if let Some((own, handing)) = hold.renewal(which, log) {
        let error = become_program(&own, TAKING, handing);
        let _ = writeln!(
            log,
            "omorfos-hold: cannot become its own new version: {error}"
        );
    }
}

/// Becomes the version before again, handing it back `text` and `fds`.
/// Returns why it could not.
#[must_use]
pub fn give_back(text: &str, fds: &[OwnedFd]) -> io::Error {
    match renew::handing_back(text, fds) {
        Ok((old, handing)) => become_program(&old, TAKING_BACK, handing),
        Err(error) => error,
    }
}

/// Runs `program` in this process with `argument`, reading from
/// `handing`. Returns only if it could not.
fn become_program(program: &Path, argument: &str, handing: UnixStream) -> io::Error {
    Command::new(program)
        .arg0("omorfos-hold")
        .arg(argument)
        .stdin(OwnedFd::from(handing))
        .exec()
}
