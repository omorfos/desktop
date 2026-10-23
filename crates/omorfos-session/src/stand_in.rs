//! What the tests speak to in place of the real thing: a bus of their own,
//! and on it logind, players and the notifications, each written down
//! what it was asked.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use omorfos_ipc::{Reply, Request, State, encode};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};

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

/// A program keeping the machine from something, as logind lists it: what
/// from, who, why, how, and its user's and its own numbers.
pub(crate) type Inhibitor = (String, String, String, String, u32, u32);

struct Manager {
    asked: Asked<String>,
    held: Asked<UnixStream>,
    /// What it answers when asked whether sleep, restart and shut down may
    /// be done, in that order.
    can: Arc<Mutex<[&'static str; 3]>>,
    inhibitors: Asked<Inhibitor>,
    /// How long it waits for sleep to be let go, in microseconds.
    delay: u64,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    #[zbus(name = "Inhibit")]
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> OwnedFd {
        assert_eq!((what, mode), ("sleep", "delay"));
        // What `systemd-inhibit --list` shows the user.
        assert_eq!(who, "omorfos");
        assert_ne!(why, "");
        let (ours, theirs) = UnixStream::pair().unwrap();
        self.held.lock().unwrap().push(ours);
        OwnedFd::from(std::os::fd::OwnedFd::from(theirs))
    }

    #[zbus(name = "GetSession")]
    #[expect(clippy::unused_self, reason = "as the interface macro needs it")]
    fn get_session(&self, which: &str) -> OwnedObjectPath {
        assert_eq!(which, "auto");
        OwnedObjectPath::try_from(SESSION).unwrap()
    }

    #[zbus(name = "Suspend")]
    fn suspend(&self, interactive: bool) {
        self.asked
            .lock()
            .unwrap()
            .push(format!("Suspend {interactive}"));
    }

    #[zbus(name = "Reboot")]
    fn reboot(&self, interactive: bool) {
        self.asked
            .lock()
            .unwrap()
            .push(format!("Reboot {interactive}"));
    }

    #[zbus(name = "PowerOff")]
    fn power_off(&self, interactive: bool) {
        self.asked
            .lock()
            .unwrap()
            .push(format!("PowerOff {interactive}"));
    }

    #[zbus(name = "CanSuspend")]
    fn can_suspend(&self) -> String {
        self.can.lock().unwrap()[0].to_owned()
    }

    #[zbus(name = "CanReboot")]
    fn can_reboot(&self) -> String {
        self.can.lock().unwrap()[1].to_owned()
    }

    #[zbus(name = "CanPowerOff")]
    fn can_power_off(&self) -> String {
        self.can.lock().unwrap()[2].to_owned()
    }

    #[zbus(name = "ListInhibitors")]
    fn list_inhibitors(&self) -> Vec<Inhibitor> {
        self.inhibitors.lock().unwrap().clone()
    }

    #[zbus(property, name = "InhibitDelayMaxUSec")]
    const fn inhibit_delay_max(&self) -> u64 {
        self.delay
    }
}

/// Where the stand-in's session is.
pub(crate) const SESSION: &str = "/org/freedesktop/login1/session/_1";

/// logind, standing in for the real one.
pub(crate) struct Logind {
    pub(crate) connection: Connection,
    /// Each brightness it was asked to set.
    pub(crate) brightness: Asked<(String, String, u32)>,
    /// What else it was asked to do, written as the method and its
    /// argument.
    pub(crate) asked: Asked<String>,
    /// The other end of each hold it gave on sleep, which reads as ended
    /// once that hold is let go of.
    pub(crate) held: Asked<UnixStream>,
    /// What it answers when asked whether sleep, restart and shut down may
    /// be done: `yes` to each, unless a test says otherwise.
    pub(crate) can: Arc<Mutex<[&'static str; 3]>>,
    /// The programs it says keep the machine from something.
    pub(crate) inhibitors: Asked<Inhibitor>,
}

impl Logind {
    /// Says that the machine is about to sleep, or has woken.
    pub(crate) fn sleeping(&self, sleeping: bool) {
        let manager = "org.freedesktop.login1.Manager";
        let nobody: Option<&str> = None;
        let path = "/org/freedesktop/login1";
        let said = (sleeping,);
        let connection = &self.connection;
        let emitted = connection.emit_signal(nobody, path, manager, "PrepareForSleep", &said);
        emitted.unwrap();
    }

    /// Says `member` of the session at `path`, as `Lock`.
    pub(crate) fn tell(&self, path: &str, member: &str) {
        let session = "org.freedesktop.login1.Session";
        let nobody: Option<&str> = None;
        let connection = &self.connection;
        connection
            .emit_signal(nobody, path, session, member, &())
            .unwrap();
    }

    /// How many holds on sleep it has given, and whether each is still held.
    pub(crate) fn holds(&self) -> Vec<bool> {
        let held = self.held.lock().unwrap();
        held.iter().map(still_held).collect()
    }
}

/// Whether the other end of `ours` is still open.
fn still_held(ours: &UnixStream) -> bool {
    ours.set_nonblocking(true).unwrap();
    let read = (&*ours).read(&mut [0]);
    !matches!(read, Ok(0))
}

/// logind, which writes down what it is asked, and waits a fifth of a
/// second for sleep to be let go, so that a test need not wait its five.
pub(crate) fn logind(daemon: &Daemon) -> Logind {
    logind_waiting(daemon, Duration::from_millis(200))
}

/// logind, which waits `delay` for sleep to be let go.
pub(crate) fn logind_waiting(daemon: &Daemon, delay: Duration) -> Logind {
    let brightness = Asked::default();
    let session = Session {
        asked: brightness.clone(),
    };
    let (asked, held) = (Asked::default(), Asked::default());
    let can = Arc::new(Mutex::new(["yes"; 3]));
    let inhibitors = Asked::default();
    let manager = Manager {
        asked: Arc::clone(&asked),
        held: Arc::clone(&held),
        can: Arc::clone(&can),
        inhibitors: Arc::clone(&inhibitors),
        delay: u64::try_from(delay.as_micros()).unwrap(),
    };
    let connection = Builder::address(daemon.address.as_str())
        .unwrap()
        .name("org.freedesktop.login1")
        .unwrap()
        .serve_at("/org/freedesktop/login1/session/auto", session)
        .unwrap()
        .serve_at("/org/freedesktop/login1", manager)
        .unwrap()
        .build()
        .unwrap();
    Logind {
        connection,
        brightness,
        asked,
        held,
        can,
        inhibitors,
    }
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
/// level and tag; or, for one that says something went wrong, no level and
/// its words.
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
        // time, with no actions.
        assert_eq!(
            (program.as_str(), replaces, actions.len()),
            ("omorfos", 0, 0)
        );
        let mut hints = hints;
        let value = hints
            .remove("value")
            .and_then(|value| i32::try_from(value).ok());
        let tag = hints
            .remove("synchronous")
            .and_then(|tag| String::try_from(tag).ok());
        // A level has no words and goes by itself soon; a warning has its
        // words, here in place of the tag, and stays.
        let heard = if let Some(value) = value {
            assert_eq!((body.as_str(), stays), ("", 1500));
            (icon, title, value, tag.unwrap())
        } else {
            assert_eq!(stays, 0);
            (icon, title, -1, body)
        };
        self.asked.lock().unwrap().push(heard);
        1
    }
}

/// logind's name taken by something that answers nothing, as a logind
/// too busy to does.
pub(crate) fn mute_logind(daemon: &Daemon) -> Connection {
    let builder = Builder::address(daemon.address.as_str()).unwrap();
    builder
        .name("org.freedesktop.login1")
        .unwrap()
        .build()
        .unwrap()
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

/// The compositor, standing in for the real one: it tells those who
/// subscribe whether the session is locked when a test says, and writes
/// down every other request.
pub(crate) struct Compositor {
    pub(crate) path: PathBuf,
    /// Whether it refuses to lock, as one with no locker does.
    refusing: Arc<AtomicBool>,
    subscribers: Asked<UnixStream>,
    pub(crate) asked: Asked<String>,
    _directory: tempfile::TempDir,
}

impl Compositor {
    pub(crate) fn start() -> Self {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join("ipc.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let (subscribers, asked) = (Asked::default(), Asked::default());
        let (kept, heard) = (Arc::clone(&subscribers), Arc::clone(&asked));
        let refusing = Arc::new(AtomicBool::new(false));
        let refuses = Arc::clone(&refusing);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let request: Request = omorfos_ipc::decode(&line).unwrap();
                if request == Request::Subscribe {
                    kept.lock().unwrap().push(stream);
                } else {
                    heard.lock().unwrap().push(line.trim().to_owned());
                    let message = "nothing is set to lock the session with".to_owned();
                    let answer = [Reply::Done, Reply::Error { message }];
                    let answer = &answer[usize::from(refuses.load(Ordering::Relaxed))];
                    stream.write_all(encode(answer).as_bytes()).unwrap();
                }
            }
        });
        Self {
            path,
            refusing,
            subscribers,
            asked,
            _directory: directory,
        }
    }

    /// Refuses to lock from now on.
    pub(crate) fn refuse(&self) {
        self.refusing.store(true, Ordering::Relaxed);
    }

    /// How many are following the state.
    pub(crate) fn subscribers(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }

    /// Tells each that follows the state whether the session is locked.
    pub(crate) fn locked(&self, locked: bool) {
        let state = State {
            locked,
            ..State::default()
        };
        let said = encode(&Reply::State(state));
        for mut subscriber in self.subscribers.lock().unwrap().iter() {
            subscriber.write_all(said.as_bytes()).unwrap();
        }
    }

    /// Ends, as far as those following the state can tell.
    pub(crate) fn end(&self) {
        self.subscribers.lock().unwrap().clear();
    }
}
