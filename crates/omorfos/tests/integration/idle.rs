//! Knowing when the user is away, and when something on screen says not to
//! mind.

use std::time::{Duration, Instant};

use omorfos::Input;
use omorfos_config::Action;
use serde_json::json;

use crate::common::{Client, Event, Session};
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

/// A session with a window, whose client asks to be told when the user is
/// away.
fn watched() -> (Session, Client, crate::common::Toplevel) {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();
    client.watch_idle(AWAY);
    session.roundtrip(&mut client);
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
    let (mut session, mut client, window) = watched();
    let inhibitor = client.inhibit_idle(&window.surface);
    assert!(
        !idles_within(&mut session, &mut client, A_WHILE),
        "something is being watched"
    );

    inhibitor.destroy();
    assert!(idles_within(&mut session, &mut client, SOON), "it is over");
}

#[test]
fn part_of_a_window_can_ask_as_well() {
    let (mut session, mut client, window) = watched();
    let part = client.subsurface(&window.surface, (10, 10), (50, 50), BLUE);
    let _inhibitor = client.inhibit_idle(&part);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));
}

#[test]
fn a_window_that_is_not_shown_has_no_say() {
    let (mut session, mut client, window) = watched();
    let _inhibitor = client.inhibit_idle(&window.surface);
    let elsewhere: Action = serde_json::from_value(json!({ "workspace": 2 })).unwrap();
    session.compositor_mut().perform(&elsewhere);
    assert!(idles_within(&mut session, &mut client, SOON));
}

#[test]
fn nothing_keeps_a_locked_session_from_going_idle() {
    let (mut session, mut client, window) = watched();
    let _inhibitor = client.inhibit_idle(&window.surface);
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

    let (mut session, mut client, _window) = watched();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let (surface, _role) = session.open_layer(&mut client, &panel, (WIDTH, 30), BLUE);
    let _inhibitor = client.inhibit_idle(&surface);
    assert!(!idles_within(&mut session, &mut client, A_WHILE));
}
