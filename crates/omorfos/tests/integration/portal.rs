//! The program that answers the portals, run against a compositor as it
//! is in a session: an application asks for a picture of the screen, the
//! user is asked, and the application is answered.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use omorfos::{Input, Roles};
use omorfos_config::{Part, States, Theme};
use smithay::reexports::wayland_server::ListeningSocket;
use xdg_desktop_portal_omorfos::{NAME, PATH, REQUEST, SCREENSHOT};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, RED};

const LEFT: u32 = 0x110;

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
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1", &listen])
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

    /// A program on the bus that is not the service, as the portal that
    /// passes applications' questions on is.
    fn peer(&self) -> Connection {
        Builder::address(self.address.as_str())
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

/// What a request for a picture was answered with.
type Answered = (u32, HashMap<String, OwnedValue>);

/// Starts the program in the session, its Wayland socket giving `roles`,
/// and waits until it answers on the bus. Its home folder is the
/// session's own.
fn start(session: &mut Session, daemon: &Daemon, roles: Roles) -> JoinHandle<String> {
    let home = session.config.path().to_owned();
    start_at_home(session, daemon, roles, &home)
}

/// The same, with a home folder of its own.
fn start_at_home(
    session: &mut Session,
    daemon: &Daemon,
    roles: Roles,
    home: &Path,
) -> JoinHandle<String> {
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-portal")).unwrap();
    session.inner().serve(socket, roles).unwrap();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-portal")),
        ("HOME", home.as_os_str().to_owned()),
        ("DBUS_SESSION_BUS_ADDRESS", OsString::from(&daemon.address)),
    ];
    let running = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        xdg_desktop_portal_omorfos::start(&variable).to_string()
    });
    let peer = daemon.peer();
    let bus = "org.freedesktop.DBus";
    session.eventually(|session| {
        session.step();
        let asked = peer.call_method(Some(bus), "/", Some(bus), "NameHasOwner", &(NAME,));
        asked.unwrap().body().deserialize::<bool>().unwrap() || running.is_finished()
    });
    running
}

/// Asks for a picture of the screen for an application, as the portal
/// does, by a request of this name, and waits for the answer on a thread
/// of its own.
fn ask(daemon: &Daemon, request: &'static str) -> JoinHandle<Answered> {
    let peer = daemon.peer();
    std::thread::spawn(move || {
        let request = format!("/org/freedesktop/portal/desktop/request/1_1/{request}");
        let path = ObjectPath::try_from(request.as_str()).unwrap();
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let body = (path, "org.example.App", "", options);
        let reply = peer.call_method(Some(NAME), PATH, Some(SCREENSHOT), "Screenshot", &body);
        reply.unwrap().body().deserialize().unwrap()
    })
}

/// Draws until the question is on the screen.
fn asked(session: &mut Session) {
    let looks = Theme::default()
        .style(Part::Launcher, States::NONE)
        .background;
    let shown = u32::from_be_bytes([looks.alpha, looks.red, looks.green, looks.blue]);
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == shown);
}

/// Draws until the application has been answered, and returns how.
fn answered(session: &mut Session, asking: JoinHandle<Answered>) -> Answered {
    session.eventually(|session| {
        session.draw();
        asking.is_finished()
    });
    asking.join().unwrap()
}

fn press(session: &mut Session, code: smithay::input::keyboard::Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
}

/// Clicks at a place on the question, which is in the middle of the
/// screen and 440 by 150.
fn click(session: &mut Session, (x, y): (i32, i32)) {
    let (left, top) = ((WIDTH - 440) / 2, (HEIGHT - 150) / 2);
    let at = Input::Motion {
        x: f64::from(left + x),
        y: f64::from(top + y),
    };
    session.compositor_mut().input(at, 0);
    session.step();
    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
}

/// The file an address is for.
fn file(results: &HashMap<String, OwnedValue>) -> PathBuf {
    let address = results["uri"].downcast_ref::<&str>().unwrap();
    let path = address.strip_prefix("file://").unwrap().replace("%20", " ");
    PathBuf::from(path)
}

/// The colour at a place in a saved picture, as `0xAARRGGBB`.
fn colour(path: &Path, (x, y): (usize, usize)) -> u32 {
    let reading = BufReader::new(std::fs::File::open(path).unwrap());
    let mut reader = png::Decoder::new(reading).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let at = (y * usize::try_from(info.width).unwrap() + x) * 3;
    u32::from_be_bytes([0xff, pixels[at], pixels[at + 1], pixels[at + 2]])
}

/// May ask the user over everything and take the picture, as the
/// compositor starts it.
const ITS_OWN: Roles = Roles::PANEL.with(Roles::OVERLAY).with(Roles::CAPTURE);

#[test]
fn an_application_is_given_a_picture_once_the_user_says_so() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let _running = start(&mut session, &daemon, ITS_OWN);

    let asking = ask(&daemon, "a");
    asked(&mut session);
    assert!(!asking.is_finished(), "it waits for the user");
    // A key that is neither yes nor no is passed over, and Enter twice is
    // one yes.
    press(&mut session, key::A);
    press(&mut session, key::RETURN);
    press(&mut session, key::RETURN);
    let (response, results) = answered(&mut session, asking);
    assert_eq!(response, 0);

    // The picture is among the user's pictures, and the question that
    // led to it is not in it.
    let path = file(&results);
    let folder = session.config.path().join("Pictures/Screenshots");
    assert_eq!(path.parent(), Some(folder.as_path()));
    assert_eq!(colour(&path, (10, 10)), RED);
    let middle = (
        usize::try_from(WIDTH / 2).unwrap(),
        usize::try_from(HEIGHT / 2).unwrap(),
    );
    assert_eq!(colour(&path, middle), BACKGROUND);
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == BACKGROUND);
}

#[test]
fn an_application_is_refused_when_the_user_says_no() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);
    let asking = ask(&daemon, "b");
    asked(&mut session);
    press(&mut session, key::ESCAPE);
    assert_eq!(answered(&mut session, asking), (1, HashMap::new()));
    assert!(
        !session.config.path().join("Pictures").exists(),
        "nothing was taken"
    );
}

#[test]
fn the_buttons_say_yes_and_no_and_the_rest_of_the_box_nothing() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);

    let asking = ask(&daemon, "c");
    asked(&mut session);
    click(&mut session, (220, 30));
    session.advance(Duration::from_millis(50));
    assert!(!asking.is_finished(), "a click on what is said is nothing");
    click(&mut session, (228, 114));
    assert_eq!(answered(&mut session, asking).0, 1, "cancelled");
    // Gone, before the next is asked, so that it is the next that is seen.
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == BACKGROUND);

    let asking = ask(&daemon, "d");
    asked(&mut session);
    click(&mut session, (360, 114));
    assert_eq!(answered(&mut session, asking).0, 0, "taken");
}

#[test]
fn one_question_is_asked_at_a_time_and_one_withdrawn_goes() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);
    let first = ask(&daemon, "e");
    asked(&mut session);
    let second = ask(&daemon, "f");
    assert_eq!(answered(&mut session, second).0, 2, "while another is up");

    // A withdrawal of a request that is not the one asked changes
    // nothing; of the one asked, it goes, and is answered as refused.
    let peer = daemon.peer();
    let other = "/org/freedesktop/portal/desktop/request/1_1/elsewhere";
    peer.call_method(Some(NAME), other, Some(REQUEST), "Close", &())
        .unwrap();
    session.advance(Duration::from_millis(50));
    assert!(!first.is_finished());
    let request = "/org/freedesktop/portal/desktop/request/1_1/e";
    peer.call_method(Some(NAME), request, Some(REQUEST), "Close", &())
        .unwrap();
    assert_eq!(answered(&mut session, first).0, 1);
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == BACKGROUND);
}

#[test]
fn without_leave_to_take_pictures_the_application_is_told_it_failed() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, Roles::PANEL.with(Roles::OVERLAY));
    let asking = ask(&daemon, "g");
    asked(&mut session);
    press(&mut session, key::RETURN);
    assert_eq!(answered(&mut session, asking), (2, HashMap::new()));
}

#[test]
fn not_let_put_a_question_on_the_screen_the_application_is_refused() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, Roles::CAPTURE);
    let asking = ask(&daemon, "h");
    assert_eq!(answered(&mut session, asking).0, 1);
}

#[test]
fn where_nothing_can_be_asked_the_application_is_told_at_once() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    // In a sandbox, it is not even shown how a question is put up.
    let mut flatpak = session.connect_plain();
    let path = session.config.path().join("sandbox");
    let _open = flatpak.sandbox(&path);
    session.roundtrip(&mut flatpak);
    let runtime = session.config.path().to_owned();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", path.into_os_string()),
        ("HOME", runtime.into_os_string()),
        ("DBUS_SESSION_BUS_ADDRESS", OsString::from(&daemon.address)),
    ];
    let _running = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        xdg_desktop_portal_omorfos::start(&variable).to_string()
    });
    let peer = daemon.peer();
    let bus = "org.freedesktop.DBus";
    session.eventually(|session| {
        session.step();
        let asked = peer.call_method(Some(bus), "/", Some(bus), "NameHasOwner", &(NAME,));
        asked.unwrap().body().deserialize::<bool>().unwrap()
    });
    let asking = ask(&daemon, "k");
    assert_eq!(answered(&mut session, asking).0, 2);
}

#[test]
fn it_ends_when_the_bus_does_and_needs_no_configuration_it_can_watch() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    // A home that is a file has no folder in it to watch.
    let home = session.config.path().join("file");
    std::fs::write(&home, "").unwrap();
    let running = start_at_home(&mut session, &daemon, ITS_OWN, &home);
    drop(daemon);
    session.eventually(|session| {
        session.step();
        running.is_finished()
    });
    let said = running.join().unwrap();
    assert!(said.starts_with("the bus has gone"), "{said}");
}

#[test]
fn a_question_whose_screen_goes_is_taken_for_a_no() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);
    let asking = ask(&daemon, "i");
    asked(&mut session);
    let only = session.output();
    session.compositor_mut().remove_output(&only);
    session.eventually(|session| {
        session.step();
        asking.is_finished()
    });
    assert_eq!(asking.join().unwrap().0, 1);
}

#[test]
fn the_question_takes_the_look_of_the_theme_and_ends_with_the_session() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let running = start(&mut session, &daemon, ITS_OWN);
    // How the desktop looks is told as well as asked about.
    let peer = daemon.peer();
    let settings = Some("org.freedesktop.impl.portal.Settings");
    let looks = ("org.freedesktop.appearance", "color-scheme");
    let reply = peer
        .call_method(Some(NAME), PATH, settings, "Read", &looks)
        .unwrap();
    let scheme: OwnedValue = reply.body().deserialize().unwrap();
    assert_eq!(scheme.downcast_ref::<u32>().unwrap(), 1, "dark");
    let asking = ask(&daemon, "j");
    asked(&mut session);
    // Its configuration is where a session's is, under its home.
    let config = session.config.path().join(".config/omorfos");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("theme.css"),
        "launcher { background: #ff0000; }",
    )
    .unwrap();
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == RED);
    drop(session);
    assert_eq!(running.join().unwrap(), "the session has ended");
    drop(asking);
}

#[test]
fn the_question_is_drawn_at_the_scale_of_its_screen() {
    let daemon = Daemon::start();
    let mut session = Session::with_config(&[("config.yaml", "output: { scale: 2 }\n")]);
    let _running = start(&mut session, &daemon, ITS_OWN);
    let asking = ask(&daemon, "l");
    // At twice the size, the middle of the screen is well inside it.
    asked(&mut session);
    press(&mut session, key::ESCAPE);
    assert_eq!(answered(&mut session, asking).0, 1);
}

#[test]
fn where_there_is_no_compositor_it_says_so() {
    let variable = |name: &str| match name {
        "HOME" | "XDG_RUNTIME_DIR" => Some(OsString::from("/nowhere")),
        "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
        _ => None,
    };
    let said = xdg_desktop_portal_omorfos::start(&variable).to_string();
    assert!(said.starts_with("/nowhere/wayland-none: "), "{said}");
}

#[test]
fn after_a_picture_is_taken_the_next_is_asked_about_too() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);
    for request in ["m", "n", "o"] {
        let asking = ask(&daemon, request);
        asked(&mut session);
        press(&mut session, key::RETURN);
        assert_eq!(answered(&mut session, asking).0, 0, "{request}");
        session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == BACKGROUND);
    }
}

#[test]
fn asked_first_as_the_portal_asks_the_question_still_goes_up() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN);
    // The portal reads the settings and the version before anything else.
    let peer = daemon.peer();
    let settings = Some("org.freedesktop.impl.portal.Settings");
    let groups: Vec<&str> = Vec::new();
    peer.call_method(Some(NAME), PATH, settings, "ReadAll", &(groups,))
        .unwrap();
    let properties = Some("org.freedesktop.DBus.Properties");
    peer.call_method(
        Some(NAME),
        PATH,
        properties,
        "Get",
        &(SCREENSHOT, "version"),
    )
    .unwrap();
    let asking = std::thread::spawn(move || {
        let path = ObjectPath::try_from("/org/freedesktop/portal/desktop/request/1_9/x").unwrap();
        let options: HashMap<&str, Value<'_>> = HashMap::from([
            ("modal", Value::from(true)),
            ("interactive", Value::from(false)),
            ("permission_store_checked", Value::from(true)),
        ]);
        let body = (path, "", "", options);
        let reply = peer.call_method(Some(NAME), PATH, Some(SCREENSHOT), "Screenshot", &body);
        reply.unwrap().body().deserialize::<Answered>().unwrap()
    });
    asked(&mut session);
    press(&mut session, key::RETURN);
    assert_eq!(answered(&mut session, asking).0, 0);
}
