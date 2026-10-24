//! polkit's agent, `omorfos-ask`, run against a compositor as it is in a
//! session: polkit, standing in for the real one on a bus of the test's
//! own, asks it to ask; the user types in the sealed prompt; and polkit's
//! helper, standing in too, says whether that was right.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use omorfos::{Input, Roles};
use omorfos_ask::{AGENT, CANCEL, EYE, FIELD, GO, PATH, Places, SIZE, WRONG};
use omorfos_config::{Part, States, Theme};
use smithay::input::keyboard::Keycode;
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::{Header, Message};
use zbus::zvariant::{OwnedValue, Value};

use crate::common::{HEIGHT, Session, WIDTH, key};

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
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Who registered as the agent, and for which session.
type Registered = Arc<Mutex<Vec<(String, String, String)>>>;

/// polkit's authority, which takes the agent of a session.
struct Authority {
    registered: Registered,
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl Authority {
    #[zbus(name = "RegisterAuthenticationAgent")]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the interface macro hands each argument over as it is sent"
    )]
    fn register(
        &self,
        #[zbus(header)] header: Header<'_>,
        subject: (String, HashMap<String, OwnedValue>),
        locale: String,
        // A string, as polkit has it, not an object's path.
        path: String,
    ) {
        assert_eq!(subject.0, "unix-session");
        assert_eq!(path, PATH);
        let session = String::try_from(subject.1["session-id"].clone()).unwrap();
        let sender = header.sender().unwrap().to_string();
        let mut registered = self.registered.lock().unwrap();
        registered.push((sender, session, locale));
    }
}

/// polkit, standing in for the real one.
struct Polkit {
    connection: Connection,
    registered: Registered,
}

impl Polkit {
    fn start(daemon: &Daemon) -> Self {
        let registered = Registered::default();
        let authority = Authority {
            registered: Arc::clone(&registered),
        };
        let connection = Builder::address(daemon.address.as_str())
            .unwrap()
            .name("org.freedesktop.PolicyKit1")
            .unwrap()
            .serve_at("/org/freedesktop/PolicyKit1/Authority", authority)
            .unwrap()
            .build()
            .unwrap();
        Self {
            connection,
            registered,
        }
    }

    /// The agent that registered, once one has.
    fn agent(&self) -> Option<String> {
        let registered = self.registered.lock().unwrap();
        registered.first().map(|(agent, _, _)| agent.clone())
    }
}

/// Asks the agent to ask the user, as polkit does, for the request whose
/// cookie is the file the helper writes down what it was asked in; for
/// `uids` and `gids`, and someone of a kind it does not know. It is sent
/// before this returns, and its answer waited for on a thread of its own.
fn begin(
    connection: &Connection,
    agent: &str,
    cookie: &Path,
    (uids, gids): (&[u32], &[u32]),
) -> JoinHandle<Result<(), String>> {
    let identity = |kind: &str, key: &str, number: u32| {
        let details = HashMap::from([(key.to_owned(), Value::from(number))]);
        (kind.to_owned(), details)
    };
    let mut identities: Vec<_> = uids
        .iter()
        .map(|uid| identity("unix-user", "uid", *uid))
        .collect();
    identities.extend(gids.iter().map(|gid| identity("unix-group", "gid", *gid)));
    identities.push(identity("unix-netgroup", "id", 1));
    let details: HashMap<&str, &str> = HashMap::new();
    let asked = (
        "org.example.install",
        "Authentication is needed to install software",
        "",
        details,
        cookie.display().to_string(),
        identities,
    );
    let message = Message::method_call(PATH, "BeginAuthentication")
        .unwrap()
        .destination(agent)
        .unwrap()
        .interface(AGENT)
        .unwrap()
        .build(&asked)
        .unwrap();
    let serial = message.primary_header().serial_num();
    // Listening before it is sent, so that the answer is not missed.
    let replies = MessageIterator::from(connection);
    connection.send(&message).unwrap();
    std::thread::spawn(move || {
        for reply in replies.flatten() {
            let header = reply.header();
            if header.reply_serial() == Some(serial) {
                let refused = header.error_name().map(ToString::to_string);
                return refused.map_or(Ok(()), Err);
            }
        }
        Err("the bus went away".to_owned())
    })
}

/// The user at the keyboard, and another, as the system lists them.
fn people(directory: &Path) -> (PathBuf, PathBuf) {
    let me = rustix::process::getuid().as_raw();
    let users = directory.join("passwd");
    let groups = directory.join("group");
    let listed = format!("root:x:0:0::/root:/bin/sh\nme:x:{me}:{me}::/home/me:/bin/sh\n");
    std::fs::write(&users, listed).unwrap();
    std::fs::write(&groups, "root:x:0:\nsudo:x:27:me\n").unwrap();
    (users, groups)
}

/// The agent, started for the session, and polkit, once it has registered.
struct Asker {
    polkit: Polkit,
    agent: String,
    running: JoinHandle<String>,
    directory: tempfile::TempDir,
    daemon: Daemon,
}

fn start(session: &mut Session) -> Asker {
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/polkit-helper");
    start_with(session, helper)
}

/// The same, with the helper at `helper`.
fn start_with(session: &mut Session, helper: PathBuf) -> Asker {
    let daemon = Daemon::start();
    let polkit = Polkit::start(&daemon);
    let directory = tempfile::TempDir::new().unwrap();
    let (users, groups) = people(directory.path());
    let places = Places {
        system: daemon.address.clone(),
        session: Some("7".to_owned()),
        locale: "en_GB.UTF-8".to_owned(),
        helper,
        users,
        groups,
        me: rustix::process::getuid().as_raw(),
        config: Some(session.config.path().to_owned()),
        wayland: None,
    };
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, Roles::ASK)
        .unwrap();
    let running =
        std::thread::spawn(move || crate::keeper::said(omorfos_ask::run(&places, theirs)));
    session.eventually(|_| polkit.agent().is_some() || running.is_finished());
    let agent = polkit.agent().unwrap();
    Asker {
        polkit,
        agent,
        running,
        directory,
        daemon,
    }
}

/// The colour of the prompt's box, as the theme has it.
fn question() -> u32 {
    let colour = Theme::default()
        .style(Part::Question, States::NONE)
        .background;
    u32::from_be_bytes([colour.alpha, colour.red, colour.green, colour.blue])
}

/// Where on the screen a place on the prompt is: it is in the middle.
fn on_prompt((x, y, width, height): (i32, i32, u32, u32)) -> (f64, f64) {
    let left = (WIDTH - SIZE.0.cast_signed()) / 2;
    let top = (HEIGHT - SIZE.1.cast_signed()) / 2;
    let middle = |at: i32, across: u32| f64::from(at + across.cast_signed() / 2);
    (middle(left + x, width), middle(top + y, height))
}

/// Whether the prompt is up, by the colour near its top left corner.
fn up(session: &mut Session) -> bool {
    let (left, top) = (
        (WIDTH - SIZE.0.cast_signed()) / 2,
        (HEIGHT - SIZE.1.cast_signed()) / 2,
    );
    session.draw().at(left + 12, top + 12) == question()
}

/// Waits for the prompt to be up and to have been given the keyboard.
fn wait_up(session: &mut Session) {
    session.eventually(up);
    for _ in 0..10 {
        session.step();
    }
}

fn tap(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
}

fn click(session: &mut Session, at: (f64, f64)) {
    session
        .compositor_mut()
        .input(Input::Motion { x: at.0, y: at.1 }, 0);
    for pressed in [true, false] {
        session.compositor_mut().input(
            Input::Button {
                button: 0x110,
                pressed,
            },
            0,
        );
    }
}

/// What the helper wrote down.
fn helped(cookie: &Path) -> String {
    std::fs::read_to_string(cookie).unwrap_or_default()
}

#[test]
fn the_password_typed_in_the_sealed_prompt_is_checked_by_polkits_helper() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let registered = asker.polkit.registered.lock().unwrap().clone();
    assert_eq!(registered[0].1, "7", "for this session");
    assert_eq!(registered[0].2, "en_GB.UTF-8");
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[0, me], &[]),
    );
    wait_up(&mut session);
    assert!(
        helped(&cookie).starts_with("user me\n"),
        "the user's own password"
    );

    // Wrong first, and asked again, afresh.
    tap(&mut session, key::A);
    tap(&mut session, key::RETURN);
    session.eventually(|_| helped(&cookie).matches("user me").count() == 2);
    assert!(helped(&cookie).contains("typed a\n"));
    assert!(!answer.is_finished());
    wait_up(&mut session);
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    // Shown and hidden again by the button, which types nothing.
    click(&mut session, on_prompt(EYE));
    click(&mut session, on_prompt(EYE));
    click(&mut session, on_prompt(GO));
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()));
    assert!(helped(&cookie).ends_with("typed atx\n"));
    session.eventually(|session| !up(session));
    assert!(!asker.running.is_finished());
    let _ = WRONG;
}

#[test]
fn giving_up_says_so_to_polkit() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    tap(&mut session, key::ESCAPE);
    session.eventually(|_| answer.is_finished());
    let refused = answer.join().unwrap().unwrap_err();
    assert!(refused.contains("Cancelled"), "{refused}");
    // Gone, as it fades out, before the next is asked.
    session.eventually(|session| !up(session));

    // And by the button, for a member of a group that may.
    let cookie = asker.directory.path().join("again");
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[], &[27]),
    );
    wait_up(&mut session);
    click(&mut session, on_prompt(CANCEL));
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());
    session.eventually(|session| !up(session));
}

#[test]
fn polkit_may_give_up_asking_and_ask_again() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let me = rustix::process::getuid().as_raw();
    let first = asker.directory.path().join("first");
    let second = asker.directory.path().join("second");
    let answer = begin(&asker.polkit.connection, &asker.agent, &first, (&[me], &[]));
    wait_up(&mut session);
    // Asked again meanwhile, the second waits for the first.
    let next = begin(
        &asker.polkit.connection,
        &asker.agent,
        &second,
        (&[me], &[]),
    );
    let cancel = |cookie: &Path| {
        let cookie = cookie.display().to_string();
        let agent = asker.agent.as_str();
        let reply = asker.polkit.connection.call_method(
            Some(agent),
            PATH,
            Some(AGENT),
            "CancelAuthentication",
            &(cookie,),
        );
        reply.unwrap();
    };
    // Given up on while it waits, it is never asked.
    cancel(&second);
    session.eventually(|_| next.is_finished());
    assert!(next.join().unwrap().is_err());
    // And the one being asked, which a third then takes the place of.
    let third = asker.directory.path().join("third");
    let last = begin(&asker.polkit.connection, &asker.agent, &third, (&[me], &[]));
    cancel(&first);
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());
    session.eventually(|_| !helped(&third).is_empty());
    cancel(&third);
    session.eventually(|_| last.is_finished());
    assert!(last.join().unwrap().is_err());
    assert_eq!(helped(&second), "", "nothing was checked of the second");
}

#[test]
fn a_request_nobody_here_may_say_yes_to_is_given_up_at_once() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[4242], &[]),
    );
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());
    assert_eq!(helped(&cookie), "", "nothing was checked");
}

#[test]
fn only_polkit_is_heard() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let other = Builder::address(asker.daemon.address.as_str())
        .unwrap()
        .build()
        .unwrap();
    let cookie = asker.directory.path().join("asked");
    let answer = begin(&other, &asker.agent, &cookie, (&[0], &[]));
    session.eventually(|_| answer.is_finished());
    let refused = answer.join().unwrap().unwrap_err();
    assert!(refused.contains("AccessDenied"), "{refused}");
    // Nor is anything but what an agent is asked.
    let reply = asker.polkit.connection.call_method(
        Some(asker.agent.as_str()),
        PATH,
        Some(AGENT),
        "Explode",
        &(),
    );
    assert!(reply.unwrap_err().to_string().contains("UnknownMethod"));
    let reply = asker.polkit.connection.call_method(
        Some(asker.agent.as_str()),
        "/elsewhere",
        Some(AGENT),
        "BeginAuthentication",
        &(),
    );
    assert!(reply.unwrap_err().to_string().contains("UnknownMethod"));
    let reply = asker.polkit.connection.call_method(
        Some(asker.agent.as_str()),
        PATH,
        Some(AGENT),
        "BeginAuthentication",
        &("too few",),
    );
    assert!(reply.unwrap_err().to_string().contains("InvalidArgs"));
}

#[test]
fn the_agent_ends_once_polkit_has_gone() {
    let mut session = Session::new();
    let asker = start(&mut session);
    drop(asker.polkit);
    session.eventually(|_| asker.running.is_finished());
    assert_eq!(
        asker.running.join().unwrap(),
        "polkit can no longer be heard"
    );
}

/// The colour of the border of the field while it is typed into, or not.
fn field_border(typing: bool) -> u32 {
    let states = if typing {
        States::NONE.with(omorfos_config::State::Focused)
    } else {
        States::NONE
    };
    let colour = Theme::default()
        .style(Part::QuestionInput, states)
        .border
        .top
        .color;
    u32::from_be_bytes([colour.alpha, colour.red, colour.green, colour.blue])
}

/// Whether the field is being typed into, by the colour of its border.
fn typing(session: &mut Session) -> bool {
    let left = (WIDTH - SIZE.0.cast_signed()) / 2 + FIELD.0;
    let top = (HEIGHT - SIZE.1.cast_signed()) / 2 + FIELD.1;
    session.draw().at(left + 20, top) == field_border(true)
}

#[test]
fn what_is_typed_can_be_taken_back_and_what_types_nothing_is_passed_over() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked-open");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    tap(&mut session, key::X);
    tap(&mut session, key::BACKSPACE);
    // Tab types nothing that could be shown, nor does a chord.
    tap(&mut session, key::TAB);
    session.compositor_mut().input(
        Input::Key {
            code: key::CONTROL,
            pressed: true,
        },
        0,
    );
    tap(&mut session, key::A);
    session.compositor_mut().input(
        Input::Key {
            code: key::CONTROL,
            pressed: false,
        },
        0,
    );
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    // A click on the field is no button.
    click(&mut session, on_prompt(FIELD));
    // Nor is the pointer anywhere once it has left the box.
    session
        .compositor_mut()
        .input(Input::Motion { x: 2.0, y: 2.0 }, 0);
    // Gone on, a click on Cancel after is too late to count.
    tap(&mut session, key::RETURN);
    click(&mut session, on_prompt(CANCEL));
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()));
    assert!(
        helped(&cookie).ends_with("typed atx\n"),
        "{}",
        helped(&cookie)
    );
}

#[test]
fn keys_pressed_while_it_is_not_yet_asked_are_not_taken() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked-slow");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    assert!(!typing(&mut session), "up, and not yet asking");
    tap(&mut session, key::X);
    // The prompt is given time to have heard the key, and the helper only
    // then asks: a helper that asked sooner, as one busy machine had it,
    // would have the key typed into the field.
    let heard = std::time::Instant::now() + std::time::Duration::from_millis(300);
    while std::time::Instant::now() < heard {
        session.step();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut go = cookie.clone().into_os_string();
    go.push(".go");
    std::fs::write(go, "").unwrap();
    session.eventually(typing);
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    tap(&mut session, key::RETURN);
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()), "{}", helped(&cookie));
}

#[test]
fn a_helper_that_cannot_be_started_or_started_again_gives_up() {
    let mut session = Session::new();
    let asker = start_with(
        &mut session,
        PathBuf::from("/nowhere/polkit-agent-helper-1"),
    );
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());

    // One that goes once it has been tried.
    let mut session = Session::new();
    let directory = tempfile::TempDir::new().unwrap();
    let helper = directory.path().join("polkit-agent-helper-1");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/polkit-helper");
    std::fs::copy(fixture, &helper).unwrap();
    let asker = start_with(&mut session, helper.clone());
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    std::fs::remove_file(&helper).unwrap();
    tap(&mut session, key::A);
    tap(&mut session, key::RETURN);
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());
}

#[test]
fn a_prompt_whose_screen_goes_is_given_up() {
    let mut session = Session::new();
    let other = session.add_output((WIDTH, HEIGHT));
    session.step();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    let first = session.output();
    session.compositor_mut().remove_output(&first);
    session.eventually(|_| answer.is_finished());
    assert!(answer.join().unwrap().is_err());
    drop(other);
}

#[test]
fn the_prompt_is_drawn_at_the_scale_of_its_screen() {
    let mut session = Session::with_config(&[("config.yaml", "output: { scale: 1.25 }\n")]);
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let _answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    // In the middle of a screen 512 by 384 units large, each unit a pixel
    // and a quarter.
    let scaled = |units: i32| units * 5 / 4;
    let (left, top) = (
        (512 - SIZE.0.cast_signed()) / 2,
        (384 - SIZE.1.cast_signed()) / 2,
    );
    let inside = (scaled(left + 12), scaled(top + 12));
    session.eventually(|session| session.draw().at(inside.0, inside.1) == question());
}

#[test]
fn the_agent_ends_with_the_session() {
    let mut session = Session::new();
    let asker = start(&mut session);
    drop(session);
    let begun = std::time::Instant::now();
    while !asker.running.is_finished() {
        assert!(
            begun.elapsed() < std::time::Duration::from_secs(10),
            "never ended"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(asker.running.join().unwrap(), "the session ended");
}

#[test]
fn the_agent_looks_for_the_compositor_where_the_environment_says() {
    let directory = tempfile::TempDir::new().unwrap();
    let runtime = directory.path().as_os_str().to_owned();
    let variable = move |name: &str| match name {
        "XDG_RUNTIME_DIR" => Some(runtime.clone()),
        "WAYLAND_DISPLAY" => Some(std::ffi::OsString::from("wayland-9")),
        "LANG" => Some(std::ffi::OsString::from("pl_PL.UTF-8")),
        _ => None,
    };
    let failure = omorfos_ask::start(&variable).to_string();
    let socket = directory.path().join("wayland-9");
    assert!(
        failure.starts_with(&format!("{}: ", socket.display())),
        "{failure}"
    );
}

#[test]
fn the_agent_cannot_start_without_its_session_or_polkit() {
    // Where the compositor is reached, but not told which session this is.
    let directory = tempfile::TempDir::new().unwrap();
    let _listening = std::os::unix::net::UnixListener::bind(directory.path().join("w")).unwrap();
    let runtime = directory.path().as_os_str().to_owned();
    let variable = move |name: &str| match name {
        "XDG_RUNTIME_DIR" => Some(runtime.clone()),
        "WAYLAND_DISPLAY" => Some(std::ffi::OsString::from("w")),
        _ => None,
    };
    let failure = omorfos_ask::start(&variable).to_string();
    assert!(failure.contains("XDG_SESSION_ID"), "{failure}");

    // Told, on a bus with no polkit on it.
    let mut session = Session::new();
    let daemon = Daemon::start();
    let (users, groups) = people(directory.path());
    let places = Places {
        system: daemon.address.clone(),
        session: Some("7".to_owned()),
        locale: "C".to_owned(),
        helper: PathBuf::from("/nowhere"),
        users,
        groups,
        me: 0,
        config: None,
        wayland: None,
    };
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, Roles::ASK)
        .unwrap();
    let running = std::thread::spawn(move || omorfos_ask::run(&places, theirs).to_string());
    session.eventually(|_| running.is_finished());
    let failure = running.join().unwrap();
    assert!(failure.contains("PolicyKit1"), "{failure}");
}

/// Whether the box is where it rests: its left edge where it is in the
/// middle of the screen.
fn at_rest(session: &mut Session) -> bool {
    let left = (WIDTH - SIZE.0.cast_signed()) / 2;
    let top = (HEIGHT - SIZE.1.cast_signed()) / 2;
    let picture = session.draw();
    picture.at(left + 3, top + 40) == question() && picture.at(left - 3, top + 40) != question()
}

/// Asks, types what is not right, and returns the session and the box up
/// again, with the configuration `files`.
fn wrong(files: &[(&str, &str)]) -> (Session, Asker, JoinHandle<Result<(), String>>) {
    let mut session = Session::with_config(files);
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    assert!(at_rest(&mut session));
    // Shown, and told so, the box keeps still until there is cause.
    session.send_frames(std::time::Duration::from_millis(1));
    session.step();
    assert!(at_rest(&mut session));
    tap(&mut session, key::A);
    tap(&mut session, key::RETURN);
    session.eventually(|_| helped(&cookie).matches("user me").count() == 2);
    (session, asker, answer)
}

#[test]
fn the_box_shakes_when_what_was_typed_was_not_right_and_comes_to_rest() {
    // The tests' sessions keep still unless a part says otherwise.
    let theme = "question { transition: 120ms; }\n";
    let (mut session, _asker, _answer) = wrong(&[("theme.css", theme)]);
    // Frames are shown a sixtieth of a second apart, by the time the
    // compositor tells them at.
    let mut shown = 0;
    let mut frame = |session: &mut Session| {
        shown += 16;
        session.send_frames(std::time::Duration::from_millis(shown));
    };
    let mut moved = false;
    session.eventually(|session| {
        frame(session);
        moved |= !at_rest(session);
        moved
    });
    session.eventually(|session| {
        frame(session);
        at_rest(session)
    });
}

#[test]
fn a_theme_that_moves_nothing_has_the_box_keep_still() {
    let theme = "question { transition: none; }\n";
    let (mut session, _asker, _answer) = wrong(&[("theme.css", theme)]);
    for _ in 0..30 {
        session.send_frames(std::time::Duration::from_millis(16));
        assert!(at_rest(&mut session));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn the_eye_says_what_it_does_when_the_pointer_rests_on_it() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let _answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    // Under the end of the field, where the tooltip is.
    let left = (WIDTH - SIZE.0.cast_signed()) / 2 + FIELD.0 + FIELD.2.cast_signed();
    let top = (HEIGHT - SIZE.1.cast_signed()) / 2 + FIELD.1 + FIELD.3.cast_signed();
    let under = (left - 30, top + 12);
    // A stretch across it, which its words cross.
    let row = |session: &mut Session| {
        let picture = session.draw();
        (under.0 - 100..under.0)
            .map(|x| picture.at(x, under.1))
            .collect::<Vec<_>>()
    };
    let before = row(&mut session);
    let eye = on_prompt(EYE);
    session
        .compositor_mut()
        .input(Input::Motion { x: eye.0, y: eye.1 }, 0);
    session.eventually(|session| row(session) != before);
    // And not once it has gone.
    session
        .compositor_mut()
        .input(Input::Motion { x: 2.0, y: 2.0 }, 0);
    session.eventually(|session| row(session) == before);
}

/// Whether the keyboard is on the button in `area` of the prompt, by the
/// colour of its top edge.
fn ringed(session: &mut Session, (x, y, across, _): (i32, i32, u32, u32)) -> bool {
    let colour = Theme::default()
        .style(
            Part::QuestionButton,
            States::NONE.with(omorfos_config::State::Focused),
        )
        .border
        .top
        .color;
    let ring = u32::from_be_bytes([colour.alpha, colour.red, colour.green, colour.blue]);
    let left = (WIDTH - SIZE.0.cast_signed()) / 2 + x;
    let top = (HEIGHT - SIZE.1.cast_signed()) / 2 + y;
    session.draw().at(left + across.cast_signed() / 2, top) == ring
}

/// Presses Tab with Shift held.
fn back_tab(session: &mut Session) {
    let shift = |session: &mut Session, pressed| {
        session.compositor_mut().input(
            Input::Key {
                code: key::SHIFT,
                pressed,
            },
            0,
        );
    };
    shift(session, true);
    tap(session, key::TAB);
    shift(session, false);
}

/// A row across the field, which shows what is typed.
fn field_row(session: &mut Session) -> Vec<u32> {
    let left = (WIDTH - SIZE.0.cast_signed()) / 2 + FIELD.0;
    let middle = (HEIGHT - SIZE.1.cast_signed()) / 2 + FIELD.1 + FIELD.3.cast_signed() / 2;
    let picture = session.draw();
    (left..left + FIELD.2.cast_signed())
        .map(|x| picture.at(x, middle))
        .collect()
}

#[test]
fn the_buttons_are_reached_with_tab_and_pressed_with_enter() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    session.eventually(typing);
    let hidden = field_row(&mut session);
    // The eye first, which Enter has show what is typed.
    tap(&mut session, key::TAB);
    session.eventually(|session| ringed(session, EYE) && !typing(session));
    tap(&mut session, key::RETURN);
    session.eventually(|session| field_row(session) != hidden);
    // Then Cancel, Authenticate, and round to the field.
    tap(&mut session, key::TAB);
    session.eventually(|session| ringed(session, CANCEL) && !ringed(session, EYE));
    tap(&mut session, key::TAB);
    session.eventually(|session| ringed(session, GO) && !ringed(session, CANCEL));
    tap(&mut session, key::TAB);
    session.eventually(|session| typing(session) && !ringed(session, GO));
    // And back.
    back_tab(&mut session);
    session.eventually(|session| ringed(session, GO));
    tap(&mut session, key::RETURN);
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()));
    assert!(helped(&cookie).ends_with("typed atx\n"));
}

#[test]
fn typing_goes_back_to_the_field_and_enter_on_cancel_gives_up() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let cookie = asker.directory.path().join("asked");
    let me = rustix::process::getuid().as_raw();
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    for _ in 0..2 {
        back_tab(&mut session);
    }
    session.eventually(|session| ringed(session, CANCEL));
    tap(&mut session, key::A);
    session.eventually(|session| typing(session) && !ringed(session, CANCEL));
    for _ in 0..2 {
        back_tab(&mut session);
    }
    session.eventually(|session| ringed(session, CANCEL));
    tap(&mut session, key::RETURN);
    session.eventually(|_| answer.is_finished());
    let refused = answer.join().unwrap().unwrap_err();
    assert!(refused.contains("Cancelled"), "{refused}");
}
