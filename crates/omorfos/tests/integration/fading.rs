//! The picture the screens showed as the compositor was started, drawn over
//! the session's first frames and faded out (ADR 62, the third step).
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::os::fd::AsFd;
use std::time::Duration;

use omorfos_ipc::relay::{self, Hand};
use smithay::backend::allocator::Fourcc;

use omorfos::Scale;

use crate::common::{HEIGHT, Session, WIDTH};
use crate::relayed::unready;
use crate::windows::{BACKGROUND, BLUE, RED};

/// How long the picture before takes to fade.
const FADING: Duration = Duration::from_millis(250);

/// A picture the size of the screen, all of it `colour`, in the format
/// `Fourcc::Argb8888` names, which is little-endian.
fn picture(colour: u32) -> Vec<u8> {
    let pixels = usize::try_from(WIDTH * HEIGHT).unwrap();
    colour.to_le_bytes().repeat(pixels)
}

/// The red in `colour`, from 0 to 255.
const fn red(colour: u32) -> u32 {
    (colour >> 16) & 0xff
}

#[test]
fn the_picture_before_is_drawn_over_the_session_and_fades_out() {
    let mut session = Session::new();
    let output = session.output();
    let before = picture(RED);
    let size = (WIDTH, HEIGHT);
    session
        .compositor_mut()
        .fade_from(&output, &before, size, Fourcc::Argb8888);
    session.step();
    assert_eq!(session.draw().at(300, 300), RED, "all of it at first");
    let frame = session.now + Duration::from_millis(16);
    assert_eq!(session.compositor().deadline(), Some(frame));

    session.advance(FADING / 2);
    let halfway = session.draw();
    assert!(halfway.damaged);
    let mixed = halfway.at(300, 300);
    assert!(
        red(mixed) > red(BACKGROUND) && red(mixed) < red(RED),
        "{mixed:08x}"
    );

    session.advance(FADING / 2);
    assert_eq!(session.draw().at(300, 300), BACKGROUND, "gone");
    assert_eq!(session.compositor().deadline(), None);
}

#[test]
fn the_picture_before_waits_for_the_session_to_be_shown_to_fade() {
    let mut session = Session::new();
    let (_holder, relays, _path, _socket) = unready(&mut session);
    let output = session.output();
    session
        .compositor_mut()
        .fade_from(&output, &picture(RED), (WIDTH, HEIGHT), Fourcc::Argb8888);
    relay::send(relays.as_fd(), &Hand::Kept, &[]).unwrap();
    session.step();
    assert!(!session.draw().damaged, "the screen shows it already");
    // Long after, the session shown, it fades from all of it.
    session.advance(FADING * 2);
    relay::send(relays.as_fd(), &Hand::Ready, &[]).unwrap();
    session.step();
    assert_eq!(session.draw().at(300, 300), RED);
    session.advance(FADING);
    assert_eq!(session.draw().at(300, 300), BACKGROUND);
}

#[test]
fn a_picture_that_is_not_the_size_it_says_is_not_drawn() {
    let mut session = Session::new();
    let output = session.output();
    let short = picture(RED);
    let short = &short[..short.len() - 4];
    session
        .compositor_mut()
        .fade_from(&output, short, (WIDTH, HEIGHT), Fourcc::Argb8888);
    session
        .compositor_mut()
        .fade_from(&output, &picture(RED), (0, HEIGHT), Fourcc::Argb8888);
    session.step();
    assert_eq!(session.draw().at(300, 300), BACKGROUND);
    assert_eq!(session.compositor().deadline(), None);
}

#[test]
fn the_picture_before_covers_the_screen_at_the_scale_it_is_given_after() {
    let mut session = Session::new();
    let output = session.output();
    // Red on the left, blue on the right.
    let half = usize::try_from(WIDTH / 2).unwrap();
    let row = [
        RED.to_le_bytes().repeat(half),
        BLUE.to_le_bytes().repeat(half),
    ]
    .concat();
    let before = row.repeat(usize::try_from(HEIGHT).unwrap());
    session
        .compositor_mut()
        .fade_from(&output, &before, (WIDTH, HEIGHT), Fourcc::Argb8888);
    // As the backend does, which says how large the screen's pixels are
    // only once it has taken it on.
    session.suggest_scale(Scale::new(2.0));
    session.step();
    let first = session.draw();
    assert_eq!(first.at(20, 300), RED);
    assert_eq!(first.at(WIDTH - 20, 300), BLUE, "not made larger");
}
