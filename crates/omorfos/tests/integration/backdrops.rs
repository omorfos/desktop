//! What is behind a program blurred where it asks
//! (`ext-background-effect-v1`, ADR 33).

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, LayerShape, Session, Toplevel, WIDTH};
use crate::windows::{BACKGROUND, RED};

/// A theme in which nothing moves or casts a shadow, and what is behind a
/// program is blurred by 16 pixels where it asks.
const BLURRED: &str = "* { transition: none; }\n\
                       window, window:focus { box-shadow: none; backdrop-filter: blur(16px); }\n";

/// The same, but giving programs no blur.
const UNBLURRED: &str = "* { transition: none; }\n\
                         window, window:focus { box-shadow: none; backdrop-filter: none; }\n";

/// What a program is told the compositor can do: blur.
const BLUR: u32 = 1;

/// How red a pixel is.
fn red(pixel: u32) -> u32 {
    (pixel >> 16) & 0xff
}

/// A session with `theme`, a red window 200 by 400 in the top left corner,
/// and over it a window that cannot be seen at all, `width` by 200,
/// cascaded from the corner to 32 across and down.
fn over_a_red_window(theme: &str, width: i32) -> (Session, [(Client, Toplevel); 2]) {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut red = session.connect();
    let under = session.open(&mut red, "red", (200, 400), RED);
    let mut clear = session.connect();
    let over = session.open(&mut clear, "clear", (width, 200), 0);
    (session, [(red, under), (clear, over)])
}

/// The capabilities the compositor has told `client` of, once it has.
fn told(session: &mut Session, client: &mut Client) -> Vec<u32> {
    session.roundtrip(client);
    client.state.backdrop_capabilities.clone()
}

/// The error `client` was ended with, if any, once the compositor has had
/// a few chances to say it.
fn ended(session: &mut Session, client: &mut Client) -> Option<(String, u32)> {
    let mut error = None;
    for _ in 0..10 {
        session.step();
        error = error.or_else(|| client.error());
    }
    error
}

#[test]
fn a_program_is_told_it_may_have_blur_unless_the_theme_or_the_user_says_not() {
    let settings = [
        (vec![], BLUR),
        (vec![("theme.css", BLURRED)], BLUR),
        (vec![("theme.css", UNBLURRED)], 0),
        (
            vec![("config.yaml", "appearance: { reduce_transparency: true }")],
            0,
        ),
    ];
    for (files, flags) in settings {
        let mut session = Session::with_config(&files);
        let mut client = session.connect();
        assert_eq!(told(&mut session, &mut client), [flags], "{files:?}");
    }
}

#[test]
fn a_program_is_told_again_when_it_may_no_longer_have_blur_and_when_it_may() {
    let mut session = Session::with_config(&[("theme.css", BLURRED)]);
    let mut client = session.connect();
    session.write_config("theme.css", UNBLURRED);
    session.eventually(|session| told(session, &mut client) == [BLUR, 0]);
    session.write_config("theme.css", BLURRED);
    session.eventually(|session| told(session, &mut client) == [BLUR, 0, BLUR]);
}

#[test]
fn a_window_that_can_be_seen_through_has_what_is_behind_it_blurred_where_it_asks() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    // On the screen from 172 to 272 across, over the red window's edge.
    let _effect = clear.blur_behind(&window.surface, &[(140, 0, 100, 100)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let picture = session.draw();
    let past = picture.at(204, 80);
    assert!(
        red(past) > red(BACKGROUND) + 8,
        "blurred past the edge: {past:#x}"
    );
    assert_eq!(picture.at(280, 80), BACKGROUND, "the desktop, beside it");
    assert_eq!(picture.at(204, 160), BACKGROUND, "below where it asked");
    assert_eq!(picture.at(150, 160), RED, "the red window, sharp");
    assert!(session.compositor().blurs() > 0);
}

#[test]
fn what_a_program_asks_for_beyond_its_own_surface_is_not_blurred() {
    // From 32 to 182 across: short of the red window's edge.
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 150);
    let _effect = clear.blur_behind(&window.surface, &[(-100, -100, 2000, 300)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let picture = session.draw();
    assert!(session.compositor().blurs() > 0, "blurred within it");
    assert_eq!(picture.at(204, 80), BACKGROUND, "and not beyond it");
    assert_eq!(picture.at(20, 80), RED, "nor to its left");
}

#[test]
fn a_region_made_of_several_rectangles_is_blurred_as_one() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    // Two rows on the screen, 32 to 52 and 72 to 92 down, apart.
    let rows = [(140, 0, 100, 20), (140, 40, 100, 20)];
    let _effect = clear.blur_behind(&window.surface, &rows);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let picture = session.draw();
    assert!(
        red(picture.at(204, 40)) > red(BACKGROUND) + 8,
        "the first row"
    );
    assert!(red(picture.at(204, 80)) > red(BACKGROUND) + 8, "the second");
    assert_eq!(picture.at(204, 62), BACKGROUND, "nothing between");
    assert_eq!(session.compositor().blurs(), 1, "one blur for both");
}

#[test]
fn nothing_is_blurred_behind_a_program_when_the_theme_gives_programs_no_blur() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(UNBLURRED, 300);
    let _effect = clear.blur_behind(&window.surface, &[(140, 0, 100, 100)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let picture = session.draw();
    assert_eq!(picture.at(204, 80), BACKGROUND);
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn a_program_that_takes_its_region_away_or_its_effect_is_no_longer_blurred_behind() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let effect = clear.blur_behind(&window.surface, &[(140, 0, 100, 100)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    let blurred = |session: &mut Session| red(session.draw().at(204, 80)) > red(BACKGROUND);
    assert!(blurred(&mut session));
    // Until the surface's next commit, as it is.
    clear.blur_within(&effect, &[]);
    session.roundtrip(&mut clear);
    assert!(blurred(&mut session), "not before the commit");
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred(&mut session), "taken away");

    clear.blur_within(&effect, &[(140, 0, 100, 100)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(blurred(&mut session), "asked again");
    effect.destroy();
    session.roundtrip(&mut clear);
    assert!(blurred(&mut session), "not before the commit");
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(!blurred(&mut session), "gone with it");
}

#[test]
fn what_a_program_asked_for_stays_once_it_lets_go_of_the_protocol() {
    let (mut session, [_red, (mut clear, window)]) = over_a_red_window(BLURRED, 300);
    let _effect = clear.blur_behind(&window.surface, &[(140, 0, 100, 100)]);
    clear.let_go_of_backdrops();
    window.surface.commit();
    session.roundtrip(&mut clear);
    assert!(red(session.draw().at(204, 80)) > red(BACKGROUND) + 8);
    assert_eq!(clear.error(), None);
}

#[test]
fn a_program_that_asks_twice_for_one_surface_is_ended() {
    let mut session = Session::new();
    let mut client = session.connect();
    let surface = client.surface();
    let _effect = client.blur_behind(&surface, &[]);
    let _again = client.blur_behind(&surface, &[]);
    let about = "ext_background_effect_manager_v1".to_owned();
    assert_eq!(ended(&mut session, &mut client), Some((about, 0)));
}

#[test]
fn a_program_may_ask_again_for_a_surface_once_it_has_destroyed_what_it_asked_with() {
    let mut session = Session::new();
    let mut client = session.connect();
    let surface = client.surface();
    let effect = client.blur_behind(&surface, &[]);
    effect.destroy();
    let _again = client.blur_behind(&surface, &[(0, 0, 10, 10)]);
    assert_eq!(ended(&mut session, &mut client), None);
}

#[test]
fn a_program_that_asks_for_blur_behind_a_surface_it_has_destroyed_is_ended() {
    let mut session = Session::new();
    let mut client = session.connect();
    let surface = client.surface();
    let effect = client.blur_behind(&surface, &[]);
    surface.destroy();
    client.blur_within(&effect, &[(0, 0, 10, 10)]);
    let about = "ext_background_effect_surface_v1".to_owned();
    assert_eq!(ended(&mut session, &mut client), Some((about, 0)));
}

#[test]
fn a_panel_of_a_program_has_what_is_behind_it_blurred_where_it_asks() {
    let mut session = Session::with_config(&[("theme.css", BLURRED)]);
    let mut under = session.connect();
    let _window = session.open(&mut under, "red", (200, 400), RED);
    let mut panel = session.connect();
    let across = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    let (surface, _role) = session.open_layer(&mut panel, &across, (WIDTH, 30), 0);
    let _effect = panel.blur_behind(&surface, &[(0, 0, WIDTH, 30)]);
    surface.commit();
    session.roundtrip(&mut panel);
    let picture = session.draw();
    assert!(
        red(picture.at(204, 15)) > red(BACKGROUND) + 8,
        "blurred past the edge"
    );
    assert_eq!(picture.at(204, 60), BACKGROUND, "below it");
}

/// A session with a red window 200 by 260 in the top left corner, and over
/// it, cascaded to 32 across and down, a window that cannot be seen at all
/// in a frame, 150 by 200 within it, whose bottom is the red window's,
/// asking for what is behind all of it to be blurred.
fn a_clear_window_in_a_frame() -> (Session, [(Client, Toplevel); 2]) {
    let mut session = Session::with_config(&[("theme.css", BLURRED)]);
    let mut under = session.connect();
    let red = session.open(&mut under, "red", (200, 260), RED);
    let mut clear = session.connect();
    let window = session.open_decorated(&mut clear, "clear", (150, 200), 0);
    let _effect = clear.blur_behind(&window.surface, &[(0, 0, 150, 200)]);
    window.surface.commit();
    session.roundtrip(&mut clear);
    (session, [(under, red), (clear, window)])
}

#[test]
fn what_is_behind_a_window_is_not_blurred_beyond_its_frames_rounded_corners() {
    let (mut session, _windows) = a_clear_window_in_a_frame();
    let picture = session.draw();
    // Along its bottom, blurred with the desktop below; but in its corner,
    // outside the curve, the red window, sharp.
    assert_ne!(picture.at(60, 259), RED, "blurred");
    assert_eq!(picture.at(33, 259), RED, "not in the corner");
}

#[test]
fn what_is_behind_a_program_where_blur_cannot_be_drawn_is_left_as_it_is() {
    let (mut session, _windows) = a_clear_window_in_a_frame();
    session.frosting = None;
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(60, 259), RED, "neither blurred nor solid");
    assert_eq!(picture.at(60, 262), BACKGROUND);
}
