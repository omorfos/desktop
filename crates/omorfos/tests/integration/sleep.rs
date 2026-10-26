//! Locking before the machine sleeps, as `omorfos-session` does it in a
//! session: logind, standing in for the real one, says the machine is
//! about to sleep, and is let go only once the compositor has locked the
//! session.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use omorfos_session::Places;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedFd, OwnedObjectPath};

use crate::common::Session;

/// A bus of the test's own, gone when the test is.
struct Daemon {
    child: Child,
    address: String,
    _directory: tempfile::TempDir,
}

impl Daemon {
    fn start() -> Self {
        let directory = tempfile::TempDir::new().unwrap();
        let listen = format!("--address=unix:path={}/bus", directory.path().display());
        // Killed by the kernel once the thread that started it ends, so
        // that a test killed before it could stop it leaves none running.
        let mut child = Command::new("setpriv")
            .args(["--pdeathsig", "KILL", "--", "dbus-daemon", "--session"])
            .args(["--nofork", "--print-address=1", &listen])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        let stdout = child.stdout.take().unwrap();
        BufReader::new(stdout).read_line(&mut address).unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
            _directory: directory,
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// logind's manager, which hands out holds on sleep and keeps the other
/// end of each.
struct Manager {
    held: Arc<Mutex<Vec<UnixStream>>>,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    #[zbus(name = "Inhibit")]
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> OwnedFd {
        assert_eq!((what, who, mode), ("sleep", "omorfos", "delay"));
        assert_ne!(why, "");
        let (ours, theirs) = UnixStream::pair().unwrap();
        self.held.lock().unwrap().push(ours);
        OwnedFd::from(std::os::fd::OwnedFd::from(theirs))
    }

    #[zbus(name = "GetSession")]
    #[expect(clippy::unused_self, reason = "as the interface macro needs it")]
    fn get_session(&self, which: &str) -> OwnedObjectPath {
        assert_eq!(which, "auto");
        OwnedObjectPath::try_from("/org/freedesktop/login1/session/_1").unwrap()
    }

    /// Long enough that the test is never let go for want of patience.
    #[zbus(property, name = "InhibitDelayMaxUSec")]
    #[expect(clippy::unused_self, reason = "as the interface macro needs it")]
    const fn inhibit_delay_max(&self) -> u64 {
        60_000_000
    }
}

/// logind, standing in for the real one.
struct Logind {
    connection: Connection,
    held: Arc<Mutex<Vec<UnixStream>>>,
}

impl Logind {
    fn start(daemon: &Daemon) -> Self {
        let held = Arc::default();
        let manager = Manager {
            held: Arc::clone(&held),
        };
        let connection = Builder::address(daemon.address.as_str())
            .unwrap()
            .name("org.freedesktop.login1")
            .unwrap()
            .serve_at("/org/freedesktop/login1", manager)
            .unwrap()
            .build()
            .unwrap();
        Self { connection, held }
    }

    /// Says that the machine is about to sleep, or has woken.
    fn sleeping(&self, sleeping: bool) {
        let nobody: Option<&str> = None;
        let (path, manager) = ("/org/freedesktop/login1", "org.freedesktop.login1.Manager");
        let said = (sleeping,);
        let connection = &self.connection;
        let emitted = connection.emit_signal(nobody, path, manager, "PrepareForSleep", &said);
        emitted.unwrap();
    }

    /// Each hold on sleep it has given, and whether it is still held.
    fn holds(&self) -> Vec<bool> {
        let held = self.held.lock().unwrap();
        let open = |ours: &UnixStream| {
            ours.set_nonblocking(true).unwrap();
            !matches!((&*ours).read(&mut [0]), Ok(0))
        };
        held.iter().map(open).collect()
    }
}

/// Watches over the session as `omorfos-session` does, on a thread of its
/// own, with logind on `daemon`, until it holds sleep back.
fn watching(session: &mut Session, daemon: &Daemon, logind: &Logind) -> JoinHandle<String> {
    let places = Places {
        backlights: PathBuf::from("/nowhere"),
        path: std::ffi::OsString::new(),
        session: None,
        system: daemon.address.clone(),
        compositor: Some(session.listen()),
        autostart: Vec::new(),
        desktops: Vec::new(),
        scoper: None,
        journal: PathBuf::from("/nowhere/journal"),
        rfkill: PathBuf::from("/nowhere/rfkill"),
    };
    let running = std::thread::spawn(move || match omorfos_session::watch(&places) {
        Ok(()) => "ended".to_owned(),
        Err(failure) => failure.to_string(),
    });
    session.eventually(|_| logind.holds() == [true]);
    running
}

#[test]
fn the_machine_sleeps_only_once_the_session_is_locked() {
    let directory = tempfile::TempDir::new().unwrap();
    let started = directory.path().join("locker-started");
    let config = format!("locker: [touch, '{}']\n", started.display());
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let daemon = Daemon::start();
    let logind = Logind::start(&daemon);
    let running = watching(&mut session, &daemon, &logind);

    logind.sleeping(true);
    // The locker it names is started, and until it has locked the session
    // the machine is held awake.
    session.eventually(|_| started.exists());
    for _ in 0..20 {
        session.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(logind.holds(), [true]);

    // Locked, and the locker told so once a frame of nothing else shows.
    let _locked = crate::lock::lock(&mut session);
    session.eventually(|_| logind.holds() == [false]);

    // Woken, the next sleep is held back again.
    logind.sleeping(false);
    session.eventually(|_| logind.holds() == [false, true]);

    // It ends with the compositor.
    drop(session);
    assert_eq!(running.join().unwrap(), "ended");
}

#[test]
fn with_nothing_set_to_lock_with_the_machine_sleeps_at_once() {
    let mut session = Session::new();
    let daemon = Daemon::start();
    let logind = Logind::start(&daemon);
    let _running = watching(&mut session, &daemon, &logind);
    logind.sleeping(true);
    // Far sooner than logind would wait.
    session.eventually(|_| logind.holds() == [false]);
}
