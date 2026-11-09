//! "About omorfos": shown in the middle of the screen when asked for, over
//! everything, and put away by Escape, a click beside it, or asking again.

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use smithay::input::keyboard::Keycode;

use crate::common::{Event, HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, GREEN};

const LEFT_BUTTON: u32 = 0x110;
/// What "About" is drawn on in the dark scheme: its glass, the surface at
/// 90% over the background; and the same over its own shadow, where it is
/// not blurred.
const SURFACE: u32 = 0xff13_1b37;
const UNBLURRED: u32 = 0xff13_1a36;

/// Where "About" has nothing drawn on it but its ground: a little inside
/// its left edge, half way down, the screen being 640 wide and it as wide
/// less 12 each side.
const GROUND: (i32, i32) = (12 + 8, HEIGHT / 2);

/// A session whose "About" is not blurred, which in the software
/// renderer takes seconds a picture: over the plain background it is the
/// same colour.
fn unblurred() -> Session {
    Session::with_config(&[("theme.css", "about { backdrop-filter: none; }\n")])
}

fn about(session: &mut Session) {
    session.compositor_mut().perform(&Action::About);
    session.settle();
}

fn tap(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.settle();
}

fn click(session: &mut Session, (x, y): (i32, i32)) {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    for pressed in [true, false] {
        let input = Input::Button {
            button: LEFT_BUTTON,
            pressed,
        };
        session.compositor_mut().input(input, 0);
    }
    session.settle();
}

/// The rows of the screen down the middle of "About" with something drawn
/// on its ground: its text, and the mark.
fn inked(session: &mut Session) -> usize {
    let screen = session.draw();
    let (left, right) = (GROUND.0 + 12, WIDTH - GROUND.0 - 12);
    (0..HEIGHT)
        .filter(|&y| (left..right).any(|x| ![SURFACE, BACKGROUND].contains(&screen.at(x, y))))
        .count()
}

#[test]
fn about_is_shown_in_the_middle_with_what_it_says_and_escape_puts_it_away() {
    let mut session = Session::new();
    assert_eq!(session.draw().at(GROUND.0, GROUND.1), BACKGROUND);
    about(&mut session);
    let screen = session.draw();
    assert_eq!(screen.at(GROUND.0, GROUND.1), SURFACE, "its ground");
    assert_ne!(screen.at(4, 4), SURFACE, "and not the whole screen");
    // The mark and some twenty lines of words, each some rows of ink.
    let rows = inked(&mut session);
    assert!(rows > 150, "{rows}");
    tap(&mut session, key::ESCAPE);
    assert_eq!(
        session.draw().at(GROUND.0, GROUND.1),
        BACKGROUND,
        "put away"
    );
}

#[test]
fn a_click_beside_about_puts_it_away_and_reaches_no_window_and_one_on_it_does_nothing() {
    let mut session = unblurred();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), GREEN);
    about(&mut session);
    click(&mut session, GROUND);
    assert_eq!(
        session.draw().at(GROUND.0, GROUND.1),
        UNBLURRED,
        "still shown"
    );
    session.roundtrip(&mut client);
    client.take_events();
    // On the window, which is at the top left, and beside "About".
    click(&mut session, (5, 60));
    session.roundtrip(&mut client);
    let events = client.take_events();
    let pressed = events
        .iter()
        .any(|event| matches!(event, Event::PointerButton { .. }));
    assert!(!pressed, "the window under the click is not pressed");
    assert_eq!(
        session.draw().at(GROUND.0, GROUND.1),
        BACKGROUND,
        "put away"
    );
    drop(window);
}

#[test]
fn about_asked_for_again_goes_and_is_painted_again_as_the_scheme_changes() {
    let mut session = unblurred();
    about(&mut session);
    session.write_config("config.yaml", "appearance: { scheme: light }\n");
    session.compositor_mut().reload_config();
    session.settle();
    // Cream, glass over cream.
    let light = |pixel: u32| {
        pixel.to_be_bytes()[1..]
            .iter()
            .all(|&channel| channel > 0xe0)
    };
    let ground = session.draw().at(GROUND.0, GROUND.1);
    assert!(light(ground), "{ground:#x}");
    let shown = ground;
    about(&mut session);
    let ground = session.draw().at(GROUND.0, GROUND.1);
    assert!(light(ground) && ground != shown, "gone: {ground:#x}");
}

#[test]
fn about_fades_out_as_it_goes_and_a_theme_may_give_it_no_shadow() {
    let theme = "about { backdrop-filter: none; }\n\
                 about:closing { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    about(&mut session);
    for pressed in [true, false] {
        let escape = Input::Key {
            code: key::ESCAPE,
            pressed,
        };
        session.compositor_mut().input(escape, 0);
    }
    // It begins to go, and is half way through its time to go.
    session.step();
    session.advance(Duration::from_millis(50));
    let fading = session.draw().at(GROUND.0, GROUND.1);
    assert!(
        ![BACKGROUND, UNBLURRED].contains(&fading),
        "fading: {fading:#x}"
    );
    session.settle();
    assert_eq!(session.draw().at(GROUND.0, GROUND.1), BACKGROUND);

    let theme = "about { backdrop-filter: none; box-shadow: none; }\n";
    session.write_config("theme.css", theme);
    session.compositor_mut().reload_config();
    about(&mut session);
    // Its shadow under it no longer shows through its glass.
    assert_eq!(session.draw().at(GROUND.0, GROUND.1), SURFACE);
}

/// The middle of the button that closes "About": what is in the colour of
/// danger at its top right, as a window's button is with focus.
fn close_button(session: &mut Session) -> (i32, i32) {
    const DANGER: u32 = 0xfff8_616d;
    let screen = session.draw();
    let red: Vec<(i32, i32)> = (0..HEIGHT / 2)
        .flat_map(|y| (WIDTH / 2..WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| screen.at(x, y) == DANGER)
        .collect();
    // Nothing found is the corner, which the test then finds wanting.
    let across = red.iter().map(|&(x, _)| x);
    let down = red.iter().map(|&(_, y)| y);
    let middle = |from: Option<i32>, to: Option<i32>| {
        i32::midpoint(from.unwrap_or_default(), to.unwrap_or_default())
    };
    (
        middle(across.clone().min(), across.max()),
        middle(down.clone().min(), down.max()),
    )
}

#[test]
fn the_button_that_closes_about_shows_its_cross_under_the_pointer_and_closes_it() {
    let mut session = unblurred();
    about(&mut session);
    let (x, y) = close_button(&mut session);
    let red = session.draw().at(x, y);
    assert_ne!(red, UNBLURRED, "the button: {red:#x}");
    // Under the pointer, its cross is drawn in the middle of it.
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    session.settle();
    let crossed = session.draw().at(x, y);
    assert_ne!(crossed, red, "its cross: {crossed:#x}");
    click(&mut session, (x, y));
    assert_eq!(session.draw().at(GROUND.0, GROUND.1), BACKGROUND, "closed");
}
