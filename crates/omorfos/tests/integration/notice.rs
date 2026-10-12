//! The on-screen notice that tells the user something is wrong.

use omorfos::Input;

use crate::common::{Event, HEIGHT, Session, WIDTH};
use crate::windows::{BACKGROUND, RED};

const LEFT: u32 = 0x110;
/// The notice's panel colour: the default theme's focused titlebar.
const PANEL: u32 = 0xff23_262e;
const ACCENT: u32 = 0xff5b_9cf5;

/// A point inside the notice, clear of its text: near its right edge, in
/// the padding above the first line.
const INSIDE: (i32, i32) = (WIDTH - 20, 16);

const BROKEN: &str = ":root { --accent: nope; }\n";

fn click_at(session: &mut Session, (x, y): (i32, i32)) {
    session.compositor_mut().input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
    session.compositor_mut().input(
        Input::Button {
            button: LEFT,
            pressed: true,
        },
        0,
    );
    session.compositor_mut().input(
        Input::Button {
            button: LEFT,
            pressed: false,
        },
        0,
    );
    session.settle();
}

#[test]
fn nothing_is_shown_when_nothing_is_wrong() {
    let mut session = Session::new();
    assert_eq!(session.draw().at(INSIDE.0, INSIDE.1), BACKGROUND);
}

#[test]
fn a_configuration_mistake_is_shown_on_screen() {
    let mut session = Session::with_config(&[("theme.css", BROKEN)]);
    session.settle();
    let picture = session.draw();
    assert_eq!(
        picture.at(INSIDE.0, INSIDE.1),
        PANEL,
        "in the top-right corner"
    );
    assert_eq!(picture.at(WIDTH - 13, 40), ACCENT, "outlined");
    assert_eq!(picture.at(WIDTH - 5, 40), BACKGROUND, "with a margin");
    assert_eq!(picture.at(100, HEIGHT - 20), BACKGROUND);
}

#[test]
fn the_notice_goes_away_when_the_mistake_is_fixed() {
    let mut session = Session::with_config(&[("theme.css", BROKEN)]);
    session.settle();
    assert_eq!(session.draw().at(INSIDE.0, INSIDE.1), PANEL);

    session.write_config("theme.css", ":root { --accent: #00ff00; }\n");
    session.compositor_mut().reload_config();
    session.settle();

    assert_eq!(session.draw().at(INSIDE.0, INSIDE.1), BACKGROUND);
}

#[test]
fn a_mistake_made_while_running_is_shown_and_the_old_settings_kept() {
    let mut session = Session::new();
    session.write_config("theme.css", "desktop { background: oops; }\n");
    session.compositor_mut().reload_config();
    session.settle();

    let picture = session.draw();
    assert_eq!(picture.at(INSIDE.0, INSIDE.1), PANEL);
    assert_eq!(
        picture.at(100, HEIGHT - 20),
        BACKGROUND,
        "the previous background is still used"
    );
}

#[test]
fn a_click_dismisses_the_notice_without_reaching_the_window_beneath() {
    let mut session = Session::with_config(&[("theme.css", BROKEN)]);
    let mut client = session.connect();
    session.open(&mut client, "app", (WIDTH, 100), RED);
    session.settle();
    assert_eq!(
        session.draw().at(INSIDE.0, INSIDE.1),
        PANEL,
        "the notice is above windows"
    );
    client.take_events();

    click_at(&mut session, INSIDE);
    session.roundtrip(&mut client);

    assert_eq!(
        session.draw().at(INSIDE.0, INSIDE.1),
        RED,
        "the window shows through again"
    );
    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "{events:?}"
    );
}

#[test]
fn a_dismissed_notice_stays_away_until_the_problems_change() {
    let mut session = Session::with_config(&[("theme.css", BROKEN)]);
    session.settle();
    click_at(&mut session, INSIDE);
    session.compositor_mut().reload_config();
    session.settle();
    assert_eq!(
        session.draw().at(INSIDE.0, INSIDE.1),
        BACKGROUND,
        "the same problem stays dismissed"
    );

    session.write_config("keys.yaml", "bindings: nope\n");
    session.compositor_mut().reload_config();
    session.settle();
    assert_eq!(
        session.draw().at(INSIDE.0, INSIDE.1),
        PANEL,
        "a new problem brings it back"
    );
}

#[test]
fn a_problem_that_comes_back_after_being_fixed_is_shown_again() {
    let mut session = Session::with_config(&[("theme.css", BROKEN)]);
    session.settle();
    click_at(&mut session, INSIDE);

    session.write_config("theme.css", "");
    session.compositor_mut().reload_config();
    session.settle();
    session.write_config("theme.css", BROKEN);
    session.compositor_mut().reload_config();
    session.settle();

    assert_eq!(session.draw().at(INSIDE.0, INSIDE.1), PANEL);
}

#[test]
fn the_notice_follows_the_output_and_the_theme() {
    let mut session = Session::with_config(&[("keys.yaml", "bindings: nope\n")]);
    session.settle();
    session.resize_output((400, 300));
    session.write_config(
        "theme.css",
        "notice { background: #00ff00; color: #ffffff; }\n",
    );
    session.compositor_mut().reload_config();
    session.settle();
    // On a 400 pixel output the notice cannot have its usual width: it
    // spans the output less a margin each side, so it now starts at 12.
    let picture = session.draw();
    assert_eq!(
        picture.at(20, 16),
        0xff00_ff00,
        "restyled, and reaching to the left margin"
    );
    assert_eq!(picture.at(5, 16), BACKGROUND);
}
