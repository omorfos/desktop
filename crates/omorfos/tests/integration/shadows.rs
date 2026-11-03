//! The shadows parts cast on what is under them (`box-shadow`, ADR 33).

use std::time::Duration;

use omorfos::{Input, Roles};
use omorfos_config::Action;
use omorfos_core::Button;

use crate::common::{HEIGHT, Picture, Session, WIDTH, key};
use crate::layers::panel;
use crate::windows::{BACKGROUND, RED, shaded};

/// The frame of a window 200 by 100, as the default theme makes it, ends
/// this far down: a titlebar of 28, then the window.
const BOTTOM: i32 = 128;

/// A theme in which a window casts a shadow straight down, darker while it
/// has focus.
const SHADOWED: &str = "window { box-shadow: 0 8px 16px #00000060; }\n\
                        window:focus { box-shadow: 0 8px 16px #000000c0; }\n";

/// How dark a pixel is: how far below the background its green falls.
fn darkness(pixel: u32) -> u32 {
    let green = |pixel: u32| (pixel >> 8) & 0xff;
    green(BACKGROUND).saturating_sub(green(pixel))
}

#[test]
fn a_window_casts_the_shadow_its_theme_gives_it_under_it_and_no_further() {
    let mut session = Session::with_config(&[("theme.css", SHADOWED)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    let picture = session.draw();
    assert!(darkness(picture.at(100, BOTTOM + 4)) > 0, "just under it");
    assert!(
        darkness(picture.at(100, BOTTOM + 4)) > darkness(picture.at(100, BOTTOM + 16)),
        "fading as it goes"
    );
    assert_eq!(picture.at(100, BOTTOM + 40), BACKGROUND, "and gone");
    assert_eq!(picture.at(100, 60), RED, "the window over it");
    assert_eq!(picture.at(260, 60), BACKGROUND, "nothing to the side");
}

#[test]
fn a_window_without_focus_casts_the_shadow_its_theme_gives_that() {
    let mut session = Session::with_config(&[("theme.css", SHADOWED)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    // Clear of what the next window covers, and of its shadow.
    let under = (160, BOTTOM + 4);
    let focused = darkness(session.draw().at(under.0, under.1));
    // Another, small, cascaded down and across from it, takes the focus.
    let mut other = session.connect();
    session.open_decorated(&mut other, "other", (100, 50), RED);
    session.eventually(|session| darkness(session.draw().at(under.0, under.1)) < focused);
    assert!(darkness(session.draw().at(under.0, under.1)) > 0);
}

#[test]
fn a_theme_without_shadows_casts_none() {
    let none = "* { transition: none; }\n\
                window, window:focus, menu, notice { box-shadow: none; }\n";
    // With a mistake in the keys, so that the notice says so.
    let mut session = Session::with_config(&[("theme.css", none), ("keys.yaml", "nonsense: [")]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    session.settle();
    let picture = session.draw();
    assert_eq!(picture.at(100, BOTTOM + 4), BACKGROUND);
    assert_ne!(picture.at(WIDTH - 20, 16), BACKGROUND, "the notice");
    assert_eq!(shaded_background(&picture), 0);

    // Nor do the pictures of arrangements, which are drawn as a menu.
    let (x, y) = rest_on_the_second_button(&mut session);
    let picture = session.draw();
    assert_ne!(picture.at(x + 330, y + 3), BACKGROUND, "the pictures");
    assert_eq!(shaded_background(&picture), 0);

    // Nor does a menu, which is 220 wide, beside it or under it.
    let mut session = Session::with_config(&[("theme.css", none)]);
    open_menu(&mut session);
    let picture = session.draw();
    assert_ne!(picture.at(200, 110), BACKGROUND, "the menu");
    assert_eq!(picture.at(322, 110), BACKGROUND, "beside it");
    assert_eq!(picture.at(96, 110), BACKGROUND);
}

/// Opens the menu with its corner at the pointer, at (100, 100).
fn open_menu(session: &mut Session) {
    let motion = Input::Motion { x: 100.0, y: 100.0 };
    session.compositor_mut().input(motion, 0);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
}

#[test]
fn a_menu_casts_the_shadow_its_theme_gives_it() {
    // Cast to the right, as the menu is 220 wide and its height depends on
    // its rows.
    let theme = "* { transition: none; }\nmenu { box-shadow: 8px 0 8px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    open_menu(&mut session);
    let picture = session.draw();
    assert!(
        darkness(picture.at(322, 110)) > 0,
        "beside it, where it falls"
    );
    assert_eq!(picture.at(360, 110), BACKGROUND, "and no further");
    assert_eq!(picture.at(96, 110), BACKGROUND, "nothing on the other side");
}

#[test]
fn what_goes_from_the_screen_takes_its_shadow_with_it_as_it_fades() {
    let theme = "menu, window { transition: 100ms linear; }\n\
                 menu { box-shadow: 8px 0 8px #000000c0; }\n\
                 window, window:focus { box-shadow: 0 8px 16px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    session.advance(Duration::from_millis(100));
    let shadows = |session: &mut Session| {
        let picture = session.draw();
        [
            darkness(picture.at(60, BOTTOM + 4)),
            darkness(picture.at(322, 110)),
        ]
    };
    let [window, menu] = shadows(&mut session);
    assert!(window > 0 && menu > 0, "{window} {menu}");

    session.compositor_mut().perform(&Action::Minimize);
    for pressed in [true, false] {
        let code = key::ESCAPE;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    session.advance(Duration::from_millis(50));
    let [fading_window, fading_menu] = shadows(&mut session);
    assert!(
        (1..window).contains(&fading_window),
        "{fading_window} of {window}"
    );
    assert!((1..menu).contains(&fading_menu), "{fading_menu} of {menu}");
    session.advance(Duration::from_millis(50));
    assert_eq!(shadows(&mut session), [0, 0], "gone");
}

#[test]
fn a_panel_of_a_component_casts_the_shadow_of_its_part_and_is_not_drawn_again() {
    let theme = "* { transition: none; }\npanel { box-shadow: 0 8px 8px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    client.layer_name = "panel";
    session.open_layer(&mut client, &panel(), (WIDTH, 30), RED);
    let picture = session.draw();
    assert_eq!(picture.at(100, 10), RED);
    assert!(darkness(picture.at(100, 32)) > 0, "under it");
    assert_eq!(picture.at(100, 60), BACKGROUND, "and no further");
    // Nothing changing, nothing is drawn again, shadow and all.
    session.step();
    drop(session.draw());
    session.step();
    assert!(!session.draw().damaged);
}

#[test]
fn a_window_that_casts_a_shadow_is_not_drawn_again_while_nothing_changes() {
    let mut session = Session::with_config(&[("theme.css", SHADOWED)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    drop(session.draw());
    session.step();
    assert!(!session.draw().damaged);
}

#[test]
fn a_panel_that_goes_takes_its_shadow_with_it_as_it_fades() {
    let theme = "panel { transition: 100ms linear; box-shadow: 0 8px 8px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    client.layer_name = "panel";
    let (surface, role) = session.open_layer(&mut client, &panel(), (WIDTH, 30), RED);
    session.advance(Duration::from_millis(100));
    let under = |session: &mut Session| darkness(session.draw().at(100, 32));
    let whole = under(&mut session);
    assert!(whole > 0);

    role.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    session.advance(Duration::from_millis(50));
    let fading = under(&mut session);
    assert!((1..whole).contains(&fading), "{fading} of {whole}");
    session.advance(Duration::from_millis(50));
    assert_eq!(under(&mut session), 0, "gone");
}

#[test]
fn windows_that_cast_the_same_shadow_each_cast_it() {
    let same = "window, window:focus { box-shadow: 0 8px 16px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", same)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "first", (200, 100), RED);
    session.open_decorated(&mut client, "second", (100, 50), RED);
    // The first's at x 160, clear of the second, cascaded to (32, 32); the
    // second's under it, on the first, whose red it darkens.
    let picture = session.draw();
    assert!(darkness(picture.at(160, BOTTOM + 4)) > 0);
    let red = |pixel: u32| (pixel >> 16) & 0xff;
    let on_the_first = picture.at(80, 32 + 28 + 50 + 2);
    assert!(red(on_the_first) < red(RED), "{on_the_first:#x}");
}

#[test]
fn the_notice_casts_the_shadow_its_theme_gives_it_and_takes_it_as_it_goes() {
    // Cast to the left of the notice, which is at the top right, from 108
    // across.
    let theme = "notice { transition: 100ms linear; box-shadow: -8px 0 8px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme), ("keys.yaml", "nonsense: [")]);
    session.step();
    session.advance(Duration::from_millis(100));
    let beside = |session: &mut Session| darkness(session.draw().at(104, 20));
    let whole = beside(&mut session);
    assert!(whole > 0, "beside it, where it falls");
    assert_eq!(session.draw().at(60, 20), BACKGROUND, "and no further");
    session.step();
    assert!(!session.draw().damaged, "and it is not drawn again");

    session.write_config("keys.yaml", "");
    session.compositor_mut().reload_config();
    session.step();
    session.advance(Duration::from_millis(50));
    let fading = beside(&mut session);
    assert!((1..whole).contains(&fading), "{fading} of {whole}");
    session.advance(Duration::from_millis(50));
    assert_eq!(beside(&mut session), 0, "gone");
}

/// How many pixels of `picture` are the background in a shadow.
fn shaded_background(picture: &Picture) -> usize {
    let pixels = (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x, y)));
    let shadowed = pixels.map(|(x, y)| picture.at(x, y));
    shadowed
        .filter(|pixel| *pixel != BACKGROUND && shaded(*pixel, BACKGROUND))
        .count()
}

#[test]
fn the_tooltip_of_the_mark_casts_the_shadow_its_theme_gives_it() {
    // Cast up over the mark, and round the tooltip.
    let theme = "* { transition: none; }\ntooltip { box-shadow: 0 -16px 0 16px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut camera = session.connect_as(Roles::CAPTURE);
    let _screen = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let rest = |session: &mut Session, (x, y): (f64, f64)| {
        session.compositor_mut().input(Input::Motion { x, y }, 0);
        session.step();
        session.step();
    };
    // The mark, at the top right: a disc 16 across, and inside it.
    let mark = |picture: &Picture| {
        let around = (WIDTH - 24..WIDTH - 16).flat_map(|x| (12..20).map(move |y| (x, y)));
        around.map(|(x, y)| picture.at(x, y)).collect::<Vec<_>>()
    };
    rest(&mut session, (400.0, 300.0));
    let picture = session.draw();
    assert_eq!(shaded_background(&picture), 0, "no shadow yet");
    let unshaded = mark(&picture);

    rest(&mut session, (f64::from(WIDTH - 20), 16.0));
    assert_ne!(session.compositor().mark_tooltip(), [""; 0], "it is up");
    let picture = session.draw();
    assert!(shaded_background(&picture) > 0, "round the tooltip");
    assert_eq!(mark(&picture), unshaded, "but never over the mark");
    session.step();
    assert!(!session.draw().damaged, "and it is not drawn again");
}

#[test]
fn the_pictures_of_arrangements_cast_the_shadow_of_a_menu() {
    let theme = "* { transition: none; }\n\
                 window, window:focus { box-shadow: none; }\n\
                 menu { box-shadow: 0 8px 8px #000000c0; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    assert_eq!(shaded_background(&session.draw()), 0, "no shadow yet");

    let (x, y) = rest_on_the_second_button(&mut session);
    let picture = session.draw();
    assert_ne!(picture.at(x + 330, y + 3), BACKGROUND, "the pictures");
    assert!(shaded_background(&picture) > 0, "under them");
    session.step();
    assert!(!session.draw().damaged, "and it is not drawn again");
}

/// Rests the pointer on the second button of the titlebar of the only
/// window until the pictures of arrangements are up, under it, past the
/// window to the right; returns where their top left corner is.
fn rest_on_the_second_button(session: &mut Session) -> (i32, i32) {
    let frame = session.compositor().desktop().frames().next();
    let button = frame.and_then(|frame| frame.decoration);
    let button = button.map(|decoration| decoration.button(Button::Maximize));
    let button = button.unwrap_or_default();
    let motion = Input::Motion {
        x: f64::from(button.origin.x + 4),
        y: f64::from(button.origin.y + 4),
    };
    session.compositor_mut().input(motion, 0);
    session.step();
    session.advance(Duration::from_millis(400));
    // A little under the button, with room between for the arrow.
    let below = button.origin.y + button.size.height.cast_signed() + 6;
    (button.origin.x, below)
}
