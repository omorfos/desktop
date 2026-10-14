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

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub use appearance::Appearance;
pub use bus::{LOOKS, Look, NAME, PATH, Portal, SETTINGS};
use omorfos_config::{Store, Watcher};
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
    let Some((config, wayland)) = config.zip(wayland) else {
        return "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                not set"
            .into();
    };
    let session = omorfos_shell::connect(&wayland);
    let session = session.map_err(|error| Failure::from(format!("{}: {error}", wayland.display())));
    let asked = config.clone();
    let portal = session.and_then(|session| {
        let portal = Portal::open(address, Box::new(move || look(&asked)));
        portal.map(|portal| (portal, session))
    });
    match portal {
        Ok((portal, session)) => serve(&portal, &config, session),
        Err(failure) => failure,
    }
}

/// Tells applications when the look of the desktop changes, until the bus
/// or the session has gone.
fn serve(portal: &Portal, config: &Path, mut session: UnixStream) -> Failure {
    // A folder that is not there cannot be watched for what is put in it.
    let _ = fs::create_dir_all(config);
    let mut watcher = Watcher::new(config).ok();
    let mut last = look(config);
    loop {
        let mut sources = vec![
            PollFd::new(portal, PollFlags::IN),
            PollFd::new(&session, PollFlags::IN),
        ];
        sources.extend(
            watcher
                .as_ref()
                .map(|watcher| PollFd::new(watcher, PollFlags::IN)),
        );
        // If the wait fails, what there is to do is looked for all the same.
        let _ = poll(&mut sources, None);
        let ready = |index: usize| {
            sources
                .get(index)
                .is_some_and(|it| !it.revents().is_empty())
        };
        let (gone, said) = (ready(0), ready(1));
        drop(sources);
        if gone {
            return "the bus has gone, or another copy has taken over".into();
        }
        // Nothing is asked of the compositor, so nothing is to come from it
        // but its going. Anything that does come is passed over.
        let mut heard = [0; 256];
        if said && session.read(&mut heard).unwrap_or(0) == 0 {
            return "the session has ended".into();
        }
        // What is said to have changed is read, so that it is not said
        // again. Whether anything did is seen by looking.
        let _ = watcher.as_mut().map(Watcher::drain);
        let now = look(config);
        portal.changed(last, now);
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use std::thread::{self, JoinHandle};

    use std::io::Write;
    use std::os::unix::net::UnixListener;

    use super::*;
    use crate::bus::tests::{Daemon, listen_for_changes, next_change, read_all, scheme};

    /// A stand-in for the compositor: a socket the program connects to,
    /// and the connection it makes, once it has made it.
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

        /// The program's connection to it.
        fn accepted(&self) -> UnixStream {
            self.listener.accept().unwrap().0
        }
    }

    /// Starts the program on a bus, with a folder for a home, in the
    /// session of a compositor.
    fn started(daemon: &Daemon, home: &Path, compositor: &Compositor) -> JoinHandle<String> {
        let (address, home) = (daemon.address.clone(), home.to_owned());
        let runtime = compositor.runtime.clone();
        let running = thread::spawn(move || {
            let variable = |name: &str| match name {
                "HOME" => Some(home.clone().into_os_string()),
                "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from(&address)),
                "XDG_RUNTIME_DIR" => Some(runtime.clone()),
                "WAYLAND_DISPLAY" => Some(OsString::from("wayland-test")),
                _ => None,
            };
            start(&variable).to_string()
        });
        // It has the name once the bus says someone has.
        let peer = daemon.peer();
        let bus = "org.freedesktop.DBus";
        let owned = || {
            let asked = peer.call_method(Some(bus), "/", Some(bus), "NameHasOwner", &(NAME,));
            asked.unwrap().body().deserialize::<bool>().unwrap()
        };
        while !owned() && !running.is_finished() {
            thread::sleep(std::time::Duration::from_millis(2));
        }
        running
    }

    #[test]
    fn applications_are_told_how_the_theme_looks_and_when_it_changes() {
        let mut daemon = Daemon::start();
        let home = tempfile::TempDir::new().unwrap();
        let compositor = Compositor::new();
        let running = started(&daemon, home.path(), &compositor);
        let peer = daemon.peer();
        assert_eq!(
            scheme(&read_all(&peer, &[LOOKS])),
            1,
            "the built-in theme is dark"
        );

        // The theme is made light, and applications are told so. A file
        // that is not the theme changes nothing and says nothing.
        let mut heard = listen_for_changes(&peer);
        let config = home.path().join(".config/omorfos");
        fs::write(config.join("notes.txt"), "nothing to do with it").unwrap();
        let light = "menu { background: #ffffff; color: #101010; }";
        fs::write(config.join("theme.css"), light).unwrap();
        let (key, value) = next_change(&mut heard);
        assert_eq!(
            (key.as_str(), value.downcast_ref::<u32>().unwrap()),
            ("color-scheme", 2)
        );
        assert_eq!(scheme(&read_all(&peer, &[LOOKS])), 2);

        daemon.stop();
        assert!(running.join().unwrap().starts_with("the bus has gone"));
    }

    #[test]
    fn it_ends_when_the_session_does() {
        let daemon = Daemon::start();
        let home = tempfile::TempDir::new().unwrap();
        let compositor = Compositor::new();
        let running = started(&daemon, home.path(), &compositor);
        let mut session = compositor.accepted();
        // What the compositor says, which it would not, is passed over.
        session.write_all(b"something").unwrap();
        assert_eq!(scheme(&read_all(&daemon.peer(), &[LOOKS])), 1);
        assert!(!running.is_finished());
        drop(session);
        assert_eq!(running.join().unwrap(), "the session has ended");
    }

    #[test]
    fn a_configuration_that_cannot_be_watched_is_still_answered_for() {
        let mut daemon = Daemon::start();
        let home = tempfile::TempDir::new().unwrap();
        // A home that is a file has no folder in it to watch.
        let file = home.path().join("file");
        fs::write(&file, "").unwrap();
        let compositor = Compositor::new();
        let running = started(&daemon, &file, &compositor);
        assert_eq!(scheme(&read_all(&daemon.peer(), &[LOOKS])), 1);
        daemon.stop();
        assert!(running.join().unwrap().starts_with("the bus has gone"));
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
    }
}
