//! Starting the machine again or turning it off, asked first, and sleep.
//!
//! What watches over the session is a stand-in here, which notes what it
//! is asked: nothing is ever started again or turned off.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::path::PathBuf;
use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use smithay::input::keyboard::Keycode;

use crate::common::{HEIGHT, Session, WIDTH, key, runnable};
use crate::idle::a_moment;
use crate::windows::{BACKGROUND, GREEN};

const LEFT: u32 = 0x110;

/// A session with `config` as its config.yaml, whose stand-in for what
/// watches over it writes what it is asked to the file returned.
fn session(config: &str) -> (Session, tempfile::TempDir, PathBuf) {
    let directory = tempfile::TempDir::new().unwrap();
    let asked = directory.path().join("asked");
    let program = directory.path().join("omorfos-session");
    let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\n", asked.display());
    runnable(&program, &script);
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.compositor_mut().set_session(&program);
    session.step();
    (session, directory, asked)
}

fn said(asked: &PathBuf) -> String {
    std::fs::read_to_string(asked).unwrap_or_default()
}

fn press(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
        session.step();
    }
}

fn click_at(session: &mut Session, (x, y): (i32, i32)) {
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    for pressed in [true, false] {
        let input = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(input, 0);
        session.step();
    }
}

/// The middle of a button, as the question says where it is.
fn middle((x, y, width, height): (i32, i32, u32, u32)) -> (i32, i32) {
    (
        x + i32::try_from(width / 2).unwrap(),
        y + i32::try_from(height / 2).unwrap(),
    )
}

/// Whether the question is on the screen, in its middle.
fn asked_on_screen(session: &mut Session) -> bool {
    session.draw().at(WIDTH / 2, HEIGHT / 2) != BACKGROUND
}

#[test]
fn shutting_down_asks_first_and_escape_or_cancel_or_a_click_elsewhere_say_no() {
    let (mut session, _directory, asked) = session("");
    assert!(!asked_on_screen(&mut session));
    session.compositor_mut().perform(&Action::ShutDown);
    session.step();
    assert!(asked_on_screen(&mut session), "the question");
    assert!(session.compositor().leaving_buttons().is_some());

    press(&mut session, key::ESCAPE);
    assert!(!asked_on_screen(&mut session), "gone at Escape");
    assert_eq!(session.compositor().leaving_buttons(), None);

    session.compositor_mut().perform(&Action::ShutDown);
    session.step();
    let (cancel, _) = session.compositor().leaving_buttons().unwrap();
    click_at(&mut session, middle(cancel));
    assert!(!asked_on_screen(&mut session), "gone at Cancel");

    session.compositor_mut().perform(&Action::Restart);
    session.step();
    click_at(&mut session, (5, 5));
    assert!(!asked_on_screen(&mut session), "gone at a click elsewhere");
    a_moment();
    assert_eq!(said(&asked), "", "nothing asked of the machine");
}

#[test]
fn enter_or_the_button_says_yes_and_the_machine_is_asked_to_do_it() {
    let (mut session, _directory, asked) = session("");
    session.compositor_mut().perform(&Action::ShutDown);
    session.step();
    // A click on the card, not on a button, changes nothing.
    let (cancel, go_on) = session.compositor().leaving_buttons().unwrap();
    click_at(&mut session, (cancel.0 - 30, cancel.1 - 30));
    assert!(asked_on_screen(&mut session), "still asked");
    press(&mut session, key::RETURN);
    session.eventually(|_| said(&asked) == "shut-down\n");

    session.compositor_mut().perform(&Action::Restart);
    session.step();
    let (_, go_on_again) = session.compositor().leaving_buttons().unwrap();
    assert_ne!(go_on_again, go_on, "restart's button says otherwise");
    click_at(&mut session, middle(go_on_again));
    session.eventually(|_| said(&asked) == "shut-down\nrestart\n");
    assert!(!asked_on_screen(&mut session));
}

#[test]
fn the_button_under_the_pointer_is_lit() {
    let (mut session, _directory, _asked) = session("");
    session.compositor_mut().perform(&Action::Restart);
    session.step();
    let (cancel, _) = session.compositor().leaving_buttons().unwrap();
    let (x, y) = middle(cancel);
    let before = session.draw().at(cancel.0 + 2, y);
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    session.step();
    assert_ne!(session.draw().at(cancel.0 + 2, middle(cancel).1), before);
}

#[test]
fn told_not_to_ask_it_does_it_at_once_and_sleep_never_asks() {
    let (mut session, _directory, asked) = session("power: { ask: false }\n");
    session.compositor_mut().perform(&Action::Restart);
    session.step();
    assert_eq!(session.compositor().leaving_buttons(), None, "not asked");
    session.eventually(|_| said(&asked) == "restart\n");
    session.compositor_mut().perform(&Action::Sleep);
    session.eventually(|_| said(&asked) == "restart\nsleep\n");
}

#[test]
fn the_buttons_of_the_question_are_pointed_at_with_a_hand() {
    const YELLOW: u32 = 0xffff_ff00;
    let (mut session, _directory, _asked) = session("");
    let themes =
        session.install_cursors("default", &[("default", 8, GREEN), ("pointer", 8, YELLOW)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    session.compositor_mut().perform(&Action::ShutDown);
    session.step();
    let (cancel, go_on) = session.compositor().leaving_buttons().unwrap();
    for button in [cancel, go_on] {
        let (x, y) = middle(button);
        assert_eq!(crate::cursors::pointer_at(&mut session, x, y), YELLOW);
    }
    let (x, y) = middle(cancel);
    assert_eq!(
        crate::cursors::pointer_at(&mut session, x, y - 40),
        GREEN,
        "elsewhere on it"
    );
}

#[test]
fn the_session_is_dimmed_round_the_question_as_round_a_prompt_for_a_secret() {
    let (mut session, _directory, _asked) = session("");
    assert_eq!(session.draw().at(5, 5), BACKGROUND);
    session.compositor_mut().perform(&Action::Restart);
    session.step();
    let dimmed = session.draw().at(5, 5);
    assert_ne!(dimmed, BACKGROUND, "dimmed");
    let [_, red, green, blue] = dimmed.to_be_bytes();
    let [_, was_red, was_green, was_blue] = BACKGROUND.to_be_bytes();
    assert!(
        red <= was_red && green <= was_green && blue <= was_blue,
        "darker"
    );
    press(&mut session, key::ESCAPE);
    assert_eq!(session.draw().at(5, 5), BACKGROUND, "and bright again");
}

#[test]
fn left_alone_the_question_counts_down_a_minute_and_then_it_is_done() {
    let (mut session, _directory, asked) = session("");
    session.compositor_mut().perform(&Action::ShutDown);
    session.step();
    let (cancel, _) = session.compositor().leaving_buttons().unwrap();
    // The lines above the buttons, where the seconds left are said.
    let said_above = |session: &mut Session| {
        let picture = session.draw();
        let rows = cancel.1 - 70..cancel.1;
        let columns = cancel.0 - 100..cancel.0 + 100;
        let place = columns.flat_map(|x| rows.clone().map(move |y| (x, y)));
        place.map(|(x, y)| picture.at(x, y)).collect::<Vec<u32>>()
    };
    let at_first = said_above(&mut session);
    assert!(
        session.compositor().deadline().is_some(),
        "woken as a second goes"
    );
    session.advance(Duration::from_secs(30));
    assert_ne!(said_above(&mut session), at_first, "fewer seconds said");
    a_moment();
    assert_eq!(said(&asked), "", "not yet");
    assert!(asked_on_screen(&mut session));

    session.advance(Duration::from_secs(31));
    session.eventually(|_| said(&asked) == "shut-down\n");
    assert!(!asked_on_screen(&mut session), "done, and gone");
}

#[test]
fn the_question_is_over_the_dim_not_under_it() {
    // A dim that hides all under it: the card is to be seen through it.
    let mut session = Session::with_config(&[("theme.css", "sealed { background: #000000; }\n")]);
    session.compositor_mut().perform(&Action::Restart);
    session.step();
    let (cancel, go_on) = session.compositor().leaving_buttons().unwrap();
    let picture = session.draw();
    assert_eq!(picture.at(5, 5), 0xff00_0000, "the dim");
    let (x, y) = middle(go_on);
    assert_ne!(
        picture.at(x, y),
        0xff00_0000,
        "the button that goes on, over it"
    );
    let (x, y) = middle(cancel);
    assert_ne!(picture.at(x, y - 50), 0xff00_0000, "and the card's words");
}

#[test]
fn logging_out_asks_first_and_ends_the_session_once_told_to() {
    let (mut session, _directory, asked) = session("");
    session.compositor_mut().perform(&Action::LogOut);
    session.step();
    assert!(asked_on_screen(&mut session), "asked");
    press(&mut session, key::ESCAPE);
    assert!(session.compositor().is_running(), "not at Escape");
    session.compositor_mut().perform(&Action::LogOut);
    session.step();
    press(&mut session, key::RETURN);
    assert!(!session.compositor().is_running(), "ended at Enter");
    assert_eq!(said(&asked), "", "nothing asked of the machine");
}

#[test]
fn told_not_to_ask_logging_out_ends_the_session_at_once() {
    let (mut session, _directory, _asked) = session("power: { ask: false }\n");
    session.compositor_mut().perform(&Action::LogOut);
    assert!(!session.compositor().is_running());
}
