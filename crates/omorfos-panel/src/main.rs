//! The `omorfos-panel` executable: the panel of the omorfos desktop.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-panel",
    what: "The panel of the omorfos desktop: workspaces and windows along the edge of a screen.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "The omorfos compositor starts it, and again if it ends.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    // The panel runs until the session ends, so it only ever returns with
    // the reason it could not go on.
    let failure = omorfos_panel::start(
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        env::var_os("WAYLAND_DISPLAY").as_deref(),
        env::var_os("OMORFOS_SOCKET").as_deref(),
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    );
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "omorfos-panel: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
