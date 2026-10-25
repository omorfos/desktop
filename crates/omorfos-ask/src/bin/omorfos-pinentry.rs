//! The `omorfos-pinentry` executable: what `gpg-agent` starts to ask for a
//! passphrase, named in `gpg-agent.conf` as its `pinentry-program`. It asks
//! in the sealed prompt, in words of omorfos's alone (ADR 58).

use std::os::fd::AsFd;
use std::path::PathBuf;
use std::{env, io};

fn main() {
    let runtime = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let door = runtime.map(|runtime| runtime.join(omorfos_ask::DOOR));
    let stdin = io::stdin();
    // What gpg-agent says is read from, watched while the user is asked.
    let agent = Some(stdin.as_fd());
    omorfos_ask::pinentry::serve(stdin.lock(), io::stdout().lock(), (door.as_deref(), agent));
}
