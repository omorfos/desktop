//! Telling programs when what they drew reached the screen
//! (`presentation-time`).

use std::time::{Duration, SystemTime};

use omorfos_config::{Action, WorkspaceNumber};
use smithay::backend::drm::{DrmEventMetadata, DrmEventTime};
use smithay::utils::{Clock, Monotonic};
use wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::Kind;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Event, LayerShape, Session, WIDTH};
use crate::windows::{BLUE, RED};

/// What a graphics card says of a frame it showed: at five seconds by the
/// monotonic clock, in the screen's 42nd refresh.
const CARD: DrmEventMetadata = DrmEventMetadata {
    time: DrmEventTime::Monotonic(Duration::from_secs(5)),
    sequence: 42,
};

/// A screen that refreshes 60 times a second.
const REFRESH: Duration = Duration::from_nanos(16_666_666);

/// What the client was told of how what it drew was shown.
fn told(events: Vec<Event>) -> Vec<Event> {
    let told = |event: &Event| matches!(event, Event::Presented { .. } | Event::Discarded);
    events.into_iter().filter(told).collect()
}

#[test]
fn a_program_is_told_when_what_it_drew_reached_the_screen_by_the_cards_clock() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "player", (200, 100), RED);
    assert_eq!(client.state.clock, Some(1), "the monotonic clock is named");

    let _feedback = client.feedback(&window.surface);
    client.fill(&window.surface, (200, 100), BLUE);
    session.roundtrip(&mut client);
    session.draw();
    session.roundtrip(&mut client);
    assert_eq!(told(client.take_events()), [], "nothing before it is shown");
    session.present(Some(&CARD), Some(REFRESH));
    session.roundtrip(&mut client);

    let shown = Event::Presented {
        at: Duration::from_secs(5),
        refresh: REFRESH,
        sequence: 42,
        kind: Kind::Vsync | Kind::HwClock | Kind::HwCompletion,
    };
    assert_eq!(told(client.take_events()), [shown]);
}

#[test]
fn without_the_cards_own_time_what_was_drawn_is_said_to_be_shown_when_the_compositor_hears_of_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "player", (200, 100), RED);
    let clock = Clock::<Monotonic>::new();
    // A card that gives the time by the wall clock, which programs were not
    // promised, and a nested compositor, whose host says nothing.
    let wall = DrmEventMetadata {
        time: DrmEventTime::Realtime(SystemTime::UNIX_EPOCH),
        sequence: 7,
    };
    for card in [Some(&wall), None] {
        let _feedback = client.feedback(&window.surface);
        client.fill(&window.surface, (200, 100), BLUE);
        session.roundtrip(&mut client);
        session.draw();
        let before = Duration::from(clock.now());
        session.present(card, None);
        let after = Duration::from(clock.now());
        session.roundtrip(&mut client);

        let told = told(client.take_events());
        let [
            Event::Presented {
                at,
                refresh,
                sequence,
                kind,
            },
        ] = told[..]
        else {
            panic!("told {told:?}");
        };
        assert!((before..=after).contains(&at), "{at:?} is when it was told");
        assert_eq!(
            (refresh, sequence, kind),
            (Duration::ZERO, 0, Kind::empty())
        );
    }
}

#[test]
fn a_window_not_on_the_screen_is_told_of_the_first_frame_it_is_in() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    let window = session.open(&mut client, "player", (200, 100), RED);
    let workspace = |number| Action::Workspace(WorkspaceNumber::clamped(number));
    session.compositor_mut().perform(&workspace(2));
    session.step();

    let _feedback = client.feedback(&window.surface);
    client.fill(&window.surface, (200, 100), BLUE);
    session.roundtrip(&mut client);
    session.draw();
    session.present(Some(&CARD), Some(REFRESH));
    session.roundtrip(&mut client);
    assert_eq!(told(client.take_events()), [], "not while it is elsewhere");

    session.compositor_mut().perform(&workspace(1));
    session.step();
    session.draw();
    session.present(Some(&CARD), Some(REFRESH));
    session.roundtrip(&mut client);
    assert!(matches!(
        told(client.take_events())[..],
        [Event::Presented { sequence: 42, .. }]
    ));
}

#[test]
fn what_was_drawn_in_a_frame_that_is_never_shown_is_said_to_be_discarded() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "player", (200, 100), RED);

    let _feedback = client.feedback(&window.surface);
    client.fill(&window.surface, (200, 100), BLUE);
    session.roundtrip(&mut client);
    // The frame it was in is dropped for the next, as a card that could not
    // show it would.
    session.draw();
    session.draw();
    session.roundtrip(&mut client);

    assert_eq!(told(client.take_events()), [Event::Discarded]);
}

#[test]
fn a_panel_is_told_when_what_it_drew_reached_the_screen() {
    let mut session = Session::new();
    let mut client = session.connect();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let (surface, _role) = client.layer(&panel);
    session.roundtrip(&mut client);

    let _feedback = client.feedback(&surface);
    client.fill(&surface, (WIDTH, 30), BLUE);
    session.roundtrip(&mut client);
    session.draw();
    session.present(Some(&CARD), Some(REFRESH));
    session.roundtrip(&mut client);

    assert!(matches!(
        told(client.take_events())[..],
        [Event::Presented { sequence: 42, .. }]
    ));
}
