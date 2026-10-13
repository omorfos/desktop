//! Finding the panels and the titlebar again on a workspace made for a
//! fullscreen window.

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const LEFT: u32 = 0x110;
const TITLEBAR: u32 = 0xff23_262e;
/// How long the pointer rests against an edge before what is there comes
/// back.
const REST: Duration = Duration::from_millis(300);

/// A panel 20 high along one edge of the screen.
fn panel(edge: Anchor) -> LayerShape {
    LayerShape {
        layer: Layer::Top,
        anchor: edge | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 20,
        keyboard: KeyboardInteractivity::None,
    }
}

/// A session with a green panel along the top, a blue one along the
/// bottom, and a red window that has been made fullscreen. The window goes
/// on drawing itself 200 by 100, so the rest of the screen says what else
/// is there.
fn fullscreen() -> (Session, Client) {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &panel(Anchor::Top), (640, 20), GREEN);
    session.open_layer(&mut client, &panel(Anchor::Bottom), (640, 20), BLUE);
    session.open_decorated(&mut client, "player", (200, 100), RED);
    assert_eq!(seen(&mut session), [GREEN, BACKGROUND, BLUE]);
    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    (session, client)
}

/// What is at the right of the screen where the top panel is, just under
/// it where a titlebar would be, and where the bottom panel is.
fn seen(session: &mut Session) -> [u32; 3] {
    let picture = session.draw();
    [
        picture.at(600, 10),
        picture.at(600, 34),
        picture.at(600, 470),
    ]
}

fn point(session: &mut Session, x: i32, y: i32) {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    session.step();
}

fn click(session: &mut Session, x: i32, y: i32) {
    point(session, x, y);
    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    session.step();
}

fn places(session: &Session) -> usize {
    session.compositor().desktop().workspaces().count()
}

#[test]
fn a_fullscreen_window_has_the_screen_to_itself() {
    let (mut session, mut client) = fullscreen();
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "no panels");

    // A panel that is not shown is not there to be clicked, either.
    client.take_events();
    click(&mut session, 600, 10);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "{events:?}"
    );
}

#[test]
fn resting_the_pointer_against_the_top_brings_back_the_panel_and_the_titlebar() {
    let (mut session, _client) = fullscreen();

    point(&mut session, 600, 0);
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "not at a touch");
    let due = session.compositor().deadline();
    assert_eq!(due, Some(session.now + REST), "it is waited for");

    session.advance(REST);
    assert_eq!(seen(&mut session), [GREEN, TITLEBAR, BACKGROUND]);
    assert_eq!(session.compositor().deadline(), None);

    // The window has not been told to be any other size: it is where it
    // was, under what was laid over it.
    assert_eq!(session.draw().at(100, 80), RED);

    // They stay while the pointer is on them, and go when it leaves.
    point(&mut session, 600, 40);
    assert_eq!(seen(&mut session), [GREEN, TITLEBAR, BACKGROUND]);
    point(&mut session, 600, 60);
    assert_eq!(seen(&mut session), [BACKGROUND; 3]);
}

#[test]
fn the_titlebar_that_is_brought_back_works_as_a_titlebar_does() {
    let (mut session, _client) = fullscreen();
    assert_eq!(places(&session), 2);
    point(&mut session, 600, 0);
    session.advance(REST);

    // Its second button is the way out of fullscreen.
    click(&mut session, 36, 34);
    assert_eq!(places(&session), 1);
    let mut frames = session.compositor().desktop().frames();
    assert!(!frames.next().unwrap().fullscreen);
}

#[test]
fn a_panel_that_is_brought_back_can_be_used() {
    let (mut session, mut client) = fullscreen();
    point(&mut session, 600, 0);
    session.advance(REST);
    client.take_events();

    click(&mut session, 600, 10);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "{events:?}"
    );
}

#[test]
fn resting_the_pointer_against_the_bottom_brings_back_the_panel_there() {
    let (mut session, _client) = fullscreen();

    point(&mut session, 600, 479);
    session.advance(REST);
    assert_eq!(seen(&mut session), [BACKGROUND, BACKGROUND, BLUE]);

    point(&mut session, 600, 465);
    assert_eq!(seen(&mut session), [BACKGROUND, BACKGROUND, BLUE]);
    point(&mut session, 600, 300);
    assert_eq!(seen(&mut session), [BACKGROUND; 3]);
}

#[test]
fn a_pointer_that_does_not_rest_brings_nothing_back() {
    let (mut session, _client) = fullscreen();

    // Against the top and away again, then against the bottom: neither
    // for long enough, and the time at the one does not count for the
    // other.
    point(&mut session, 600, 0);
    session.advance(Duration::from_millis(200));
    point(&mut session, 600, 479);
    session.advance(Duration::from_millis(200));
    assert_eq!(seen(&mut session), [BACKGROUND; 3]);

    session.advance(Duration::from_millis(100));
    assert_eq!(seen(&mut session), [BACKGROUND, BACKGROUND, BLUE]);
}

#[test]
fn a_key_sends_away_what_was_brought_back_until_the_pointer_comes_again() {
    let (mut session, _client) = fullscreen();
    point(&mut session, 600, 0);
    session.advance(REST);
    assert_eq!(seen(&mut session), [GREEN, TITLEBAR, BACKGROUND]);

    for pressed in [true, false] {
        let code = key::A;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    assert_eq!(seen(&mut session), [BACKGROUND; 3]);
    session.advance(Duration::from_secs(1));
    assert_eq!(seen(&mut session), [BACKGROUND; 3], "it does not come back");
    assert_eq!(
        session.compositor().deadline(),
        None,
        "nor is it waited for"
    );

    point(&mut session, 600, 100);
    point(&mut session, 600, 0);
    session.advance(REST);
    assert_eq!(seen(&mut session), [GREEN, TITLEBAR, BACKGROUND]);
}

#[test]
fn on_a_desktop_the_edges_do_nothing_and_the_panels_are_always_there() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &panel(Anchor::Top), (640, 20), GREEN);
    session.open_decorated(&mut client, "app", (200, 100), RED);

    point(&mut session, 600, 0);
    assert_eq!(session.compositor().deadline(), None);
    session.advance(REST);
    let picture = session.draw();
    assert_eq!(picture.at(600, 10), GREEN);
    assert_eq!(picture.at(600, 34), BACKGROUND, "no titlebar over anything");
}

#[test]
fn leaving_fullscreen_puts_the_panels_back_where_they_are() {
    let (mut session, _client) = fullscreen();
    point(&mut session, 600, 0);
    session.advance(REST);

    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(600, 10), GREEN);
    assert_eq!(picture.at(600, 470), BLUE);
}
