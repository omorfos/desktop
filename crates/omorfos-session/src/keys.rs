//! Keys and passphrases forgotten as the session locks, where the
//! configuration names them, so that a locked machine left behind has no
//! key to use: those `ssh-agent` holds, and the passphrases `gpg-agent`
//! has kept, of GnuPG's keys and of the SSH keys it holds, and the PIN a
//! smartcard was given, which the card itself keeps until its daemon
//! lets it go.

use std::ffi::OsStr;

use crate::{Failure, ran};

/// What is to be forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forget {
    /// The keys `ssh-agent` holds.
    pub ssh: bool,
    /// The passphrases `gpg-agent` has kept.
    pub gpg: bool,
}

impl Forget {
    /// What `words` name, each `ssh` or `gpg`; none if they name nothing,
    /// or anything else.
    pub(crate) fn read(words: &[&str]) -> Option<Self> {
        let mut forget = Self {
            ssh: false,
            gpg: false,
        };
        for word in words {
            match *word {
                "ssh" => forget.ssh = true,
                "gpg" => forget.gpg = true,
                _ => return None,
            }
        }
        (forget.ssh || forget.gpg).then_some(forget)
    }
}

/// Forgets what `forget` names, with the programs found on `path`: each
/// tried whatever came of the other. `gpg-agent` is not started to be
/// told, where it is not running. A key on a smartcard is not in an agent
/// at all, and is let use by the card for as long as the smartcard daemon
/// holds it; the daemon is ended, so that the card asks its PIN again.
pub(crate) fn forget(forget: Forget, path: &OsStr) -> Result<(), Failure> {
    let asked: [(bool, &str, &[&str]); 2] = [
        (forget.ssh, "ssh-add", &["-D"]),
        (
            forget.gpg,
            "gpg-connect-agent",
            &["--no-autostart", "reloadagent", "SCD KILLSCD", "/bye"],
        ),
    ];
    let failed: Vec<String> = asked
        .into_iter()
        .filter(|(named, ..)| *named)
        .filter_map(|(_, program, arguments)| ran(program, arguments, path).err())
        .map(|failure| failure.to_string())
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; ").into())
    }
}
