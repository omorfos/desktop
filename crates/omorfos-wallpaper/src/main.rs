//! The `omorfos-wallpaper` executable: the wallpaper of the omorfos
//! desktop.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-wallpaper",
    what: "The wallpaper of the omorfos desktop: a picture or a colour behind the windows of each screen.",
    usage: "",
    commands: &[],
    options: &[],
    examples: &[],
    notes: "The omorfos compositor starts it, as `wallpaper` in config.yaml says.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    // The wallpaper runs until the session ends, so it only ever returns
    // with the reason it could not go on.
    let failure = omorfos_wallpaper::start(
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        env::var_os("WAYLAND_DISPLAY").as_deref(),
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("XDG_CACHE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
        (
            env::var_os("XDG_DATA_HOME").as_deref(),
            env::var_os("XDG_DATA_DIRS").as_deref(),
        ),
    );
    // The session ending is how it is told to stop, and nothing to say.
    let failure = omorfos_shell::ended::quietly(failure).err();
    // If even that cannot be said, there is nobody to say it to.
    let _ = failure
        .as_ref()
        .map(|failure| writeln!(io::stderr(), "omorfos-wallpaper: {failure}"));
    ExitCode::from(u8::from(failure.is_some()))
}
