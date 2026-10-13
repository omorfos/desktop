//! Notifications, as a client of a real compositor and a service on a real
//! bus.
//!
//! Which notifications are showing and how they are drawn is tested with
//! the notifications themselves, and so is what the service answers. Here
//! it is the program: that what a program sends gets onto the screen,
//! leaves it again, and that the program is told.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test support code fails the test on any error"
)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use omorfos::Input;
use omorfos_config::{Color, Part, State, States, Theme};
use omorfos_ipc::{Client as Ipc, Reply, State as Told, encode};
use omorfos_launcher::Icons;
use omorfos_notifications::{Bus, Notifier};
use smithay::reexports::wayland_server::ListeningSocket;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::Value;

use crate::common::{HEIGHT, Session, WIDTH, half_way};
use crate::windows::BACKGROUND;

const LEFT: u32 = 0x110;
const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

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
            .expect("dbus-daemon is needed to test notifications");
        let mut address = String::new();
        let stdout = child.stdout.take().unwrap();
        BufReader::new(stdout).read_line(&mut address).unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
            _directory: directory,
        }
    }

    /// A program on the bus that is not the service.
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

fn argb(color: Color) -> u32 {
    u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
}

/// The colour of a notification where nothing is written on it.
fn surface() -> u32 {
    argb(
        Theme::default()
            .style(Part::Notification, States::NONE)
            .background,
    )
}

/// The notifications in the session, hearing of them on `daemon`.
fn notifier(session: &mut Session, daemon: &Daemon) -> Notifier {
    let bus = Bus::open(Some(&daemon.address)).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::ALL)
        .unwrap();
    let config = session.config.path().to_owned();
    Notifier::new(theirs, bus, None, &config, Icons::default()).unwrap()
}

/// Sends a notification as a program would, and returns its number.
fn notify(peer: &Connection, title: &str, stays: i32, hints: HashMap<&str, Value<'_>>) -> u32 {
    let actions: Vec<&str> = Vec::new();
    let body = (
        "tester",
        0_u32,
        "",
        title,
        "Some words",
        actions,
        hints,
        stays,
    );
    let reply = peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body);
    reply.unwrap().body().deserialize().unwrap()
}

/// Lets the notifications and the compositor talk, at the time `now`,
/// until `shown` is true of the screen. What programs send arrives on a
/// thread of its own, so it is waited for.
fn until(
    session: &mut Session,
    notifier: &mut Notifier,
    now: Duration,
    shown: impl Fn(&mut Session) -> bool,
) {
    for _ in 0..5000 {
        notifier.turn(now).unwrap();
        session.step();
        if shown(session) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    unreachable!("the screen never came to show what was expected");
}

/// A point inside the first notification, towards its lower right-hand
/// corner, clear of what is written on it.
const INSIDE: (i32, i32) = (WIDTH - 20, 50);

#[test]
fn a_notification_gets_to_the_colour_of_being_pointed_at_over_time() {
    let theme = "notification { background: #ff0000; }\n\
                 notification:hover { background: #0000ff; transition: 100ms linear; }\n";
    let daemon = Daemon::start();
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    let at = Duration::from_millis;
    let (red, blue) = (0xffff_0000, 0xff00_00ff);
    // Up and to the left of the pointer, which is drawn from its tip down.
    let seen = |session: &mut Session| session.draw().at(INSIDE.0 - 6, INSIDE.1 - 6);
    notify(&peer, "Point at me", 0, HashMap::new());
    until(&mut session, &mut notifier, at(1000), |session| {
        seen(session) == red
    });
    let mut settle = |session: &mut Session, now: Duration| {
        for _ in 0..20 {
            notifier.turn(now).unwrap();
            session.step();
        }
        notifier.deadline()
    };
    assert_eq!(settle(&mut session, at(1000)), None, "nothing to wait for");

    let (x, y) = (f64::from(INSIDE.0), f64::from(INSIDE.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    assert_eq!(settle(&mut session, at(1000)), Some(at(1016)));
    assert_eq!(seen(&mut session), red, "setting out");

    settle(&mut session, at(1050));
    let between = seen(&mut session);
    assert!(half_way(between), "{between:#x}");

    assert_eq!(settle(&mut session, at(1100)), None, "at rest");
    assert_eq!(seen(&mut session), blue, "arrived");
}

fn showing(session: &mut Session) -> bool {
    session.draw().at(INSIDE.0, INSIDE.1) == surface()
}

fn clear(session: &mut Session) -> bool {
    session.draw().at(INSIDE.0, INSIDE.1) == BACKGROUND
}

#[test]
fn a_notification_is_shown_in_the_corner_of_the_screen_and_goes_when_its_time_is_up() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    until(&mut session, &mut notifier, Duration::ZERO, clear);

    assert_eq!(notify(&peer, "Hello", 2000, HashMap::new()), 1);
    until(&mut session, &mut notifier, Duration::ZERO, showing);
    // In the corner, with room around it, and nowhere else.
    let picture = session.draw();
    assert_eq!(picture.at(WIDTH - 4, 30), BACKGROUND, "the margin");
    assert_eq!(picture.at(WIDTH - 20, 4), BACKGROUND);
    assert_eq!(picture.at(100, 50), BACKGROUND);
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 20), BACKGROUND);
    assert_eq!(notifier.deadline(), Some(Duration::from_secs(2)));

    until(&mut session, &mut notifier, Duration::from_secs(1), showing);
    until(&mut session, &mut notifier, Duration::from_secs(2), clear);
    assert_eq!(notifier.deadline(), None);
}

#[test]
fn a_click_dismisses_a_notification_and_its_program_is_told() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(NAME)
        .unwrap()
        .build();
    let mut signals = zbus::blocking::MessageIterator::for_match_rule(rule, &peer, None).unwrap();
    let id = notify(&peer, "Click me", 0, HashMap::new());
    until(&mut session, &mut notifier, Duration::ZERO, showing);

    // Under the pointer it is marked as such.
    let (x, y) = (f64::from(INSIDE.0), f64::from(INSIDE.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    let hovered = States::NONE.with(State::Hovered);
    let marked = argb(
        Theme::default()
            .style(Part::Notification, hovered)
            .border
            .top
            .color,
    );
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(WIDTH - 100, 8) == marked
    });
    // Moving about on it changes nothing.
    session
        .compositor_mut()
        .input(Input::Motion { x: x - 3.0, y }, 0);
    until(&mut session, &mut notifier, Duration::ZERO, |_| true);
    assert_eq!(session.draw().at(WIDTH - 100, 8), marked);
    // And off it, no longer.
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 300.0 }, 0);
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(WIDTH - 100, 8) != marked
    });

    session.compositor_mut().input(Input::Motion { x, y }, 0);
    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    until(&mut session, &mut notifier, Duration::ZERO, clear);

    let signal = signals.next().unwrap().unwrap();
    assert_eq!(signal.body().deserialize::<(u32, u32)>().unwrap(), (id, 2));
}

#[test]
fn a_program_that_asks_is_told_of_a_click_unless_it_is_the_right_button() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(NAME)
        .unwrap()
        .build();
    let mut signals = zbus::blocking::MessageIterator::for_match_rule(rule, &peer, None).unwrap();
    let mut said = || {
        let signal = signals.next().unwrap().unwrap();
        let name = signal.header().member().unwrap().to_string();
        let body = signal.body();
        // The two signals say different things after the number.
        let id = if name == "ActionInvoked" {
            body.deserialize::<(u32, String)>().unwrap().0
        } else {
            body.deserialize::<(u32, u32)>().unwrap().0
        };
        (name, id)
    };
    let send = || {
        let hints: HashMap<&str, Value<'_>> = HashMap::new();
        let body = (
            "tester",
            0_u32,
            "",
            "Open me",
            "Some words",
            vec!["default", "Open"],
            hints,
            0,
        );
        let reply = peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body);
        reply.unwrap().body().deserialize::<u32>().unwrap()
    };
    let (x, y) = (f64::from(INSIDE.0), f64::from(INSIDE.1));
    let click = |session: &mut Session, button: u32| {
        session.compositor_mut().input(Input::Motion { x, y }, 0);
        for pressed in [true, false] {
            session
                .compositor_mut()
                .input(Input::Button { button, pressed }, 0);
        }
    };

    // The left button: its sender hears of the click, and then of its going.
    let first = send();
    until(&mut session, &mut notifier, Duration::ZERO, showing);
    click(&mut session, LEFT);
    until(&mut session, &mut notifier, Duration::ZERO, clear);
    assert_eq!(said(), ("ActionInvoked".to_owned(), first));
    assert_eq!(said(), ("NotificationClosed".to_owned(), first));

    // The right button: only of its going.
    let second = send();
    until(&mut session, &mut notifier, Duration::ZERO, showing);
    click(&mut session, 0x111);
    until(&mut session, &mut notifier, Duration::ZERO, clear);
    assert_eq!(said(), ("NotificationClosed".to_owned(), second));
}

#[test]
fn a_program_can_take_its_notification_back() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    let id = notify(&peer, "Never mind", 0, HashMap::new());
    until(&mut session, &mut notifier, Duration::ZERO, showing);

    let closed = peer.call_method(Some(NAME), PATH, Some(NAME), "CloseNotification", &id);
    closed.unwrap();
    until(&mut session, &mut notifier, Duration::ZERO, clear);
    // One that is not showing is nothing to take back.
    let closed = peer.call_method(Some(NAME), PATH, Some(NAME), "CloseNotification", &99_u32);
    closed.unwrap();
    until(&mut session, &mut notifier, Duration::ZERO, clear);
}

#[test]
fn notifications_stack_and_the_oldest_makes_room_when_there_are_too_many() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    notify(&peer, "First", 0, HashMap::new());
    until(&mut session, &mut notifier, Duration::ZERO, showing);
    assert_eq!(session.draw().at(INSIDE.0, 150), BACKGROUND);

    let urgent = HashMap::from([("urgency", Value::U8(2))]);
    notify(&peer, "Second", 0, urgent);
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(INSIDE.0, 130) == surface()
    });

    for _ in 0..8 {
        notify(&peer, "More", 0, HashMap::new());
    }
    // Eight are kept, which is as many as fit this screen and more.
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(INSIDE.0, HEIGHT - 20) == surface()
    });
}

#[test]
fn notifications_follow_the_theme_and_the_scale_of_their_screen() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    notify(&peer, "Themed", 0, HashMap::new());
    until(&mut session, &mut notifier, Duration::ZERO, showing);

    session.write_config("theme.css", "notification { background: #0000ff; }\n");
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(INSIDE.0, INSIDE.1) == 0xff00_00ff
    });
    session.write_config("config.yaml", "notifications: { timeout: 60 }\n");
    until(&mut session, &mut notifier, Duration::ZERO, |_| true);

    session.suggest_scale(omorfos::Scale::new(2.0));
    // Told of the new scale, they are drawn again for it. Twice as large,
    // they reach twice as far down the screen.
    for _ in 0..40 {
        notifier.turn(Duration::ZERO).unwrap();
        session.step();
    }
    assert_eq!(session.draw().at(INSIDE.0, 110), 0xff00_00ff);
}

#[test]
fn notifications_move_to_another_screen_when_theirs_goes() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let other = session.add_output((800, 600));
    let (x, y) = (f64::from(WIDTH + 400), 300.0);
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    let mut notifier = notifier(&mut session, &daemon);
    let peer = daemon.peer();
    notify(&peer, "Over here", 0, HashMap::new());
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw_on(1).at(800 - 20, 50) == surface()
    });

    session.compositor_mut().remove_output(&other);

    until(&mut session, &mut notifier, Duration::ZERO, showing);
}

#[test]
fn the_notifications_give_up_when_the_bus_or_the_compositor_goes() {
    let mut daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = notifier(&mut session, &daemon);
    until(&mut session, &mut notifier, Duration::ZERO, clear);
    daemon.child.kill().unwrap();
    daemon.child.wait().unwrap();
    let gone = (0..5000).any(|_| {
        std::thread::sleep(Duration::from_millis(1));
        notifier.turn(Duration::ZERO).is_err()
    });
    assert!(gone, "without the bus");

    let daemon = Daemon::start();
    let mut session = Session::new();
    let mut notifier = self::notifier(&mut session, &daemon);
    until(&mut session, &mut notifier, Duration::ZERO, clear);
    drop(session);
    let gone = (0..5000).any(|_| {
        std::thread::sleep(Duration::from_millis(1));
        notifier.turn(Duration::ZERO).is_err()
    });
    assert!(gone, "without the compositor");
}

#[test]
fn the_notifications_run_by_themselves_in_the_session_they_are_started_in() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session.inner().serve(socket, omorfos::Roles::ALL).unwrap();
    // The compositor's own socket is where a session has it.
    let own = runtime.join("omorfos-wayland-test.sock");
    session.compositor_mut().listen_ipc(own).unwrap();
    std::fs::create_dir(runtime.join("omorfos")).unwrap();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-test")),
        ("XDG_CONFIG_HOME", runtime.clone().into_os_string()),
        ("XDG_DATA_HOME", runtime.join("data").into_os_string()),
        ("DBUS_SESSION_BUS_ADDRESS", OsString::from(&daemon.address)),
    ];
    let running = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_notifications::start(&variable).to_string()
    });

    // Sent once the service has the name, it is shown without anything
    // prodding the program, and taken away when its sender says so.
    let peer = daemon.peer();
    let sent = (0..5000).find_map(|_| {
        std::thread::sleep(Duration::from_millis(1));
        session.step();
        let actions: Vec<&str> = Vec::new();
        let hints: HashMap<&str, Value<'_>> = HashMap::new();
        let body = (
            "tester",
            0_u32,
            "",
            "Standing",
            "Some words",
            actions,
            hints,
            0,
        );
        let reply = peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body);
        reply
            .ok()
            .map(|reply| reply.body().deserialize::<u32>().unwrap())
    });
    session.eventually(showing);
    let closed = peer.call_method(
        Some(NAME),
        PATH,
        Some(NAME),
        "CloseNotification",
        &sent.unwrap(),
    );
    closed.unwrap();
    session.eventually(clear);
    // And one with a time goes when the time is up, by itself.
    let actions: Vec<&str> = Vec::new();
    let hints: HashMap<&str, Value<'_>> = HashMap::new();
    let body = (
        "tester",
        0_u32,
        "",
        "Brief",
        "Some words",
        actions,
        hints,
        300,
    );
    let brief = peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body);
    brief.unwrap();
    session.eventually(showing);
    session.eventually(clear);

    // It ends with the session, saying why.
    drop(session);
    assert_ne!(running.join().unwrap(), "");
}

/// The colour of the line around something that wants attention now.
fn alarming() -> u32 {
    let urgent = States::NONE.with(State::Urgent);
    argb(
        Theme::default()
            .style(Part::Notification, urgent)
            .border
            .left
            .color,
    )
}

/// A point on the left-hand edge of the topmost notification, and one in
/// the compositor's own notice where no notification reaches.
const EDGE: (i32, i32) = (WIDTH - 8 - 340, 30);
const NOTICE: (i32, i32) = (150, 30);

/// Something wrong with the session that is not its theme.
const BROKEN: (&str, &str) = ("keys.yaml", "nonsense: [");

/// Notifications in the session that are told what is wrong with it.
fn informed(session: &mut Session, daemon: &Daemon) -> Notifier {
    let socket = session.listen();
    let bus = Bus::open(Some(&daemon.address)).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::ALL)
        .unwrap();
    let config = session.config.path().to_owned();
    let news = Ipc::connect(&socket).unwrap();
    Notifier::new(theirs, bus, Some(news), &config, Icons::default()).unwrap()
}

#[test]
fn what_is_wrong_with_the_session_is_shown_among_the_notifications() {
    let daemon = Daemon::start();
    let mut session = Session::with_config(&[BROKEN]);
    session.step();
    let seen = |session: &mut Session| {
        let picture = session.draw();
        (picture.at(EDGE.0, EDGE.1), picture.at(NOTICE.0, NOTICE.1))
    };
    assert_ne!(seen(&mut session).1, BACKGROUND, "the compositor says so");

    // With the notifications there, they say it, and the compositor does
    // not as well.
    let mut notifier = informed(&mut session, &daemon);
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        seen(session) == (alarming(), BACKGROUND)
    });

    // It stays until it is clicked away.
    let (x, y) = (f64::from(EDGE.0 + 40), f64::from(EDGE.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    for pressed in [true, false] {
        let button = Input::Button {
            button: 0x110,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        seen(session) == (BACKGROUND, BACKGROUND)
    });

    // Put right and broken again, it is said again.
    session.write_config(BROKEN.0, "");
    session.compositor_mut().reload_config();
    until(&mut session, &mut notifier, Duration::ZERO, |_| true);
    session.write_config(BROKEN.0, BROKEN.1);
    session.compositor_mut().reload_config();
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        seen(session) == (alarming(), BACKGROUND)
    });

    // And when the notifications are gone, the compositor says it again.
    drop(notifier);
    session.eventually(|session| seen(session).1 != BACKGROUND);
}

#[test]
fn the_notifications_take_only_the_state_of_the_session_from_what_they_are_told() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let bus = Bus::open(Some(&daemon.address)).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::ALL)
        .unwrap();
    let config = session.config.path().to_owned();
    let (news, mut compositor) = UnixStream::pair().unwrap();
    let news = Some(Ipc::from_stream(news));
    let mut notifier = Notifier::new(theirs, bus, news, &config, Icons::default()).unwrap();

    // It asks to be told what is wrong, as one that will show it.
    let mut asked = String::new();
    BufReader::new(&compositor).read_line(&mut asked).unwrap();
    assert_eq!(asked, "\"show_problems\"\n");

    // An answer to nothing it asked is passed over.
    let wrong = Told {
        problems: vec!["something is wrong".to_owned()],
        ..Told::default()
    };
    let said = [encode(&Reply::Done), encode(&Reply::State(wrong))].concat();
    std::io::Write::write_all(&mut compositor, said.as_bytes()).unwrap();
    until(&mut session, &mut notifier, Duration::ZERO, |session| {
        session.draw().at(EDGE.0, EDGE.1) == alarming()
    });

    // Once the compositor has gone, so are the notifications.
    drop(compositor);
    let failed = (0..50).any(|_| notifier.turn(Duration::ZERO).is_err());
    assert!(failed);
}

#[test]
fn notifications_that_cannot_ask_what_is_wrong_leave_it_to_the_compositor() {
    let daemon = Daemon::start();
    let mut session = Session::with_config(&[BROKEN]);
    session.step();
    let bus = Bus::open(Some(&daemon.address)).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::ALL)
        .unwrap();
    let config = session.config.path().to_owned();
    // The other end of what it was to ask on has gone.
    let (news, gone) = UnixStream::pair().unwrap();
    drop(gone);
    let news = Some(Ipc::from_stream(news));
    let mut notifier = Notifier::new(theirs, bus, news, &config, Icons::default()).unwrap();

    until(&mut session, &mut notifier, Duration::ZERO, |_| true);
    assert_ne!(session.draw().at(NOTICE.0, NOTICE.1), BACKGROUND);
}

/// The colour of the line around a notification that is nothing special.
fn plain() -> u32 {
    let style = Theme::default().style(Part::Notification, States::NONE);
    argb(style.border.left.color)
}

#[test]
fn what_has_happened_in_the_session_is_shown_for_a_while_among_the_notifications() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let bus = Bus::open(Some(&daemon.address)).unwrap();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::ALL)
        .unwrap();
    let config = session.config.path().to_owned();
    let (news, mut compositor) = UnixStream::pair().unwrap();
    let news = Some(Ipc::from_stream(news));
    let mut notifier = Notifier::new(theirs, bus, news, &config, Icons::default()).unwrap();

    let happened = Told {
        events: vec!["something was started again".to_owned()],
        ..Told::default()
    };
    let said = encode(&Reply::State(happened));
    std::io::Write::write_all(&mut compositor, said.as_bytes()).unwrap();

    // It looks as any notification does, since nothing is wrong.
    let edge = |session: &mut Session| session.draw().at(EDGE.0, EDGE.1);
    let at = Duration::from_secs;
    until(&mut session, &mut notifier, at(100), |session| {
        edge(session) == plain()
    });
    // And goes by itself after as long as notifications stay.
    until(&mut session, &mut notifier, at(104), |_| true);
    assert_eq!(edge(&mut session), plain());
    until(&mut session, &mut notifier, at(105), |session| {
        edge(session) == BACKGROUND
    });
}
