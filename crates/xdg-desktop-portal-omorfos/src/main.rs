//! The `xdg-desktop-portal-omorfos` executable: what applications ask the
//! desktop through the portals, answered for omorfos.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let failure = xdg_desktop_portal_omorfos::start(&|name| env::var_os(name));
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {failure}");
    ExitCode::FAILURE
}
