//! The `xdg-desktop-portal-omorfos` executable: what applications ask the
//! desktop through the portals, answered for omorfos.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let failure = xdg_desktop_portal_omorfos::start(&|name| env::var_os(name));
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
