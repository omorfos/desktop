//! Takes pictures of the screen in the omorfos desktop.
//!
//! Run by itself it takes a picture of every screen and saves each as a
//! PNG among the user's pictures, in a folder called `Screenshots`. The
//! compositor gives pictures only to a program it started for that, with
//! `may: [capture]`, which is how the built-in `Print` key starts this
//! one; started any other way it is refused and says so.

mod client;
mod save;

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use client::{Failure, Shot};
pub use save::{pictures, save};

/// Takes a picture of every screen, saves them, and says where on `out`,
/// a line to a picture, or says on `problems` why not. Returns whether it
/// failed.
pub fn run(
    variable: &dyn Fn(&str) -> Option<OsString>,
    out: &mut dyn Write,
    problems: &mut dyn Write,
) -> bool {
    let outcome = start(variable);
    // If even this cannot be said, there is nobody to say it to.
    let _ = match &outcome {
        Ok(saved) => {
            let lines = saved.iter().map(|path| format!("{}\n", path.display()));
            out.write_all(lines.collect::<String>().as_bytes())
        }
        Err(failure) => writeln!(problems, "omorfos-screenshot: {failure}"),
    };
    outcome.is_err()
}

/// Takes a picture of every screen and saves them, and returns where.
///
/// `variable` looks up an environment variable. The compositor is found
/// by `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, unless it handed over a
/// connection, and the folder for pictures by `HOME` and what
/// `user-dirs.dirs`, under `XDG_CONFIG_HOME`, says.
///
/// # Errors
///
/// Returns why no picture was taken, or why one could not be saved.
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Result<Vec<PathBuf>, Failure> {
    let get = |name: &str| variable(name).filter(|value| !value.is_empty());
    let wayland = get("XDG_RUNTIME_DIR").zip(get("WAYLAND_DISPLAY"));
    let wayland = wayland.map(|(directory, display)| Path::new(&directory).join(display));
    let Some((wayland, home)) = wayland.zip(get("HOME").map(PathBuf::from)) else {
        let said = "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME \
                    is not set";
        return Err(said.into());
    };
    let config = get("XDG_CONFIG_HOME").map_or_else(|| home.join(".config"), PathBuf::from);
    let told = fs::read_to_string(config.join("user-dirs.dirs")).ok();
    let folder = pictures(told.as_deref(), &home).join("Screenshots");

    let shots = client::shoot(&wayland)?;
    let stamp = jiff::Zoned::now().strftime("%Y-%m-%d %H-%M-%S").to_string();
    // A screen is named in the file only when there is more than one.
    let several = shots.len() > 1;
    let saved = fs::create_dir_all(&folder).and_then(|()| {
        let each = shots.iter().map(|shot| {
            let screen = several.then_some(shot.screen.as_str());
            save(&folder, &stamp, screen, shot)
        });
        each.collect::<Result<Vec<_>, _>>()
    });
    saved.map_err(|error| format!("{}: {error}", folder.display()).into())
}
