//! What applications ask the desktop through the portals, answered for
//! omorfos.
//!
//! An application that wants to know something only the desktop can say,
//! or to do something only the desktop can let it, asks
//! `xdg-desktop-portal`, which every desktop shares. That passes the
//! question to a program of the desktop's own, and this is omorfos's.
//!
//! So far it answers one thing: how the desktop looks, light or dark and
//! in what colour, so that applications can look the same. It reads that
//! from the theme, and tells applications when the theme changes.

mod appearance;
mod bus;
mod prompt;

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub use appearance::Appearance;
pub use bus::{Asked, LOOKS, Look, NAME, PATH, Portal, REQUEST, Response, SCREENSHOT, SETTINGS};
use omorfos_config::{Store, Watcher};
pub use prompt::{Answer, Desk, asking, button_at, paint};
use rustix::event::{PollFd, PollFlags, poll};

/// Why the program cannot go on, in words for the user.
pub type Failure = Box<dyn Error + Send + Sync>;

/// How the desktop whose configuration is in a folder looks.
fn look(config: &Path) -> Appearance {
    Appearance::of(&Store::open(config).current().theme)
}

/// Answers the portals for the session the environment describes, until
/// it cannot go on, and returns why.
///
/// `variable` looks a variable of the environment up. The compositor is
/// found by `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, unless it handed over
/// a connection; the bus is at `DBUS_SESSION_BUS_ADDRESS`; and the
/// configuration is found as the compositor finds it, by `XDG_CONFIG_HOME`
/// and `HOME`.
///
/// It goes on for as long as the compositor does. The bus outlives a
/// session, and without that a copy would be left answering for a session
/// that has ended.
#[must_use]
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Failure {
    let (home, config) = (variable("HOME"), variable("XDG_CONFIG_HOME"));
    let config = omorfos_config::config_dir(config.as_deref(), home.as_deref());
    let (runtime, display) = (variable("XDG_RUNTIME_DIR"), variable("WAYLAND_DISPLAY"));
    let wayland = runtime.zip(display);
    let wayland = wayland.map(|(runtime, display)| Path::new(&runtime).join(display));
    let address = variable("DBUS_SESSION_BUS_ADDRESS");
    let address = address.as_deref().and_then(|address| address.to_str());
    let found = config.zip(wayland).zip(home.map(PathBuf::from));
    let Some(((config, wayland), home)) = found else {
        return "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                not set"
            .into();
    };
    // Pictures go where the screenshot tool puts them.
    let named = config.parent().map(|above| above.join("user-dirs.dirs"));
    let named = named.and_then(|file| fs::read_to_string(file).ok());
    let folder = omorfos_screenshot::pictures(named.as_deref(), &home).join("Screenshots");
    let session = omorfos_shell::connect(&wayland);
    let session = session.map_err(|error| Failure::from(format!("{}: {error}", wayland.display())));
    let asked = config.clone();
    let portal = session.and_then(|session| {
        let portal = Portal::open(address, Box::new(move || look(&asked)));
        portal.map(|portal| (portal, session))
    });
    let portal = portal.and_then(|(portal, session)| {
        let desk = Desk::new(session, Store::open(&config));
        desk.map(|desk| (portal, desk))
    });
    match portal {
        Ok((portal, desk)) => serve(&portal, &config, desk, &folder),
        Err(failure) => failure,
    }
}

/// An address for a file, as one is given to an application: anything in
/// its path other than letters, numbers, `/` and `-._~` written as a byte
/// in hexadecimal.
#[must_use]
pub fn address(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes().iter();
    let written = bytes.map(|byte| {
        let plain = byte.is_ascii_alphanumeric() || b"/-._~".contains(byte);
        if plain {
            char::from(*byte).to_string()
        } else {
            format!("%{byte:02X}")
        }
    });
    format!("file://{}", written.collect::<String>())
}

/// Takes a picture of every screen, as one, saves it in `folder`, and
/// returns its address. Nothing, if it could not be taken or saved.
fn take(desk: &Desk, folder: &Path) -> Option<String> {
    let shots = omorfos_screenshot::shoot(desk.connection(), None).ok();
    let shot = shots.map(|(shots, _)| omorfos_screenshot::together(&shots));
    let saved = shot.and_then(|shot| {
        let saved = fs::create_dir_all(folder);
        let stamp = omorfos_screenshot::stamp();
        saved
            .and_then(|()| omorfos_screenshot::save(folder, &stamp, None, &shot))
            .ok()
    });
    saved.map(|path| address(&path))
}

/// Says in the session's log what became of something asked, so that
/// what went wrong can be found out. If even that cannot be said, there is
/// nobody to say it to.
fn note(said: &str) {
    let _ = writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {said}");
}

/// What an application is called in the log.
fn called(app: &str) -> &str {
    if app.is_empty() { "a program" } else { app }
}

/// A question that waits on the user: the call to answer, and the object
/// the portal may withdraw it by.
type Pending = (zbus::message::Message, zbus::zvariant::OwnedObjectPath);

/// Answers what waits on the user, and tells applications when the look of
/// the desktop changes, until the bus or the session has gone.
fn serve(portal: &Portal, config: &Path, mut desk: Desk, folder: &Path) -> Failure {
    // A folder that is not there cannot be watched for what is put in it.
    let _ = fs::create_dir_all(config);
    let mut watcher = Watcher::new(config).ok();
    let mut last = look(config);
    let mut pending: Option<Pending> = None;
    loop {
        let Ok(answer) = desk.turn() else {
            return "the session has ended".into();
        };
        let answered = pending.take_if(|_| answer.is_some());
        if let Some((answer, (call, _))) = answer.zip(answered) {
            // The question is taken off the screen at once, and before a
            // picture is taken, so that it is not in it.
            let _ = desk.turn();
            if answer == Answer::Give {
                let taken = take(&desk, folder);
                let response = if let Some(taken) = &taken {
                    note(&format!("the picture was taken and saved at {taken}"));
                    Response::Given
                } else {
                    note("the user said yes, and the picture could not be taken or saved");
                    Response::Failed
                };
                portal.respond(&call, response, taken);
            } else {
                note("the user said no");
                portal.respond(&call, Response::Refused, None);
            }
        }
        let asking = portal.asking();
        let mut sources = vec![
            PollFd::new(portal, PollFlags::IN),
            PollFd::new(&asking, PollFlags::IN),
            PollFd::new(&desk, PollFlags::IN),
        ];
        sources.extend(
            watcher
                .as_ref()
                .map(|watcher| PollFd::new(watcher, PollFlags::IN)),
        );
        // If the wait fails, what there is to do is looked for all the same.
        let _ = poll(&mut sources, None);
        let gone = sources.first().is_some_and(|bus| !bus.revents().is_empty());
        drop(sources);
        if gone {
            return "the bus has gone, or another copy has taken over".into();
        }
        for asked in portal.asked() {
            match asked {
                // One question at a time: another while one is up fails.
                Asked::Screenshot { call, app, .. } if pending.is_some() => {
                    let app = called(&app);
                    note(&format!(
                        "{app} asked for a picture while another was being asked about"
                    ));
                    portal.respond(&call, Response::Failed, None);
                }
                // Nor is one that cannot be asked.
                Asked::Screenshot { call, request, app } => {
                    note(&format!(
                        "{} asked for a picture of the screen",
                        called(&app)
                    ));
                    if desk.ask(&app) {
                        pending = Some((call, request));
                    } else {
                        note("the user cannot be asked: the compositor does not offer the means");
                        portal.respond(&call, Response::Failed, None);
                    }
                }
                Asked::Withdrawn(request) => {
                    let withdrawn = pending.take_if(|(_, asked)| *asked == request);
                    if let Some((call, _)) = withdrawn {
                        note("the portal no longer wants the picture it asked for");
                        desk.dismiss();
                        portal.respond(&call, Response::Refused, None);
                    }
                }
            }
        }
        // What is said to have changed is read, so that it is not said
        // again. Whether anything did is seen by looking.
        let changed = watcher.as_mut().map(Watcher::drain);
        if changed.is_some_and(|changed| changed.unwrap_or(false)) {
            desk.reread();
        }
        let now = look(config);
        portal.changed(last, now);
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use std::os::unix::net::UnixListener;

    use super::*;
    use crate::bus::tests::Daemon;

    /// A stand-in for a compositor, which hangs up on what connects.
    struct Compositor {
        _folder: tempfile::TempDir,
        runtime: OsString,
        listener: UnixListener,
    }

    impl Compositor {
        fn new() -> Self {
            let folder = tempfile::TempDir::new().unwrap();
            let listener = UnixListener::bind(folder.path().join("wayland-test")).unwrap();
            Self {
                runtime: folder.path().as_os_str().to_owned(),
                _folder: folder,
                listener,
            }
        }
    }

    #[test]
    fn an_application_without_a_name_is_called_a_program() {
        assert_eq!(called("org.example.App"), "org.example.App");
        assert_eq!(called(""), "a program");
    }

    #[test]
    fn an_address_is_given_for_a_file_with_what_would_be_misread_written_out() {
        let path = Path::new("/home/ada/Pictures/Screenshots/Screenshot 2026-10-14 10-00-00.png");
        assert_eq!(
            address(path),
            "file:///home/ada/Pictures/Screenshots/Screenshot%202026-10-14%2010-00-00.png"
        );
        assert_eq!(address(Path::new("/a/ü#~_.x")), "file:///a/%C3%BC%23~_.x");
    }

    #[test]
    fn it_says_why_it_cannot_begin() {
        let said = start(&|_| None).to_string();
        assert!(said.contains("not an omorfos session"), "{said}");

        // There is no compositor where it is said to be.
        let variable = |name: &str| match name {
            "HOME" | "XDG_RUNTIME_DIR" => Some(OsString::from("/nowhere")),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
            _ => None,
        };
        let said = start(&variable).to_string();
        assert!(said.starts_with("/nowhere/wayland-none: "), "{said}");

        // There is one, and no bus.
        let compositor = Compositor::new();
        let runtime = compositor.runtime.clone();
        let variable = move |name: &str| match name {
            "HOME" => Some(OsString::from("/nowhere")),
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-test")),
            "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from("unix:path=/nowhere/bus")),
            _ => None,
        };
        let said = start(&variable).to_string();
        assert!(!said.contains("not an omorfos session"), "{said}");

        // There are both, and what seemed a compositor hangs up.
        let compositor = Compositor::new();
        let hangs_up = compositor.runtime.clone();
        let daemon = Daemon::start();
        let address = daemon.address.clone();
        let variable = move |name: &str| match name {
            "HOME" => Some(OsString::from("/nowhere")),
            "XDG_RUNTIME_DIR" => Some(hangs_up.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-test")),
            "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from(&address)),
            _ => None,
        };
        let refused = thread::spawn(move || start(&variable).to_string());
        let (stream, _) = compositor.listener.accept().unwrap();
        drop(stream);
        let said = refused.join().unwrap();
        assert!(!said.contains("not an omorfos session"), "{said}");
    }
}
