//! The `omorfos-screenshot` executable: takes a picture of every screen.

use std::ffi::OsString;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-screenshot",
    what: "Takes a picture of every screen of the omorfos desktop, and saves it or puts it on \
           the clipboard.",
    usage: " [--window | --pick] [--to WHERE]",
    commands: &[],
    options: &[
        (
            "--window",
            "Take the window with focus, the whole of it, in place of the screens",
        ),
        (
            "--pick",
            "Pick what to take first: drag over an area, or click a window or a \
             screen",
        ),
        (
            "--to file|clipboard|both",
            "Where the picture goes, whatever `screenshots.destination` in \
             config.yaml says",
        ),
    ],
    examples: &[
        "omorfos-screenshot",
        "omorfos-screenshot --pick --to clipboard",
        "omorfos-screenshot --window --to both",
    ],
    notes: "Only a program the compositor starts with `may: [capture]` may take pictures, and \
            only one with `may: [clipboard]` may put them on the clipboard, as the `Print` keys \
            start it; run any other way it is refused and says so. --pick needs the `panel` and \
            `overlay` roles as well, and --window `panel`.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let variable = |name: &str| env::var_os(name);
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let (mut out, mut problems) = (io::stdout(), io::stderr());
    let failed = omorfos_screenshot::run(&variable, &arguments, &mut out, &mut problems);
    ExitCode::from(u8::from(failed))
}
