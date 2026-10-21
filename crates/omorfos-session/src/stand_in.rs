//! What the tests speak to in place of the real thing: a bus of their own,
//! and on it logind, players and the notifications, each written down
//! what it was asked.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::OwnedValue;

/// A bus of the test's own, gone when the test is.
pub(crate) struct Daemon {
    child: Child,
    pub(crate) address: String,
    _directory: tempfile::TempDir,
}

impl Daemon {
    pub(crate) fn start() -> Self {
        let directory = tempfile::TempDir::new().unwrap();
        let listen = format!("--address=unix:path={}/bus", directory.path().display());
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1", &listen])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is needed to test the session");
        let mut address = String::new();
        let stdout = child.stdout.take().unwrap();
        BufReader::new(stdout).read_line(&mut address).unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
            _directory: directory,
        }
    }

    /// A connection that answers as `name`, with `served` at `path`.
    fn serve<T: zbus::object_server::Interface>(
        &self,
        name: &str,
        path: &str,
        served: T,
    ) -> Connection {
        Builder::address(self.address.as_str())
            .unwrap()
            .name(name)
            .unwrap()
            .serve_at(path, served)
            .unwrap()
            .build()
            .unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What a stand-in has been asked, in order.
pub(crate) type Asked<T> = Arc<Mutex<Vec<T>>>;

struct Session {
    asked: Asked<(String, String, u32)>,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    #[zbus(name = "SetBrightness")]
    fn set_brightness(&self, subsystem: String, name: String, brightness: u32) {
        self.asked
            .lock()
            .unwrap()
            .push((subsystem, name, brightness));
    }
}

/// logind, which writes down each brightness it is asked to set.
pub(crate) fn logind(daemon: &Daemon) -> (Connection, Asked<(String, String, u32)>) {
    let asked = Asked::default();
    let session = Session {
        asked: asked.clone(),
    };
    let path = "/org/freedesktop/login1/session/auto";
    (daemon.serve("org.freedesktop.login1", path, session), asked)
}

struct Player {
    status: &'static str,
    asked: Asked<String>,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    #[zbus(property, name = "PlaybackStatus")]
    fn playback_status(&self) -> String {
        self.status.to_owned()
    }

    #[zbus(name = "PlayPause")]
    fn play_pause(&self) {
        self.asked.lock().unwrap().push("PlayPause".to_owned());
    }

    #[zbus(name = "Next")]
    fn next(&self) {
        self.asked.lock().unwrap().push("Next".to_owned());
    }

    #[zbus(name = "Previous")]
    fn previous(&self) {
        self.asked.lock().unwrap().push("Previous".to_owned());
    }

    #[zbus(name = "Stop")]
    fn stop(&self) {
        self.asked.lock().unwrap().push("Stop".to_owned());
    }
}

/// A player called `name` that says it is `status`, and writes down what
/// it is told to do.
pub(crate) fn player(
    daemon: &Daemon,
    name: &str,
    status: &'static str,
) -> (Connection, Asked<String>) {
    let asked = Asked::default();
    let player = Player {
        status,
        asked: asked.clone(),
    };
    let name = format!("org.mpris.MediaPlayer2.{name}");
    (
        daemon.serve(&name, "/org/mpris/MediaPlayer2", player),
        asked,
    )
}

/// A player called `name` that has taken its name and answers nothing.
pub(crate) fn mute(daemon: &Daemon, name: &str) -> Connection {
    let name = format!("org.mpris.MediaPlayer2.{name}");
    let builder = Builder::address(daemon.address.as_str()).unwrap();
    builder.name(name).unwrap().build().unwrap()
}

/// A notification as the stand-in heard it: its icon, its title, and its
/// level and tag.
pub(crate) type Heard = (String, String, i32, String);

struct Notifications {
    asked: Asked<Heard>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[zbus(name = "Notify")]
    #[expect(clippy::too_many_arguments, reason = "as the specification has it")]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the interface macro hands each argument over as it is sent"
    )]
    fn notify(
        &self,
        program: String,
        replaces: u32,
        icon: String,
        title: String,
        body: String,
        actions: Vec<String>,
        hints: std::collections::HashMap<String, OwnedValue>,
        stays: i32,
    ) -> u32 {
        // What is the same in every one the session sends: a new one each
        // time, with no words or actions, that goes by itself soon.
        let sent = (
            program.as_str(),
            replaces,
            body.as_str(),
            actions.len(),
            stays,
        );
        assert_eq!(sent, ("omorfos", 0, "", 0, 1500));
        let mut hints = hints;
        let value = hints
            .remove("value")
            .and_then(|value| i32::try_from(value).ok());
        let tag = hints
            .remove("synchronous")
            .and_then(|tag| String::try_from(tag).ok());
        let heard = (icon, title, value.unwrap(), tag.unwrap());
        self.asked.lock().unwrap().push(heard);
        1
    }
}

/// The notifications, which write down each they are sent.
pub(crate) fn notifications(daemon: &Daemon) -> (Connection, Asked<Heard>) {
    let asked = Asked::default();
    let notifications = Notifications {
        asked: asked.clone(),
    };
    let name = "org.freedesktop.Notifications";
    (
        daemon.serve(name, "/org/freedesktop/Notifications", notifications),
        asked,
    )
}

/// Where the stand-in for `wpctl` that behaves as `kind` is.
pub(crate) fn wpctl(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake")
        .join(kind)
}

/// Backlights as the kernel lists them, each a name, a kind, and how
/// bright it is out of how bright it goes, written as the kernel would.
pub(crate) fn backlights(each: &[(&str, &str, &str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::TempDir::new().unwrap();
    for (name, kind, now, most) in each {
        let backlight = directory.path().join(name);
        std::fs::create_dir(&backlight).unwrap();
        for (file, said) in [
            ("type", kind),
            ("brightness", now),
            ("max_brightness", most),
        ] {
            if !said.is_empty() {
                std::fs::write(backlight.join(file), format!("{said}\n")).unwrap();
            }
        }
    }
    directory
}
