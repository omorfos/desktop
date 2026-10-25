//! Programs that hold the pointer (`pointer-constraints`), and the
//! distances it moves, told as they are (`relative-pointer`).

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

/// Puts the pointer at a place on the screen, as a tablet would.
fn point(session: &mut Session, client: &mut Client, x: f64, y: f64) {
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.roundtrip(client);
    client.take_events();
}

/// Moves the pointer by a distance, as a mouse does, and returns what the
/// client was told of it.
fn nudge(session: &mut Session, client: &mut Client, dx: f64, dy: f64) -> Vec<Event> {
    let moved = Input::MotionBy {
        dx,
        dy,
        raw_dx: dx,
        raw_dy: dy,
    };
    session.compositor_mut().input(moved, 0);
    session.roundtrip(client);
    client.take_events()
}

/// Where the client was last told the pointer is on its surface.
fn pointed(events: &[Event]) -> Option<(f64, f64)> {
    events.iter().rev().find_map(|event| match event {
        Event::PointerMotion { x, y } => Some((*x, *y)),
        _ => None,
    })
}

#[test]
fn a_program_is_told_how_far_the_pointer_moved_as_the_device_measured_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "game", (200, 100), RED);
    let _relative = client.relative_pointer();
    point(&mut session, &mut client, 50.0, 50.0);

    let moved = Input::MotionBy {
        dx: 3.0,
        dy: 4.0,
        raw_dx: 1.5,
        raw_dy: 2.0,
    };
    session.compositor_mut().input(moved, 7);
    session.roundtrip(&mut client);

    let events = client.take_events();
    let relative = Event::RelativeMotion {
        by: (3.0, 4.0),
        raw: (1.5, 2.0),
        utime: 7000,
    };
    assert!(events.contains(&relative), "{events:?}");
    assert_eq!(pointed(&events), Some((53.0, 54.0)), "and it moved");
}

#[test]
fn a_program_with_the_keyboard_holds_the_pointer_where_it_is_until_it_lets_go() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    let _relative = client.relative_pointer();
    point(&mut session, &mut client, 50.0, 50.0);

    let lock = client.lock_pointer(&game.surface, None);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::PointerLocked]);

    let events = nudge(&mut session, &mut client, 10.0, 10.0);
    let moved = |event: &Event| {
        matches!(
            event,
            Event::RelativeMotion {
                by: (10.0, 10.0),
                ..
            }
        )
    };
    assert!(events.iter().any(moved), "it is told how far: {events:?}");
    assert_eq!(pointed(&events), None, "and the pointer stays");

    lock.destroy();
    session.roundtrip(&mut client);
    let events = nudge(&mut session, &mut client, 10.0, 10.0);
    assert_eq!(pointed(&events), Some((60.0, 60.0)), "let go of, it moves");
}

#[test]
fn a_window_that_loses_the_keyboard_lets_go_of_the_pointer_until_it_has_it_back() {
    let mut session = Session::new();
    let mut client = session.connect();
    // The second cascades over the first, leaving its corner bare, and has
    // the keyboard.
    let game = session.open(&mut client, "game", (200, 100), RED);
    session.open(&mut client, "other", (200, 100), BLUE);
    point(&mut session, &mut client, 5.0, 5.0);

    let _lock = client.lock_pointer(&game.surface, None);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [], "not without the keyboard");

    session.compositor_mut().perform(&Action::FocusNext);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::PointerLocked));
    assert_eq!(pointed(&nudge(&mut session, &mut client, 1.0, 1.0)), None);

    session.compositor_mut().perform(&Action::FocusNext);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::PointerUnlocked));
    let events = nudge(&mut session, &mut client, 1.0, 1.0);
    assert_eq!(pointed(&events), Some((6.0, 6.0)), "the user has it back");
}

#[test]
fn a_program_holds_the_pointer_only_over_the_part_it_asked_for() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    point(&mut session, &mut client, 50.0, 50.0);

    let right = client.region((100, 0, 100, 100));
    let _lock = client.lock_pointer(&game.surface, Some(&right));
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [], "the pointer is elsewhere");

    let reached = nudge(&mut session, &mut client, 60.0, 0.0);
    assert!(
        reached.contains(&Event::PointerLocked),
        "once it is: {reached:?}"
    );
    let events = nudge(&mut session, &mut client, 5.0, 0.0);
    assert_eq!(pointed(&events), None, "held, once over it");
}

#[test]
fn a_program_keeps_the_pointer_within_the_part_it_asked_for_sliding_along_its_edges() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    point(&mut session, &mut client, 50.0, 50.0);

    let middle = client.region((20, 20, 60, 60));
    let _confined = client.confine_pointer(&game.surface, Some(&middle));
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::PointerConfined]);

    let along = nudge(&mut session, &mut client, 100.0, 5.0);
    assert_eq!(pointed(&along), Some((50.0, 55.0)), "down the edge it met");
    let back = nudge(&mut session, &mut client, -10.0, -10.0);
    assert_eq!(pointed(&back), Some((40.0, 45.0)), "and freely within");
    let corner = nudge(&mut session, &mut client, -100.0, -100.0);
    assert_eq!(
        pointed(&corner),
        Some((40.0, 45.0)),
        "nowhere past a corner"
    );
}

#[test]
fn a_program_that_asks_for_no_part_keeps_the_pointer_within_its_surface() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    point(&mut session, &mut client, 190.0, 50.0);

    let _confined = client.confine_pointer(&game.surface, None);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::PointerConfined]);

    let events = nudge(&mut session, &mut client, 20.0, 0.0);
    assert_eq!(pointed(&events), Some((190.0, 50.0)), "not off its edge");
}

#[test]
fn a_program_that_held_the_pointer_says_where_it_is_left_once_let_go() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    point(&mut session, &mut client, 50.0, 50.0);

    let lock = client.lock_pointer(&game.surface, None);
    session.roundtrip(&mut client);
    lock.set_cursor_position_hint(20.0, 30.0);
    game.surface.commit();
    session.roundtrip(&mut client);
    lock.destroy();
    session.roundtrip(&mut client);
    client.take_events();

    let events = nudge(&mut session, &mut client, 1.0, 1.0);
    assert_eq!(pointed(&events), Some((21.0, 31.0)));
}

#[test]
fn where_a_program_not_holding_the_pointer_says_it_is_changes_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    let game = session.open(&mut client, "game", (200, 100), RED);
    point(&mut session, &mut client, 50.0, 50.0);

    let right = client.region((100, 0, 100, 100));
    let lock = client.lock_pointer(&game.surface, Some(&right));
    lock.set_cursor_position_hint(150.0, 30.0);
    game.surface.commit();
    session.roundtrip(&mut client);
    lock.destroy();
    session.roundtrip(&mut client);
    client.take_events();

    let events = nudge(&mut session, &mut client, 1.0, 1.0);
    assert_eq!(pointed(&events), Some((51.0, 51.0)));
}

#[test]
fn a_program_holding_the_pointer_against_the_top_brings_nothing_back() {
    let mut session = Session::new();
    let mut client = session.connect();
    let top = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 20,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &top, (640, 20), GREEN);
    let game = session.open(&mut client, "game", (200, 100), RED);
    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    let rest = Duration::from_secs(1);
    point(&mut session, &mut client, 100.0, 0.0);

    let confined = client.confine_pointer(&game.surface, None);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::PointerConfined]);
    session.advance(rest);
    assert_eq!(
        session.draw().at(600, 10),
        BACKGROUND,
        "held, it is the game's"
    );

    confined.destroy();
    session.roundtrip(&mut client);
    point(&mut session, &mut client, 101.0, 0.0);
    session.advance(rest);
    assert_eq!(
        session.draw().at(600, 10),
        GREEN,
        "let go of, it is the user's"
    );
}
