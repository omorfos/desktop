//! The rounded bottom corners of a decorated frame.

use omorfos::Scale;

use crate::common::Session;
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

/// The default theme's frame around a window 200 by 100: a border of one,
/// a titlebar of 28 and corners rounded to a radius of eight.
const RIGHT: i32 = 201;
const BOTTOM: i32 = 128;

#[test]
fn the_bottom_corners_of_a_frame_are_rounded_window_and_all() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let picture = session.draw();

    // Outside the curve there is nothing of the frame or the window.
    for (x, y) in [
        (0, BOTTOM),
        (1, BOTTOM - 1),
        (RIGHT, BOTTOM),
        (RIGHT - 1, BOTTOM - 1),
    ] {
        assert_eq!(picture.at(x, y), BACKGROUND, "at {x}, {y}");
    }
    // The border runs straight up to the curve and follows it round.
    let border = picture.at(0, 60);
    assert_ne!(border, RED);
    assert_ne!(border, BACKGROUND);
    assert_eq!(picture.at(100, BOTTOM), border, "along the bottom");
    assert_eq!(picture.at(RIGHT, 60), border, "down the right");
    assert_eq!(
        picture.at(0, BOTTOM - 8),
        border,
        "to where the curve begins"
    );
    assert_eq!(picture.at(8, BOTTOM), border);
    for (x, y) in [(2, BOTTOM - 2), (RIGHT - 2, BOTTOM - 2)] {
        let on_curve = picture.at(x, y);
        assert_ne!(on_curve, RED, "at {x}, {y}");
        assert_ne!(on_curve, BACKGROUND, "at {x}, {y}");
    }
    // Inside it the window is whole.
    assert_eq!(picture.at(10, BOTTOM - 8), RED);
    assert_eq!(picture.at(4, BOTTOM - 4), RED, "just inside the curve");
    assert_eq!(picture.at(RIGHT - 4, BOTTOM - 4), RED);
    assert_eq!(picture.at(100, BOTTOM - 1), RED, "its last row");
    assert_eq!(picture.at(1, 60), RED, "its first column");
}

#[test]
fn what_is_behind_a_frame_shows_past_its_corners() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "behind", (400, 300), BLUE);
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let frame = session.compositor().desktop().frames().last().unwrap();
    let (left, bottom) = (frame.geometry.origin.x, frame.geometry.bottom() - 1);
    let bottom = i32::try_from(bottom).unwrap();

    let picture = session.draw();
    assert_eq!(picture.at(left, bottom), BLUE, "the corner");
    assert_eq!(picture.at(left + 10, bottom - 8), RED, "the window");
}

#[test]
fn a_radius_of_nothing_leaves_the_corners_square() {
    let mut session = Session::with_config(&[("theme.yaml", "border: { radius: 0 }\n")]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let picture = session.draw();
    let border = picture.at(0, 60);
    assert_eq!(
        picture.at(0, BOTTOM),
        border,
        "the border reaches the corner"
    );
    assert_eq!(picture.at(RIGHT, BOTTOM), border);
    assert_eq!(picture.at(1, BOTTOM - 1), RED, "and so does the window");
    assert_eq!(picture.at(RIGHT - 1, BOTTOM - 1), RED);
}

#[test]
fn a_frame_that_has_not_changed_is_not_drawn_again() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "app", (200, 100), RED);
    session.draw();
    assert!(!session.draw().damaged, "nothing changed");
    assert!(!session.draw().damaged, "and still nothing");

    // What the window draws next does reach the screen, corners and all.
    client.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut client);
    let picture = session.draw();
    assert!(picture.damaged);
    assert_eq!(picture.at(100, 60), GREEN);
    assert_eq!(
        picture.at(4, BOTTOM - 4),
        GREEN,
        "in the strips by the curve"
    );
    assert_eq!(picture.at(1, BOTTOM - 1), BACKGROUND);
}

#[test]
fn corners_are_rounded_at_the_scale_of_the_screen() {
    let mut session = Session::new();
    session.suggest_scale(Scale::new(2.0));
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let picture = session.draw();
    // Twice the size in pixels: the frame is 404 by 258 and the radius 16.
    let (right, bottom) = (403, 257);
    for (x, y) in [(0, bottom), (3, bottom - 3), (right - 3, bottom - 3)] {
        assert_eq!(picture.at(x, y), BACKGROUND, "at {x}, {y}");
    }
    assert_eq!(picture.at(8, bottom - 8), RED, "inside the curve");
    assert_eq!(picture.at(right - 8, bottom - 8), RED);
    assert_eq!(picture.at(200, bottom - 2), RED, "the last row");
}

#[test]
fn a_pop_up_is_not_cut_to_the_frame() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "app", (200, 100), RED);
    // A menu hanging off the bottom corner of the window.
    let (surface, _popup, _positioner) = client.popup(&window, (-10, 90), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let picture = session.draw();
    assert_eq!(picture.at(20, BOTTOM - 4), GREEN, "over the window");
    assert_eq!(picture.at(5, BOTTOM + 10), GREEN, "and past the frame");
}
