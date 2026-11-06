//! What is behind a program blurred where it asks through KDE's protocol
//! (`org_kde_kwin_blur`), until the programs that ask through it ask
//! through `ext-background-effect-v1` (ADR 64).
//!
//! The windows are those of `backdrops`: a red one 200 by 400 in the top
//! left corner, and over it one that cannot be seen at all, 300 by 200,
//! from 32 across and down, so that the red window's edge, at 200 across,
//! runs down the middle of it.
//
// TODO: take these out with what they test; ADR 64 says when.

use crate::backdrops::{
    BLURRED, UNBLURRED, a_window_with_its_own_shadow, ended, over_a_red_window, red,
};
use crate::common::Session;
use crate::windows::BACKGROUND;

/// What it asks for in its own coordinates, by its top: on the screen,
/// from 172 to 272 across and 32 to 132 down, over the red window's edge.
const TOP: (i32, i32, i32, i32) = (140, 0, 100, 100);

/// Whether what is behind the window is blurred at `y` down the screen,
/// just past the red window's edge, where the red would bleed.
fn blurred_at(session: &mut Session, y: i32) -> bool {
    red(session.draw().at(204, y)) > red(BACKGROUND) + 8
}

#[test]
fn a_program_that_asks_through_kdes_protocol_with_no_region_has_all_of_it_blurred_behind() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _blur = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "by its top");
    assert!(blurred_at(&mut session, 200), "by its bottom");
    assert_eq!(session.draw().at(204, 260), BACKGROUND, "below it");
    assert!(session.compositor().blurs() > 0);
}

#[test]
fn a_program_that_asks_through_kdes_protocol_with_a_region_has_only_that_blurred_behind() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _blur = clear.kde_blur(&window.surface, Some(&[TOP]));
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "within it");
    assert!(!blurred_at(&mut session, 200), "below it");
    assert_eq!(session.draw().at(280, 80), BACKGROUND, "beside it");
}

#[test]
fn a_program_that_asks_through_kdes_protocol_with_an_empty_region_has_all_of_it_blurred_behind() {
    // As KDE's compositor does, which the programs that ask through it are written for.
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _blur = clear.kde_blur(&window.surface, Some(&[]));
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 200));
}

#[test]
fn a_region_set_through_kdes_protocol_is_asked_for_once_committed_and_then_the_surface() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let blur = clear.kde_blur(&window.surface, Some(&[TOP]));
    window.surface.commit();
    session.roundtrip(&mut clear);
    // On the screen from 172 to 212 down.
    clear.kde_blur_within(&blur, &[(140, 140, 100, 40)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "not before its own commit");
    assert!(!blurred_at(&mut session, 190));
    blur.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "nor before the surface's");
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred_at(&mut session, 80), "then the new region");
    assert!(blurred_at(&mut session, 190));
}

#[test]
fn a_program_that_unsets_its_blur_through_kdes_protocol_has_none_from_its_next_commit() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _blur = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    clear.kde_unblur(&window.surface);
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "not before the commit");
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred_at(&mut session, 80), "taken away");
}

#[test]
fn a_blur_let_go_of_through_kdes_protocol_goes_unless_a_later_one_was_committed_since() {
    // A new blur committed, then the old one let go of.
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let old = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let new = clear.kde_blur(&window.surface, Some(&[TOP]));
    old.release();
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 80), "the new one's region");
    assert!(!blurred_at(&mut session, 200), "and not the old one's");
    new.release();
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred_at(&mut session, 80), "gone with the last");
}

#[test]
fn a_program_that_asks_through_kdes_protocol_after_destroying_its_surface_is_not_ended() {
    // The protocol has no error to end it with.
    let mut session = Session::new();
    let mut client = session.connect();
    let surface = client.surface();
    let blur = client.kde_blur(&surface, None);
    surface.destroy();
    client.kde_blur_within(&blur, &[(0, 0, 10, 10)]);
    blur.commit();
    blur.release();
    assert_eq!(ended(&mut session, &mut client), None);
}

#[test]
fn nothing_is_blurred_behind_a_program_asking_through_kdes_protocol_when_the_theme_gives_none() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(UNBLURRED, 300);
    let _blur = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred_at(&mut session, 80));
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn what_a_program_last_asked_for_through_either_protocol_is_what_it_has() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _effect = clear.blur_behind(&window.surface, &[TOP]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred_at(&mut session, 200), "the standard's region");
    let _blur = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(
        blurred_at(&mut session, 200),
        "then all of it, as KDE's asks"
    );
}

#[test]
fn a_window_that_asks_through_kdes_protocol_for_all_of_it_is_not_blurred_behind_its_shadow() {
    // As Ghostty, drawn with GTK, which draws its own frame and shadow.
    let (mut session, [_red, (mut clear, window)]) = a_window_with_its_own_shadow(BLURRED);
    let _blur = clear.kde_blur(&window.surface, None);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred_at(&mut session, 180), "within it");
    assert_eq!(session.draw().at(204, 200), BACKGROUND, "not in its shadow");
}
