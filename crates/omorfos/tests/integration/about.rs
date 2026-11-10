//! "About omorfos": shown in the middle of the screen when asked for, over
//! everything, and put away by Escape, a click beside it, or asking again;
//! no taller than its screen, what it lists scrolled by the wheel and the
//! keys, which no window sees while it is shown; what the computer is,
//! which its button copies for any program to paste.

use std::io::Read;
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

/// A session whose screen is half as large in units of layout, its scale
/// being 2: too short for all that "About" lists. Its "About" has neither
/// blur nor shadow, so that its ground is one colour all over.
fn short(more: &[(&str, &str)]) -> Session {
    let theme = "about { backdrop-filter: none; box-shadow: none; }\n";
    let mut files = vec![
        ("theme.css", theme),
        ("config.yaml", "output: { scale: 2 }\n"),
    ];
    files.extend_from_slice(more);
    Session::with_config(&files)
}

fn scroll(session: &mut Session, by: f64) {
    let input = Input::Scroll {
        horizontal: 0.0,
        vertical: by,
    };
    session.compositor_mut().input(input, 0);
    session.settle();
}

/// Each row of the screen, from the top.
fn seen(session: &mut Session) -> Vec<Vec<u32>> {
    let screen = session.draw();
    let row = |y| (0..WIDTH).map(|x| screen.at(x, y)).collect();
    (0..HEIGHT).map(row).collect()
}

/// Where "About" on a short screen has nothing drawn on its ground, in
/// pixels: inside its left edge, which is 12 units in from the screen's.
const LEFT_SIDE: i32 = 40;

/// The rows of "About" that do not scroll, in pixels: from its top, 12
/// units down, past the mark, 20 units below that.
const HEAD: std::ops::Range<usize> = 24..176;

#[test]
fn about_on_a_screen_too_short_for_it_keeps_inside_it_and_scrolls_what_it_lists() {
    let mut session = short(&[]);
    about(&mut session);
    let screen = session.draw();
    // From 12 units below the top of the screen to 12 above its foot.
    let side = |y| screen.at(LEFT_SIDE, y);
    assert_eq!(side(20), BACKGROUND, "above it");
    assert_eq!(side(28), SURFACE, "its top");
    assert_eq!(side(HEIGHT - 30), SURFACE, "its foot");
    assert_eq!(side(HEIGHT - 20), BACKGROUND, "below it");

    let top = seen(&mut session);
    scroll(&mut session, 15.0);
    let on = seen(&mut session);
    assert_ne!(on, top, "what it lists moves up");
    assert_eq!(on[HEAD], top[HEAD], "and the mark stays where it is");
    assert_eq!(on[..24], top[..24], "as does what is above it");

    scroll(&mut session, 100_000.0);
    let end = seen(&mut session);
    assert_ne!(end, on);
    scroll(&mut session, 15.0);
    assert_eq!(seen(&mut session), end, "no further than its end");
    scroll(&mut session, -100_000.0);
    assert_eq!(seen(&mut session), top, "nor back past its top");
}

#[test]
fn about_has_the_keyboard_its_keys_scroll_it_and_a_window_sees_none_but_bindings_work() {
    let keys = (
        "keys.yaml",
        "bindings:\n  - { keys: Super+F2, action: about }\n",
    );
    let mut session = short(&[keys]);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), GREEN);
    session.roundtrip(&mut client);
    client.take_events();
    about(&mut session);
    let top = seen(&mut session);

    tap(&mut session, key::A);
    assert_eq!(seen(&mut session), top, "a letter does nothing");
    tap(&mut session, key::DOWN);
    let down = seen(&mut session);
    assert_ne!(down, top, "an arrow scrolls a line");
    tap(&mut session, key::PAGE_DOWN);
    let page = seen(&mut session);
    assert!(page != down && page != top, "a page further");
    tap(&mut session, key::END);
    let end = seen(&mut session);
    scroll(&mut session, 15.0);
    assert_eq!(seen(&mut session), end, "the end");
    tap(&mut session, key::UP);
    assert_ne!(seen(&mut session), end);
    tap(&mut session, key::SPACE);
    assert_eq!(seen(&mut session), end, "space is a page down too");
    tap(&mut session, key::PAGE_UP);
    assert_ne!(seen(&mut session), end, "a page back");
    tap(&mut session, key::HOME);
    assert_eq!(seen(&mut session), top, "and home its top");

    session.roundtrip(&mut client);
    let typed = |events: Vec<Event>| {
        events
            .iter()
            .any(|event| matches!(event, Event::Key { .. }))
    };
    assert!(!typed(client.take_events()), "nothing reached the window");

    // A binding still does what it is bound to: here, puts "About" away.
    for pressed in [true, false] {
        let codes = if pressed {
            [key::SUPER, key::F2]
        } else {
            [key::F2, key::SUPER]
        };
        for code in codes {
            session
                .compositor_mut()
                .input(Input::Key { code, pressed }, 0);
        }
    }
    session.settle();
    let screen = session.draw();
    assert_eq!(screen.at(LEFT_SIDE, HEIGHT - 30), BACKGROUND, "put away");
    session.roundtrip(&mut client);
    client.take_events();
    tap(&mut session, key::A);
    session.roundtrip(&mut client);
    assert!(
        typed(client.take_events()),
        "and the window has the keys again"
    );
    drop(window);
}

#[test]
fn about_does_not_outlast_the_session_being_locked() {
    let mut session = unblurred();
    about(&mut session);
    let (mut locker, lock, _surface) = crate::lock::lock(&mut session);
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    session.settle();
    assert_eq!(session.draw().at(GROUND.0, GROUND.1), BACKGROUND);
}

/// The middle of the button that copies, on a screen 640 by 480. "About"
/// is as tall as that screen lets it be, its top 12 units down; the button
/// is at the foot of the mark, at the right, inside the border and padding
/// of 21; and it is 28 tall, and wider than this is in.
pub(crate) const COPY: (i32, i32) = (WIDTH - 12 - 21 - 6, 12 + 21 + 56 - 14);

/// What a program pasting gets, once it has asked.
fn pasted(session: &mut Session, client: &mut crate::common::Client) -> String {
    let pipe = client.paste();
    session.roundtrip(client);
    // Nothing on offer is nothing pasted, which the test finds wanting.
    let mut text = String::new();
    let _ = pipe.map(|mut pipe| pipe.read_to_string(&mut text));
    text
}

#[test]
fn about_says_what_the_computer_is_and_its_button_copies_it_for_a_window_to_paste() {
    let mut session = unblurred();
    // A second screen, beside the first, for there to be two of a kind.
    session.add_output((WIDTH, HEIGHT));
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), GREEN);
    about(&mut session);
    let plain = session.draw().at(COPY.0, COPY.1);
    assert_ne!(plain, UNBLURRED, "a button");
    let position = Input::Motion {
        x: f64::from(COPY.0),
        y: f64::from(COPY.1),
    };
    session.compositor_mut().input(position, 0);
    session.settle();
    assert_ne!(
        session.draw().at(COPY.0, COPY.1),
        plain,
        "raised under the pointer"
    );

    click(&mut session, COPY);
    assert_eq!(
        session.draw().at(GROUND.0, GROUND.1),
        UNBLURRED,
        "still shown"
    );
    session.roundtrip(&mut client);
    let text = pasted(&mut session, &mut client);
    assert!(text.starts_with("omorfos "), "{text}");
    // What every computer has, and the screen of the tests, as it is.
    for kind in ["Processor", "Memory", "System"] {
        assert!(text.contains(&format!("\n{kind}: ")), "{kind} in {text}");
    }
    for name in ["test-0", "test-1"] {
        let screen = format!("\nScreens: {name} \u{b7} 640\u{d7}480 at 60 Hz \u{b7} scale 1\n");
        assert!(text.contains(&screen), "{text}");
    }
    drop(window);
}

/// The colour of what is done, the background of the button that copies
/// once it has, in the dark scheme.
const DONE: u32 = 0xffa3_e635;

/// Inside the button that copies, at its right, where nothing is written.
const COPY_SIDE: (i32, i32) = (WIDTH - 12 - 21 - 3, COPY.1);

/// The pixels of the button that copies and a little round it, row by row.
fn copy_button(session: &mut Session) -> Vec<u32> {
    let screen = session.draw();
    let (rows, columns) = (
        COPY.1 - 16..COPY.1 + 16,
        WIDTH - 12 - 21 - 80..WIDTH - 12 - 21,
    );
    let rows: Vec<Vec<u32>> = rows
        .map(|y| columns.clone().map(|x| screen.at(x, y)).collect())
        .collect();
    rows.concat()
}

/// How many pixels of two pictures differ.
fn differ(one: &[u32], other: &[u32]) -> usize {
    one.iter()
        .zip(other)
        .filter(|(one, other)| one != other)
        .count()
}

#[test]
fn the_button_that_copies_turns_green_draws_a_tick_and_is_as_it_was_a_moment_later() {
    let mut session = unblurred();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), GREEN);
    about(&mut session);
    let plain = copy_button(&mut session);
    // Pressed, then the pointer moves off it, so that it is not raised.
    let (x, y) = (f64::from(COPY.0), f64::from(COPY.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    for pressed in [true, false] {
        let click = Input::Button {
            button: LEFT_BUTTON,
            pressed,
        };
        session.compositor_mut().input(click, 0);
    }
    let (x, y) = (f64::from(GROUND.0), f64::from(GROUND.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    let copied = copy_button(&mut session);
    assert_eq!(session.draw().at(COPY_SIDE.0, COPY_SIDE.1), DONE, "green");
    // Drawn again a frame on, while its tick is being drawn.
    let now = session.now;
    let due = session.compositor().deadline();
    assert!(
        due.is_some_and(|due| due <= now + Duration::from_millis(20)),
        "{due:?}"
    );

    session.advance(Duration::from_millis(150));
    let part = copy_button(&mut session);
    session.advance(Duration::from_millis(200));
    let whole = copy_button(&mut session);
    let (some, all) = (differ(&copied, &part), differ(&copied, &whole));
    assert!(
        some > 0 && all > some,
        "the tick drawn bit by bit: {some}, then {all}"
    );
    // Then nothing moves until it is to be as it was.
    let due = session.compositor().deadline();
    let later = session.now + Duration::from_secs(1);
    assert!(due.is_some_and(|due| due > later), "{due:?}");

    session.advance(Duration::from_millis(1200));
    assert_eq!(copy_button(&mut session), plain, "as it was");
    assert_eq!(
        session.compositor().deadline(),
        None,
        "and nothing more to draw"
    );
    drop(window);
}

#[test]
fn the_buttons_of_about_are_pointed_at_with_a_hand() {
    const YELLOW: u32 = 0xffff_ff00;
    let mut session = unblurred();
    let themes =
        session.install_cursors("default", &[("default", 8, GREEN), ("pointer", 8, YELLOW)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    about(&mut session);
    assert_eq!(
        crate::cursors::pointer_at(&mut session, COPY.0, COPY.1),
        YELLOW
    );
    assert_eq!(
        crate::cursors::pointer_at(&mut session, GROUND.0, GROUND.1),
        GREEN
    );
}
