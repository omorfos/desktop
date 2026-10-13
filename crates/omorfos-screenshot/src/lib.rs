//! Takes pictures of the screen in the omorfos desktop.
//!
//! Run by itself it takes a picture of every screen and saves each as a
//! PNG among the user's pictures, in a folder called `Screenshots`. It can
//! take the window with focus in place of the screens, with `--window`, and
//! put the picture on the clipboard as well or instead: `screenshots.destination`
//! in `config.yaml` says which, and `--to` says it for one run.
//!
//! The compositor gives pictures only to a program it started for that,
//! with `may: [capture]`, and the clipboard only to one started with
//! `may: [clipboard]`, which is how the built-in `Print` keys start this
//! one. Started any other way it is refused and says so.

mod client;
mod save;

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use client::{Failure, Shot};
use omorfos_config::{Kept, Store};
pub use save::{encode, pictures, save, together};

/// How the program is used, said when it is used otherwise.
const USAGE: &str = "usage: omorfos-screenshot [--window] [--to file|clipboard|both]";

/// What a run was told on its command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Told {
    /// Where to put its picture, if it was told.
    pub to: Option<Kept>,
    /// Whether to take the window with focus in place of the screens.
    pub window: bool,
}

/// What a run was told.
///
/// # Errors
///
/// Returns how the program is used, if `arguments` are not that.
pub fn told(arguments: &[OsString]) -> Result<Told, Failure> {
    let mut told = Told::default();
    let mut words = arguments.iter().map(|word| word.to_str());
    while let Some(word) = words.next() {
        match word {
            Some("--window") => told.window = true,
            Some("--to") => {
                let place = words.next().flatten().and_then(Kept::named);
                told.to = Some(place.ok_or(USAGE)?);
            }
            _ => return Err(USAGE.into()),
        }
    }
    Ok(told)
}

/// What the window with focus is called in the list of windows, as the
/// compositor says at its socket for scripts. Nothing, if it cannot be
/// asked or no window has focus.
fn focused(socket: Option<PathBuf>) -> Option<String> {
    let client = socket.and_then(|socket| omorfos_ipc::Client::connect(&socket).ok());
    let reply = client.and_then(|mut client| client.request(&omorfos_ipc::Request::State).ok());
    let state = reply.and_then(omorfos_ipc::Reply::into_state);
    let mut windows = state.into_iter().flat_map(|state| state.windows);
    windows
        .find(|window| window.focused)
        .map(|window| window.handle)
}

/// Takes a picture of every screen, puts it where it is to go, and says
/// on `out` where each was saved, a line to a picture; or says on
/// `problems` why not. Returns whether it failed.
///
/// A picture put on the clipboard is held there by this program, so it
/// does not return until something else is copied.
pub fn run(
    variable: &dyn Fn(&str) -> Option<OsString>,
    arguments: &[OsString],
    out: &mut dyn Write,
    problems: &mut dyn Write,
) -> bool {
    let outcome = start(variable, arguments, out);
    // If even this cannot be said, there is nobody to say it to.
    let _ = outcome
        .as_ref()
        .map_err(|failure| writeln!(problems, "omorfos-screenshot: {failure}"));
    outcome.is_err()
}

/// Takes a picture of every screen and puts it where it is to go, saying
/// on `out` where each was saved.
///
/// `variable` looks up an environment variable. The compositor is found
/// by `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, unless it handed over a
/// connection; the configuration by `XDG_CONFIG_HOME` and `HOME`; and
/// the folder for pictures by what `user-dirs.dirs` there says.
///
/// # Errors
///
/// Returns why no picture was taken, or why one could not be saved or
/// put on the clipboard.
pub fn start(
    variable: &dyn Fn(&str) -> Option<OsString>,
    arguments: &[OsString],
    out: &mut dyn Write,
) -> Result<(), Failure> {
    let told = told(arguments)?;
    let get = |name: &str| variable(name).filter(|value| !value.is_empty());
    let wayland = get("XDG_RUNTIME_DIR").zip(get("WAYLAND_DISPLAY"));
    let wayland = wayland.map(|(directory, display)| Path::new(&directory).join(display));
    let home = get("HOME");
    let config = get("XDG_CONFIG_HOME");
    let config = omorfos_config::config_dir(config.as_deref(), home.as_deref());
    let Some(((wayland, home), config)) = wayland.zip(home.map(PathBuf::from)).zip(config) else {
        let said = "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME \
                    is not set";
        return Err(said.into());
    };
    // The folder the configuration is in is one beside where the user's
    // folders are named.
    let beside = config.parent().map(|above| above.join("user-dirs.dirs"));
    let named = beside.and_then(|file| fs::read_to_string(file).ok());
    let folder = pictures(named.as_deref(), &home).join("Screenshots");
    let kept = Store::open(config)
        .current()
        .general
        .screenshots
        .destination;
    let kept = told.to.unwrap_or(kept);
    let window = if told.window {
        let socket = get("OMORFOS_SOCKET");
        let (runtime, display) = (get("XDG_RUNTIME_DIR"), get("WAYLAND_DISPLAY"));
        let socket =
            omorfos_ipc::socket_path(socket.as_deref(), runtime.as_deref(), display.as_deref());
        let focused = focused(socket);
        Some(focused.ok_or("there is no window with focus to take a picture of")?)
    } else {
        None
    };

    let (shots, camera) = client::shoot(&wayland, window)?;
    let saved = if kept.saves() {
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
        saved.map_err(|error| format!("{}: {error}", folder.display()))?
    } else {
        Vec::new()
    };
    // If where they went cannot be said, they are there all the same.
    let lines = saved.iter().map(|path| format!("{}\n", path.display()));
    let _ = out.write_all(lines.collect::<String>().as_bytes());
    let _ = out.flush();
    if kept.copies() {
        let mut png = Vec::new();
        let encoded = encode(&together(&shots), &mut png);
        encoded.map_err(|_| "there is no screen to take a picture of")?;
        camera.hold(png)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str) -> OsString {
        OsString::from(text)
    }

    #[test]
    fn a_run_can_be_told_where_its_picture_goes() {
        assert_eq!(told(&[]).unwrap(), Told::default());
        let to = |place: &str| told(&[word("--to"), word(place)]).map(|told| told.to);
        assert_eq!(to("file").unwrap(), Some(Kept::File));
        assert_eq!(to("clipboard").unwrap(), Some(Kept::Clipboard));
        assert_eq!(to("both").unwrap(), Some(Kept::Both));

        // And to take the window with focus, with or without that.
        let window = told(&[word("--window")]).unwrap();
        assert_eq!((window.window, window.to), (true, None));
        let both = told(&[word("--to"), word("clipboard"), word("--window")]).unwrap();
        assert_eq!((both.window, both.to), (true, Some(Kept::Clipboard)));
    }

    #[test]
    fn told_anything_else_it_says_how_it_is_used() {
        let wrong: [&[OsString]; 4] = [
            &[word("--to"), word("printer")],
            &[word("--to")],
            &[word("--help")],
            &[word("--to"), word("file"), word("more")],
        ];
        for arguments in wrong {
            let said = told(arguments).unwrap_err().to_string();
            assert!(said.starts_with("usage: omorfos-screenshot"), "{said}");
        }
    }
}
