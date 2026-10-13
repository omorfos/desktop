//! The arrangements offered under the second button of a titlebar.

use std::time::Duration;

use omorfos::Input;
use omorfos_core::Button;

use crate::common::{Client, Session, key};
use crate::windows::{BACKGROUND, RED};

const LEFT: u32 = 0x110;
/// Colours of the built-in theme: a menu's ground, a part of a picture,
/// and one the pointer is on.
const GROUND: u32 = 0xff23_262e;
const BLOCK: u32 = 0xff2c_303a;
const PICKED: u32 = 0xff5b_9cf5;
/// How long the pointer rests on the button before the pictures come.
const REST: Duration = Duration::from_millis(400);

/// A session with one window, 200 by 100 with a titlebar, and where the
/// pictures of arrangements go: the corner under its second button.
fn window() -> (Session, Client, (i32, i32)) {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let frame = session.compositor().desktop().frames().next();
    let button = frame.and_then(|frame| frame.decoration);
    let button = button.map(|decoration| decoration.button(Button::Maximize));
    let button = button.unwrap_or_default();
    let corner = (
        button.origin.x,
        button.origin.y + button.size.height.cast_signed(),
    );
    (session, client, corner)
}

fn point(session: &mut Session, (x, y): (i32, i32)) {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    session.step();
}

fn press(session: &mut Session) {
    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    session.step();
}

/// Rests the pointer on the second button of the titlebar until the
/// pictures are up.
fn rest(session: &mut Session, corner: (i32, i32)) {
    point(session, (corner.0 + 4, corner.1 - 10));
    session.advance(REST);
}

/// A point on a part of the first picture, two side by side: the left
/// part, or the right.
fn half(corner: (i32, i32), right: bool) -> (i32, i32) {
    (corner.0 + 8 + 5 + 22 * i32::from(right), corner.1 + 8 + 5)
}

fn places(session: &Session) -> usize {
    session.compositor().desktop().workspaces().count()
}

#[test]
fn resting_on_the_second_button_brings_up_pictures_of_the_arrangements() {
    let (mut session, _client, corner) = window();
    // Far along the pictures, past the window, where nothing else is.
    let far = (corner.0 + 330, corner.1 + 3);
    assert_eq!(session.draw().at(far.0, far.1), BACKGROUND);

    point(&mut session, (corner.0 + 4, corner.1 - 10));
    assert_eq!(session.draw().at(far.0, far.1), BACKGROUND, "not at once");
    let due = session.compositor().deadline();
    assert_eq!(due, Some(session.now + REST));
    session.advance(REST);
    let picture = session.draw();
    assert_eq!(picture.at(far.0, far.1), GROUND);
    let part = half(corner, false);
    assert_eq!(picture.at(part.0, part.1), BLOCK);
    assert_eq!(session.compositor().deadline(), None);
    assert_eq!(places(&session), 1, "and nothing has been done yet");

    // The part under the pointer is picked out, and only that one.
    point(&mut session, part);
    let picture = session.draw();
    assert_eq!(picture.at(part.0, part.1), PICKED);
    let other = half(corner, true);
    assert_eq!(picture.at(other.0, other.1), BLOCK);
    session.step();
    assert!(
        !session.draw().damaged,
        "and they are not painted again as they are"
    );
}

#[test]
fn a_press_on_a_part_of_a_picture_puts_the_window_there() {
    let (mut session, _client, corner) = window();
    rest(&mut session, corner);

    point(&mut session, half(corner, true));
    press(&mut session);
    let desktop = session.compositor().desktop();
    assert_eq!(desktop.active_workspace(), 1);
    let part = desktop.frames().next().map(|frame| frame.geometry);
    let part = part.map(|at| (at.origin.x, at.size.width, at.size.height));
    assert_eq!(
        part,
        Some((320, 320, 480)),
        "the right half of a new workspace"
    );
    // Where they were, on the left, there is nothing now.
    let near = (corner.0 + 100, corner.1 + 3);
    assert_eq!(session.draw().at(near.0, near.1), BACKGROUND);
}

#[test]
fn a_press_between_the_pictures_takes_them_away_and_does_nothing_else() {
    let (mut session, mut client, corner) = window();
    rest(&mut session, corner);
    client.take_events();

    point(&mut session, (corner.0 + 2, corner.1 + 2));
    press(&mut session);
    assert_eq!(places(&session), 1);
    let far = (corner.0 + 330, corner.1 + 3);
    assert_eq!(session.draw().at(far.0, far.1), BACKGROUND);

    // And the window under them was told of none of it.
    session.roundtrip(&mut client);
    let events = client.take_events();
    let pressed =
        |event: &crate::common::Event| matches!(event, crate::common::Event::PointerButton { .. });
    assert!(!events.iter().any(pressed), "{events:?}");
}

#[test]
fn the_pictures_go_when_the_pointer_leaves_them_or_a_key_is_pressed() {
    let (mut session, _client, corner) = window();
    let far = (corner.0 + 330, corner.1 + 3);
    rest(&mut session, corner);

    // A little outside them is still on its way to them.
    point(&mut session, (corner.0 + 100, corner.1 + 50));
    assert_eq!(session.draw().at(far.0, far.1), GROUND);
    point(&mut session, (corner.0 + 100, corner.1 + 200));
    assert_eq!(session.draw().at(far.0, far.1), BACKGROUND);

    rest(&mut session, corner);
    assert_eq!(session.draw().at(far.0, far.1), GROUND);
    for pressed in [true, false] {
        let code = key::A;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    assert_eq!(session.draw().at(far.0, far.1), BACKGROUND);
}

#[test]
fn a_click_on_the_button_is_still_for_fullscreen() {
    let (mut session, _client, corner) = window();
    rest(&mut session, corner);
    press(&mut session);

    let frame = session.compositor().desktop().frames().next();
    assert_eq!(frame.map(|frame| frame.fullscreen), Some(true));
    let far = (corner.0 + 330, corner.1 + 3);
    assert_eq!(
        session.draw().at(far.0, far.1),
        BACKGROUND,
        "and they are gone"
    );
}

#[test]
fn the_pictures_go_with_the_window_they_are_of() {
    let (mut session, client, corner) = window();
    let far = (corner.0 + 330, corner.1 + 3);
    rest(&mut session, corner);
    assert_eq!(session.draw().at(far.0, far.1), GROUND);

    drop(client);
    session.eventually(|session| session.draw().at(far.0, far.1) == BACKGROUND);
}

#[test]
fn a_window_that_fills_a_workspace_already_is_offered_none() {
    let (mut session, _client, _corner) = window();
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Fullscreen);
    // Its titlebar is brought back at the top of the screen, and the
    // pointer rests on the second button of that.
    point(&mut session, (36, 0));
    session.advance(Duration::from_millis(300));
    point(&mut session, (36, 14));
    let before = session.draw().at(360, 50);
    session.advance(Duration::from_secs(2));
    assert_eq!(session.draw().at(360, 50), before);
    assert_eq!(places(&session), 2);
}

#[test]
fn a_button_let_go_on_the_pictures_chooses_nothing() {
    let (mut session, _client, corner) = window();
    // Held down on the button until the pictures come, then carried onto
    // them and let go.
    point(&mut session, (corner.0 + 4, corner.1 - 10));
    let hold = |pressed: bool| Input::Button {
        button: LEFT,
        pressed,
    };
    session.compositor_mut().input(hold(true), 0);
    session.advance(REST);
    point(&mut session, half(corner, false));
    session.compositor_mut().input(hold(false), 0);
    session.step();

    assert_eq!(places(&session), 1, "neither an arrangement nor fullscreen");
    let far = (corner.0 + 330, corner.1 + 3);
    assert_eq!(
        session.draw().at(far.0, far.1),
        GROUND,
        "and they are still up"
    );
}

/// Whether anything is written in the line of words under the pictures:
/// ink that is not the ground, wherever in the line the font puts it.
fn written(session: &mut Session, corner: (i32, i32), ground: u32) -> Vec<bool> {
    let picture = session.draw();
    let columns = corner.0 + 8..corner.0 + 364;
    columns
        .map(|x| (corner.1 + 48..corner.1 + 64).any(|y| picture.at(x, y) != ground))
        .collect()
}

#[test]
fn the_pictures_say_in_words_what_the_one_under_the_pointer_is() {
    let (mut session, _client, corner) = window();
    rest(&mut session, corner);
    let asked = written(&mut session, corner, GROUND);
    assert!(asked.contains(&true), "what they are for");

    // On a part, the words are of that arrangement and that part, which
    // is something else again for another.
    point(&mut session, half(corner, false));
    let left = written(&mut session, corner, GROUND);
    assert!(left.contains(&true));
    assert_ne!(left, asked);
    let quarter = (corner.0 + 8 + 6 * 52 + 5, corner.1 + 8 + 5);
    point(&mut session, quarter);
    assert_ne!(written(&mut session, corner, GROUND), left);
}

#[test]
fn an_arrow_points_from_the_pictures_at_the_button_they_belong_to() {
    let theme = "menu { background: #112233; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let frame = session.compositor().desktop().frames().next();
    let button = frame.and_then(|frame| frame.decoration);
    let button = button.map(|decoration| decoration.button(Button::Maximize));
    let button = button.unwrap_or_default();
    let middle = button.origin.x + (button.size.width / 2).cast_signed();
    let foot = button.origin.y + button.size.height.cast_signed() - 1;
    let before = session.draw().at(middle, foot);

    point(&mut session, (button.origin.x + 4, foot - 9));
    let above = session.draw().at(middle, foot - 8);
    session.advance(REST);
    let picture = session.draw();
    assert_eq!(
        picture.at(middle, foot),
        0xff11_2233,
        "up into the titlebar"
    );
    assert_eq!(picture.at(middle, foot - 4), 0xff11_2233);
    assert_eq!(
        picture.at(middle + 40, foot),
        before,
        "and only under the button"
    );
    assert_eq!(picture.at(middle, foot - 8), above, "no higher than it is");
}
