//! The `xdg-desktop-portal-omorfos` executable: what applications ask the
//! desktop through the portals, answered for omorfos.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "xdg-desktop-portal-omorfos",
    what: "What applications ask the desktop through the portals, answered for omorfos.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "xdg-desktop-portal starts it over D-Bus, as omorfos.portal says.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let failure = xdg_desktop_portal_omorfos::start(&|name| env::var_os(name));
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
