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
    start_with_door(session, helper, |directory| {
        Some(directory.join(omorfos_ask::DOOR))
    })
}

/// The same, with the door GnuPG's pinentry asks at where `door` says, in
/// the agent's directory.
fn start_with_door(
    session: &mut Session,
    helper: PathBuf,
    door: impl FnOnce(&Path) -> Option<PathBuf>,
) -> Asker {
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
        door: door(directory.path()),
        pinentry: pinentry(),
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
        door: None,
        pinentry: PathBuf::from("/nowhere"),
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

/// Presses `code` down, or lets it go.
fn key_down(session: &mut Session, code: Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

#[test]
fn backspace_held_takes_out_one_character_after_another_as_keys_repeat() {
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
    for _ in 0..6 {
        tap(&mut session, key::Q);
    }
    // Held until no dot is left where they are drawn, which one press alone
    // would not do; that more are taken out than there are is nothing.
    let undotted = |session: &mut Session| {
        let row = field_row(session);
        row[12..60].iter().all(|pixel| *pixel == row[150])
    };
    session.eventually(|session| !undotted(session));
    key_down(&mut session, key::BACKSPACE, true);
    session.eventually(undotted);
    key_down(&mut session, key::BACKSPACE, false);
    for code in [key::A, key::T, key::X, key::RETURN] {
        tap(&mut session, code);
    }
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()));
    assert!(
        helped(&cookie).ends_with("typed atx\n"),
        "{}",
        helped(&cookie)
    );
}

#[test]
fn ctrl_or_super_and_a_choose_all_that_is_typed_to_type_over_or_take_out() {
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
    let chosen = |session: &mut Session, held: Keycode| {
        key_down(session, held, true);
        tap(session, key::A);
        key_down(session, held, false);
    };
    // Taken out at once by Backspace, then typed over: only what is typed
    // after both is what polkit's helper is given.
    for code in [key::Q, key::Q] {
        tap(&mut session, code);
    }
    chosen(&mut session, key::CONTROL);
    tap(&mut session, key::BACKSPACE);
    tap(&mut session, key::Q);
    chosen(&mut session, key::SUPER);
    for code in [key::A, key::T, key::X, key::RETURN] {
        tap(&mut session, code);
    }
    session.eventually(|_| answer.is_finished());
    assert_eq!(answer.join().unwrap(), Ok(()));
    assert!(
        helped(&cookie).ends_with("typed atx\n"),
        "{}",
        helped(&cookie)
    );
}

// --- GnuPG's passphrases ------------------------------------------------

/// The pinentry, as the workspace builds it beside the tests.
fn pinentry() -> PathBuf {
    let tests = std::env::current_exe().unwrap();
    let built = tests
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("omorfos-pinentry");
    assert!(
        built.exists(),
        "omorfos-pinentry is built with the workspace: {}",
        built.display()
    );
    built
}

/// A home of GnuPG's of its own, whose agent asks with the pinentry, and a
/// key in it of Alice's whose passphrase is `atx`. Its agent is stopped
/// when it goes.
struct Gnupg {
    home: tempfile::TempDir,
    runtime: PathBuf,
}

impl Gnupg {
    fn new(asker: &Asker) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::TempDir::new().unwrap();
        std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        // A new passphrase is let be short and all letters: GnuPG would
        // otherwise ask to confirm it, which the pinentry says no to.
        let conf = format!(
            "pinentry-program {}\nallow-loopback-pinentry\nmin-passphrase-len 0\n\
             min-passphrase-nonalpha 0\n",
            pinentry().display()
        );
        std::fs::write(home.path().join("gpg-agent.conf"), conf).unwrap();
        let gnupg = Self {
            home,
            runtime: asker.directory.path().to_owned(),
        };
        let made = gnupg
            .gpg(&["--pinentry-mode", "loopback", "--passphrase", "atx"])
            .args([
                "--quick-gen-key",
                "Alice <alice@example.org>",
                "ed25519",
                "sign",
                "never",
            ])
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        gnupg
    }

    /// `gpg`, in this home, as a program of the session runs it.
    fn gpg(&self, options: &[&str]) -> Command {
        let mut gpg = Command::new("gpg");
        gpg.env("GNUPGHOME", self.home.path())
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .args(["--batch", "--yes"])
            .args(options);
        gpg
    }

    /// Signs a file, which asks for the key's passphrase, on a thread of its
    /// own; and says whether it was signed.
    fn sign(&self) -> JoinHandle<bool> {
        let file = self.home.path().join("note.txt");
        std::fs::write(&file, "hello").unwrap();
        let mut signing = self.gpg(&["--pinentry-mode", "ask", "--sign"]);
        signing.arg(&file);
        std::thread::spawn(move || signing.output().is_ok_and(|output| output.status.success()))
    }
}

impl Drop for Gnupg {
    fn drop(&mut self) {
        let _ = Command::new("gpgconf")
            .env("GNUPGHOME", self.home.path())
            .args(["--kill", "gpg-agent"])
            .status();
    }
}

#[test]
fn a_gnupg_passphrase_is_asked_in_the_sealed_prompt_and_given_to_gpg_agent() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);

    let signed = gnupg.sign();
    wait_up(&mut session);
    // Wrong first: the agent asks again, and the box says so as it shakes.
    tap(&mut session, key::A);
    tap(&mut session, key::RETURN);
    session.eventually(|session| !up(session) || signed.is_finished());
    wait_up(&mut session);
    assert!(!signed.is_finished(), "asked again");
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    tap(&mut session, key::RETURN);
    finished(&mut session, &signed);
    assert!(signed.join().unwrap(), "signed with the key");
    session.eventually(|session| !up(session));
}

#[test]
fn a_gnupg_passphrase_given_up_on_signs_nothing() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);

    let signed = gnupg.sign();
    wait_up(&mut session);
    tap(&mut session, key::ESCAPE);
    finished(&mut session, &signed);
    assert!(!signed.join().unwrap());
}

#[test]
fn a_pinentry_not_started_by_gpg_agent_is_not_heard() {
    use std::io::Write;

    let mut session = Session::new();
    let asker = start(&mut session);
    let mut started = Command::new(pinentry())
        .env("XDG_RUNTIME_DIR", asker.directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut asking = started.stdin.take().unwrap();
    asking.write_all(b"GETPIN\nBYE\n").unwrap();
    drop(asking);
    // It is turned away when the agent next looks, and says so to whoever
    // asked it.
    let said = std::thread::spawn(move || started.wait_with_output().unwrap().stdout);
    session.eventually(|_| said.is_finished());
    let said = String::from_utf8(said.join().unwrap()).unwrap();
    assert!(said.contains("ERR 83886165"), "{said}");
    assert!(!up(&mut session), "and nothing is asked");
}

#[test]
fn without_a_door_for_gnupg_polkit_is_still_asked_for() {
    let mut session = Session::new();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/polkit-helper");
    let asker = start_with_door(&mut session, helper, |_| {
        Some(PathBuf::from("/nowhere/door"))
    });
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
    tap(&mut session, key::RETURN);
    session.eventually(|_| answer.is_finished() || helped(&cookie).contains("typed atx"));
    assert!(!asker.running.is_finished());
}

#[test]
fn a_gnupg_passphrase_whose_agent_goes_is_asked_for_no_longer() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);

    let signed = gnupg.sign();
    wait_up(&mut session);
    let stopped = Command::new("gpgconf")
        .env("GNUPGHOME", gnupg.home.path())
        .args(["--kill", "gpg-agent"])
        .status()
        .unwrap();
    assert!(stopped.success());
    session.eventually(|session| !up(session));
    finished(&mut session, &signed);
    assert!(!signed.join().unwrap());
}

#[test]
fn gnupg_waits_its_turn_behind_polkit_and_is_asked_once_polkit_gives_up() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);
    let me = rustix::process::getuid().as_raw();
    let cookie = asker.directory.path().join("asked");
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);

    // Asked meanwhile, GnuPG waits, once its pinentry has knocked.
    let signed = gnupg.sign();
    knocked(&mut session, &asker);
    for _ in 0..100 {
        session.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!signed.is_finished(), "it waits");
    let reply = asker.polkit.connection.call_method(
        Some(asker.agent.as_str()),
        PATH,
        Some(AGENT),
        "CancelAuthentication",
        &(cookie.display().to_string(),),
    );
    reply.unwrap();
    session.eventually(|_| answer.is_finished());
    wait_up(&mut session);
    for code in [key::A, key::T, key::X] {
        tap(&mut session, code);
    }
    tap(&mut session, key::RETURN);
    finished(&mut session, &signed);
    assert!(signed.join().unwrap(), "then it is asked");
}

/// The pinentries the asker has let in at its door and not yet answered,
/// each by the number the kernel knows its connection by. A connection
/// there that the asker has accepted is listed as connected, the sixth of
/// what the kernel lists of it, until the asker closes it, which it does as
/// it gives the pinentry what was typed.
fn let_in(asker: &Asker) -> Vec<String> {
    let door = asker.directory.path().join(omorfos_ask::DOOR);
    let door = door.to_string_lossy().into_owned();
    let sockets = std::fs::read_to_string("/proc/net/unix").unwrap();
    let connected = sockets.lines().filter_map(|socket| {
        let fields: Vec<&str> = socket.split_whitespace().collect();
        let there = fields.last() == Some(&door.as_str()) && fields.get(5) == Some(&"03");
        there.then(|| fields[6].to_owned())
    });
    connected.collect()
}

/// Keeps the session going until `done`, giving up after a minute, as
/// `failure` says: how long GnuPG takes is its own affair, the more so
/// with other tests at work.
fn until(session: &mut Session, mut done: impl FnMut() -> bool, failure: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !done() && std::time::Instant::now() < deadline {
        session.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(done(), "{failure}");
}

/// Keeps the session going until GnuPG's pinentry has knocked at the
/// asker's door and been let in, which is when it is in line to be asked:
/// the asker takes it in before it hears anything polkit says next.
fn knocked(session: &mut Session, asker: &Asker) {
    let knocked = || !let_in(asker).is_empty();
    until(
        session,
        knocked,
        "GnuPG's pinentry did not knock in a minute",
    );
}

/// Keeps the session going until `gnupg`, a program of GnuPG's running on
/// a thread of its own, is done: GnuPG takes its time over a key, the more
/// so with other tests at work, so it is given up on only after a minute.
fn finished<T>(session: &mut Session, gnupg: &JoinHandle<T>) {
    until(
        session,
        || gnupg.is_finished(),
        "GnuPG did not finish in a minute",
    );
}

/// Whether anything has been typed in the field, by the dots it shows.
fn dotted(session: &mut Session) -> bool {
    let picture = session.draw();
    let left = (WIDTH - SIZE.0.cast_signed()) / 2 + FIELD.0 + 8;
    let middle = (HEIGHT - SIZE.1.cast_signed()) / 2 + FIELD.1 + FIELD.3.cast_signed() / 2;
    let focused = States::NONE.with(omorfos_config::State::Focused);
    let colour = Theme::default()
        .style(Part::QuestionInput, focused)
        .background;
    let field = u32::from_be_bytes([colour.alpha, colour.red, colour.green, colour.blue]);
    // Past where the line that marks where the next will go stands in an
    // empty field.
    (left + 3..left + 40).any(|x| picture.at(x, middle) != field)
}

/// Types `word` in the sealed prompt for the pinentry that has knocked,
/// and goes on; then waits for the asker to have given it what was typed,
/// so that what is typed next is for the next.
///
/// What is on the screen does not say when either is: once what was typed
/// is taken, the prompt shows an empty field until it is taken down, and
/// what is typed meanwhile is for nobody.
fn type_in(session: &mut Session, asker: &Asker, word: &[Keycode]) {
    let mut asking = Vec::new();
    let knocked = || {
        asking = let_in(asker);
        !asking.is_empty()
    };
    until(
        session,
        knocked,
        "GnuPG's pinentry did not knock in a minute",
    );
    wait_up(session);
    for code in word {
        tap(session, *code);
    }
    session.eventually(dotted);
    tap(session, key::RETURN);
    let answered = || let_in(asker).iter().all(|now| !asking.contains(now));
    until(
        session,
        answered,
        "the asker did not answer GnuPG in a minute",
    );
}

#[test]
fn a_new_passphrase_is_asked_for_twice_in_the_sealed_prompt() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);
    let mut changing = gnupg.gpg(&["--pinentry-mode", "ask", "--passwd", "alice@example.org"]);
    let changed =
        std::thread::spawn(move || changing.output().is_ok_and(|done| done.status.success()));

    // The one it has, then the new one, and the new one again.
    type_in(&mut session, &asker, &[key::A, key::T, key::X]);
    type_in(&mut session, &asker, &[key::Q, key::A, key::T]);
    type_in(&mut session, &asker, &[key::Q, key::A, key::T]);
    finished(&mut session, &changed);
    assert!(changed.join().unwrap());
    let signed = gnupg
        .gpg(&[
            "--pinentry-mode",
            "loopback",
            "--passphrase",
            "qat",
            "--sign",
        ])
        .arg(gnupg.home.path().join("gpg-agent.conf"))
        .output()
        .unwrap();
    assert!(signed.status.success(), "the key has the new one");
}

#[test]
fn a_passphrase_a_program_asks_gpg_agent_for_itself_names_no_key() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);
    let mut asking = Command::new("gpg-connect-agent");
    asking
        .env("GNUPGHOME", gnupg.home.path())
        .env("XDG_RUNTIME_DIR", asker.directory.path())
        .args([
            "GET_PASSPHRASE --data mine X X Type+your+login+password",
            "/bye",
        ]);
    let given = std::thread::spawn(move || asking.output().unwrap().stdout);

    type_in(&mut session, &asker, &[key::Q, key::A, key::T]);
    finished(&mut session, &given);
    let given = String::from_utf8(given.join().unwrap()).unwrap();
    assert!(given.contains("D qat"), "{given}");
}

#[test]
fn a_gnupg_passphrase_whose_agent_goes_while_it_waits_is_never_asked() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);
    let me = rustix::process::getuid().as_raw();
    let cookie = asker.directory.path().join("asked");
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    wait_up(&mut session);
    let signed = gnupg.sign();
    knocked(&mut session, &asker);

    // The agent goes while its pinentry waits behind polkit's question.
    let stopped = Command::new("gpgconf")
        .env("GNUPGHOME", gnupg.home.path())
        .args(["--kill", "gpg-agent"])
        .status()
        .unwrap();
    assert!(stopped.success());
    finished(&mut session, &signed);
    assert!(!signed.join().unwrap());
    for _ in 0..50 {
        session.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let reply = asker.polkit.connection.call_method(
        Some(asker.agent.as_str()),
        PATH,
        Some(AGENT),
        "CancelAuthentication",
        &(cookie.display().to_string(),),
    );
    reply.unwrap();
    session.eventually(|_| answer.is_finished());
    session.eventually(|session| !up(session));
    for _ in 0..50 {
        session.step();
    }
    assert!(!up(&mut session), "and GnuPG's is not asked after it");
}

#[test]
fn polkit_waits_its_turn_behind_gnupg() {
    let mut session = Session::new();
    let asker = start(&mut session);
    let gnupg = Gnupg::new(&asker);
    let signed = gnupg.sign();
    wait_up(&mut session);

    // Asked meanwhile, polkit waits for GnuPG's to be answered.
    let me = rustix::process::getuid().as_raw();
    let cookie = asker.directory.path().join("asked");
    let answer = begin(
        &asker.polkit.connection,
        &asker.agent,
        &cookie,
        (&[me], &[]),
    );
    for _ in 0..50 {
        session.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(helped(&cookie).is_empty(), "its helper is not started yet");
    type_in(&mut session, &asker, &[key::A, key::T, key::X]);
    finished(&mut session, &signed);
    assert!(signed.join().unwrap());
    session.eventually(|_| !helped(&cookie).is_empty());
    wait_up(&mut session);
    tap(&mut session, key::ESCAPE);
    session.eventually(|_| answer.is_finished());
}
