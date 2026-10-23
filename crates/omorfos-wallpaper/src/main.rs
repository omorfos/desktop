//! The `omorfos-wallpaper` executable: the wallpaper of the omorfos
//! desktop.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    // The wallpaper runs until the session ends, so it only ever returns
    // with the reason it could not go on.
    let failure = omorfos_wallpaper::start(
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        env::var_os("WAYLAND_DISPLAY").as_deref(),
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("XDG_CACHE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    );
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(io::stderr(), "omorfos-wallpaper: {failure}");
    ExitCode::FAILURE
}
