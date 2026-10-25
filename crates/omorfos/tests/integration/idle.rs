//! Knowing when the user is away, and when something on screen says not to
//! mind.

use std::time::{Duration, Instant};

use omorfos::Input;
use omorfos_config::Action;
use omorfos_ipc::Awake;
use serde_json::json;

use crate::common::{Client, Event, Session, runnable};
use crate::windows::{BLUE, RED};

/// The time without input after which the client asks to be told.
const AWAY: u32 = 40;

/// Whether `client` is told the user is away within `wait`.
fn idles_within(session: &mut Session, client: &mut Client, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        session.roundtrip(client);
        if client.take_events().contains(&Event::Idle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

const SOON: Duration = Duration::from_secs(10);
const A_WHILE: Duration = Duration::from_millis(250);

/// A session with a window.
fn opened() -> (Session, Client, crate::common::Toplevel) {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();
    (session, client, window)
}

/// Has the client ask to be told when the user is away.
///
/// The time it waits is real and short. Whatever is to keep the session
/// from going idle has to be in place before this is asked, or on a busy
/// machine the user is away before it is.
fn watch(session: &mut Session, client: &mut Client) {
    session.roundtrip(client);
    client.watch_idle(AWAY);
    session.roundtrip(client);
}

/// A session with a window, whose client asks to be told when the user is
/// away.
fn watched() -> (Session, Client, crate::common::Toplevel) {
    let (mut session, mut client, window) = opened();
    watch(&mut session, &mut client);
    (session, client, window)
}

#[test]
fn a_client_is_told_when_the_user_has_been_away_and_when_they_are_back() {
    let (mut session, mut client, _window) = watched();
    assert!(idles_within(&mut session, &mut client, SOON));

    session
        .compositor_mut()
        .input(Input::Motion { x: 400.0, y: 400.0 }, 0);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Back));
    assert!(
        idles_within(&mut session, &mut client, SOON),
        "and away again"
    );
}

#[test]
fn a_window_on_screen_can_ask_for_the_session_not_to_go_idle() {
    let (mut session, mut client, window) = opened();
    let inhibitor = client.inhibit_idle(&window.surface);
    watch(&mut session, &mut client);
    assert!(
        !idles_within(&mut session, &mut client, A_WHILE),
        "something is being watched"
    );

    inhibitor.destroy();
    assert!(idles_within(&mut session, &mut client, SOON), "it is over");
}

#[test]
fn part_of_a_window_can_ask_as_well() {
    let (mut session, mut client, window) = opened();
    let part = client.subsurface(&window.surface, (10, 10), (50, 50), BLUE);
    let _inhibitor = client.inhibit_idle(&part);
    watch(&mut session, &mut client);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));
}

#[test]
fn a_window_that_is_not_shown_has_no_say() {
    let (mut session, mut client, window) = watched();
    let _inhibitor = client.inhibit_idle(&window.surface);
    let elsewhere: Action = serde_json::from_value(json!({ "workspace": 2 })).unwrap();
    session.compositor_mut().perform(&elsewhere);
    session.settle();
    assert!(idles_within(&mut session, &mut client, SOON));
}

#[test]
fn nothing_keeps_a_locked_session_from_going_idle() {
    let (mut session, mut client, window) = opened();
    let _inhibitor = client.inhibit_idle(&window.surface);
    watch(&mut session, &mut client);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));

    let mut locker = session.connect();
    let _lock = locker.lock();
    session.roundtrip(&mut locker);
    assert!(session.compositor().is_locked());
    assert!(idles_within(&mut session, &mut client, SOON));
}

#[test]
fn a_panel_can_ask_too() {
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
        Anchor, KeyboardInteractivity,
    };

    use crate::common::{LayerShape, WIDTH};

    let (mut session, mut client, _window) = opened();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let (surface, _role) = session.open_layer(&mut client, &panel, (WIDTH, 30), BLUE);
    let _inhibitor = client.inhibit_idle(&surface);
    watch(&mut session, &mut client);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));
}

/// A session that turns its screens off after a minute without input.
fn dozing() -> Session {
    let mut session = Session::with_config(&[("config.yaml", "idle: { screen_off: 60 }\n")]);
    session.step();
    session
}

fn seconds(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

#[test]
fn the_screens_stay_on_unless_told_to_turn_off() {
    let mut session = Session::new();
    session.advance(seconds(100_000));
    assert!(session.compositor().screens_on());
    assert_eq!(session.compositor().deadline(), None, "nothing to wait for");
}

#[test]
fn the_screens_turn_off_after_the_time_without_input_and_on_at_the_next() {
    let mut session = dozing();
    assert!(session.compositor().screens_on());
    assert_eq!(
        session.compositor().deadline(),
        Some(session.now + seconds(60)),
        "the session wakes itself to turn them off"
    );

    session.advance(seconds(59));
    assert!(session.compositor().screens_on(), "not yet");
    session.advance(seconds(1));
    assert!(!session.compositor().screens_on());
    assert_eq!(session.compositor().deadline(), None, "and sleeps on");

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 0);
    session.step();
    assert!(session.compositor().screens_on(), "the user is back");
    session.advance(seconds(59));
    assert!(session.compositor().screens_on(), "counted from then");
    session.advance(seconds(2));
    assert!(!session.compositor().screens_on());
}

#[test]
fn something_being_watched_keeps_the_screens_on() {
    let mut session = dozing();
    let mut client = session.connect();
    let window = session.open(&mut client, "film", (200, 100), RED);
    let inhibitor = client.inhibit_idle(&window.surface);
    session.roundtrip(&mut client);

    session.advance(seconds(600));
    assert!(session.compositor().screens_on());

    inhibitor.destroy();
    session.roundtrip(&mut client);
    session.advance(seconds(59));
    assert!(
        session.compositor().screens_on(),
        "a minute from when it ended"
    );
    session.advance(seconds(2));
    assert!(!session.compositor().screens_on());
}

#[test]
fn a_locked_session_turns_its_screens_off_whatever_is_being_watched() {
    let mut session = dozing();
    let mut client = session.connect();
    let window = session.open(&mut client, "film", (200, 100), RED);
    let _inhibitor = client.inhibit_idle(&window.surface);
    let mut locker = session.connect();
    let _lock = locker.lock();
    session.roundtrip(&mut locker);

    session.advance(seconds(61));
    assert!(!session.compositor().screens_on());
}

#[test]
fn the_time_can_be_changed_or_taken_away_while_running() {
    let mut session = dozing();
    session.advance(seconds(61));
    assert!(!session.compositor().screens_on());

    session.write_config("config.yaml", "idle: { screen_off: 0 }\n");
    session.compositor_mut().reload_config();
    session.step();
    assert!(session.compositor().screens_on(), "never means on");

    session.write_config("config.yaml", "idle: { screen_off: 30 }\n");
    session.compositor_mut().reload_config();
    session.step();
    assert!(!session.compositor().screens_on(), "long since");
}

// --- Locking and sleeping when away -------------------------------------

/// A session that locks after a minute away, with a locker that only notes
/// in `noted` that it was started.
fn locking(noted: &std::path::Path) -> Session {
    let config = format!(
        "idle: {{ lock: 60 }}\nlocker: [sh, -c, 'echo started >> \"$0\"', '{}']\n",
        noted.display()
    );
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    session.step();
    session
}

/// How many times what notes in `noted` has been started.
fn started(noted: &std::path::Path) -> usize {
    std::fs::read_to_string(noted).map_or(0, |noted| noted.lines().count())
}

/// Gives what was started the moment it may need to have noted that it was:
/// enough for a check that it was not to mean something.
fn a_moment() {
    std::thread::sleep(Duration::from_millis(200));
}

#[test]
fn away_for_the_time_configured_the_session_is_locked_once_each_time() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let at = session.now + seconds(60);
    assert_eq!(
        session.compositor().deadline(),
        Some(at),
        "it is waited for"
    );

    session.advance(seconds(59));
    a_moment();
    assert_eq!(started(&noted), 0, "not yet");
    session.advance(seconds(2));
    session.eventually(|_| started(&noted) == 1);
    session.advance(seconds(600));
    a_moment();
    assert_eq!(started(&noted), 1, "once for the time away");
    assert_eq!(session.compositor().deadline(), None);

    // Back, and away again.
    session
        .compositor_mut()
        .input(Input::Motion { x: 5.0, y: 5.0 }, 0);
    session.advance(seconds(61));
    session.eventually(|_| started(&noted) == 2);
}

#[test]
fn something_being_watched_keeps_the_session_from_locking() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut client = session.connect();
    let window = session.open(&mut client, "film", (200, 100), RED);
    let _inhibitor = client.inhibit_idle(&window.surface);
    session.roundtrip(&mut client);

    session.advance(seconds(120));
    a_moment();
    assert_eq!(started(&noted), 0);
}

#[test]
fn a_session_locked_already_is_not_locked_again_when_away() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let (_locker, _lock, _surface) = crate::lock::lock(&mut session);

    session.advance(seconds(61));
    a_moment();
    assert_eq!(started(&noted), 0);
}

#[test]
fn away_for_the_time_configured_the_machine_is_asked_to_sleep() {
    let directory = tempfile::TempDir::new().unwrap();
    let asked = directory.path().join("asked");
    // A stand-in for what watches over the session, which notes what it is
    // asked to do.
    let program = directory.path().join("omorfos-session");
    let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\n", asked.display());
    runnable(&program, &script);
    let mut session = Session::with_config(&[("config.yaml", "idle: { sleep: 1800 }\n")]);
    session.compositor_mut().set_session(&program);
    session.step();

    session.advance(seconds(1801));
    let said = || std::fs::read_to_string(&asked).unwrap_or_default();
    session.eventually(|_| said() == "sleep\n");
    session.advance(seconds(3600));
    a_moment();
    assert_eq!(said(), "sleep\n", "once for the time away");
}

// --- Staying awake --------------------------------------------------------

/// Whether the session says, to those that ask it, that it is kept awake,
/// and until when.
fn said_awake(session: &mut Session, peer: &mut omorfos_ipc::Client) -> Option<Awake> {
    let omorfos_ipc::Reply::State(state) = session.ask(peer, &omorfos_ipc::Request::State) else {
        unreachable!("the state was asked for");
    };
    state.awake
}

#[test]
fn kept_awake_the_session_does_not_lock_until_it_is_let_go() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    assert_eq!(said_awake(&mut session, &mut peer), None);

    session.compositor_mut().perform(&Action::StayAwake);
    session.step();
    assert_eq!(
        said_awake(&mut session, &mut peer),
        Some(Awake { until: None }),
        "until let go"
    );
    session.advance(seconds(3600));
    a_moment();
    assert_eq!(started(&noted), 0);

    session.compositor_mut().perform(&Action::StayAwake);
    session.step();
    assert_eq!(
        said_awake(&mut session, &mut peer),
        None,
        "asked again, it is let go"
    );
    session.advance(seconds(61));
    session.eventually(|_| started(&noted) == 1);
}

#[test]
fn kept_awake_for_a_while_the_session_locks_once_that_is_over() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    let hour: Action = serde_json::from_value(json!({ "stay_awake_for": 3600 })).unwrap();
    let before = std::time::SystemTime::now();

    session.compositor_mut().perform(&hour);
    session.step();
    let until = said_awake(&mut session, &mut peer).and_then(|awake| awake.until);
    let until = until.unwrap();
    let from = before
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        (from + 3599..=from + 3601).contains(&until),
        "an hour on: {until}"
    );
    session.advance(seconds(3599));
    a_moment();
    assert_eq!(started(&noted), 0, "not within the hour");

    session.advance(seconds(2));
    assert_eq!(said_awake(&mut session, &mut peer), None, "over");
    session.advance(seconds(61));
    session.eventually(|_| started(&noted) == 1);
}

#[test]
fn locking_by_hand_lets_go_of_the_session_being_kept_awake() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    session.compositor_mut().perform(&Action::StayAwake);
    session.step();

    session.compositor_mut().perform(&Action::Lock);
    session.eventually(|_| started(&noted) == 1);
    assert_eq!(said_awake(&mut session, &mut peer), None);
}

#[test]
fn a_program_waiting_for_the_user_to_be_away_waits_while_the_session_is_kept_awake() {
    let (mut session, mut client, _window) = opened();
    session.compositor_mut().perform(&Action::StayAwake);
    session.step();
    watch(&mut session, &mut client);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));
}

#[test]
fn kept_awake_for_a_while_the_session_is_brought_up_to_date_when_that_is_over() {
    let mut session = Session::new();
    session.step();
    let hour: Action = serde_json::from_value(json!({ "stay_awake_for": 3600 })).unwrap();
    session.compositor_mut().perform(&hour);
    session.step();
    assert_eq!(
        session.compositor().deadline(),
        Some(session.now + seconds(3600))
    );
}

// --- What programs ask by way of what speaks for them ------------------

/// Says over `peer` that the programs it speaks for keep the session from
/// going idle, or from sleeping.
fn hold(session: &mut Session, peer: &mut omorfos_ipc::Client, idle: bool, sleep: bool) {
    let asked = omorfos_ipc::Request::Hold { idle, sleep };
    assert_eq!(session.ask(peer, &asked), omorfos_ipc::Reply::Done);
}

#[test]
fn a_program_kept_from_going_idle_keeps_the_session_from_locking_while_it_is_connected() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    hold(&mut session, &mut peer, true, false);

    session.advance(seconds(600));
    a_moment();
    assert_eq!(started(&noted), 0);

    hold(&mut session, &mut peer, false, false);
    session.advance(seconds(61));
    session.eventually(|_| started(&noted) == 1);
}

#[test]
fn what_a_connection_held_is_let_go_when_it_ends() {
    let directory = tempfile::TempDir::new().unwrap();
    let noted = directory.path().join("noted");
    let mut session = locking(&noted);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    hold(&mut session, &mut peer, true, false);
    drop(peer);

    session.eventually(|session| {
        session.advance(seconds(61));
        started(&noted) == 1
    });
}

#[test]
fn a_program_kept_from_sleeping_keeps_the_machine_awake_even_while_locked() {
    let directory = tempfile::TempDir::new().unwrap();
    let asked = directory.path().join("asked");
    let program = directory.path().join("omorfos-session");
    let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\n", asked.display());
    runnable(&program, &script);
    let config = "idle: { sleep: 600, screen_off: 60 }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.compositor_mut().set_session(&program);
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    // Music plays, and a film is kept from showing behind the lock.
    hold(&mut session, &mut peer, true, true);
    let (_locker, _lock, _surface) = crate::lock::lock(&mut session);

    session.advance(seconds(3600));
    a_moment();
    assert!(!asked.exists(), "not put to sleep");
    assert!(
        !session.compositor().screens_on(),
        "though the screens go off"
    );

    hold(&mut session, &mut peer, false, false);
    let said = || std::fs::read_to_string(&asked).unwrap_or_default();
    session.eventually(|_| said() == "sleep\n");
}
