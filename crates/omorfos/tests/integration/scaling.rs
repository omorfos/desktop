//! Scaling: drawing everything larger on a dense screen.

use omorfos::{Input, Scale};
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use crate::common::{Event, HEIGHT, Session, Told, WIDTH};
use crate::windows::{BACKGROUND, RED};

const WHITE: u32 = 0xffff_ffff;

/// A session on a screen that the backend says suits a scale of `factor`.
fn at_scale(factor: f64) -> Session {
    let mut session = Session::new();
    session.suggest_scale(Scale::new(factor));
    session.step();
    session
}

#[test]
fn a_scaled_screen_is_smaller_in_units_and_everything_on_it_is_drawn_larger() {
    let mut session = at_scale(2.0);
    let size = session.compositor().area_of(&session.output()).size;
    assert_eq!((size.w, size.h), (WIDTH / 2, HEIGHT / 2));

    let mut client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), RED);
    let picture = session.draw();
    // Away from its edges, which stretching softens.
    assert_eq!(picture.at(195, 95), RED, "twice the size it was drawn at");
    assert_eq!(picture.at(205, 50), BACKGROUND);
    assert_eq!(picture.at(100, 105), BACKGROUND);

    // A maximized window is told the size of the screen in units.
    let maximize = serde_json::from_value(serde_json::json!("maximize")).unwrap();
    session.compositor_mut().perform(&maximize);
    session.roundtrip(&mut client);
    let (told, states) = client.last_configure();
    assert_eq!(told, (WIDTH / 2, HEIGHT / 2));
    assert!(states.contains(&State::Maximized));
    drop(window);
}

#[test]
fn a_scale_need_not_be_whole() {
    let mut session = at_scale(1.5);
    let size = session.compositor().area_of(&session.output()).size;
    assert_eq!((size.w, size.h), (427, 320));

    let mut client = session.connect();
    session.open(&mut client, "app", (100, 50), RED);
    let picture = session.draw();
    assert_eq!(picture.at(145, 70), RED, "half as large again");
    assert_eq!(picture.at(155, 40), BACKGROUND);
    assert_eq!(picture.at(80, 80), BACKGROUND);
}

#[test]
fn the_configured_scale_is_used_whatever_suits_the_screen() {
    let mut session = Session::with_config(&[("config.yaml", "output: { scale: 2 }\n")]);
    assert_eq!(
        session.compositor().scale_of(&session.output()),
        Scale::new(2.0)
    );
    session.suggest_scale(Scale::new(1.5));
    assert_eq!(
        session.compositor().scale_of(&session.output()),
        Scale::new(2.0)
    );

    // Without one, the screen decides.
    session.write_config("config.yaml", "output: { scale: auto }\n");
    session.compositor_mut().reload_config();
    assert_eq!(
        session.compositor().scale_of(&session.output()),
        Scale::new(1.5)
    );
}

#[test]
fn applications_are_told_the_scale_so_that_they_can_draw_at_it() {
    let mut session = at_scale(1.5);
    let mut client = session.connect();
    let window = client.toplevel("app", "app");
    client.watch_scale(&window.surface);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.state.told,
        Told {
            buffer_scale: Some(2),
            scale: Some(180),
            entered: 1,
        },
        "before it has drawn anything"
    );

    // It draws at that scale and says how large the result is meant to be.
    client.fill_scaled(&window.surface, (150, 75), (100, 50), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let frame = session.compositor().desktop().frames().next().unwrap();
    assert_eq!(
        (frame.geometry.size.width, frame.geometry.size.height),
        (100, 50),
        "the window is as large as it says, not as its buffer"
    );
    let picture = session.draw();
    assert_eq!(
        picture.at(149, 74),
        RED,
        "pixel for pixel, sharp to its edge"
    );
    assert_eq!(picture.at(150, 40), BACKGROUND);

    // A change of scale reaches it, once.
    session.suggest_scale(Scale::new(2.0));
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.state.told,
        Told {
            buffer_scale: Some(2),
            scale: Some(240),
            entered: 1,
        }
    );
}

#[test]
fn panels_are_told_the_scale_too() {
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
        Anchor, KeyboardInteractivity,
    };

    use crate::common::LayerShape;

    let mut session = at_scale(2.0);
    let mut client = session.connect();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &panel, (WIDTH / 2, 30), RED);
    assert_eq!(client.state.told.buffer_scale, Some(2));
    let picture = session.draw();
    assert_eq!(
        picture.at(WIDTH - 10, 55),
        RED,
        "across the screen, 60 pixels tall"
    );
    assert_eq!(picture.at(WIDTH - 10, 65), BACKGROUND);
}

#[test]
fn decorations_are_drawn_at_the_scale() {
    let mut session = at_scale(2.0);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (100, 50), RED);
    let frame = session.compositor().desktop().frames().next().unwrap();
    let client_area = frame.client();
    assert_eq!(client_area.origin.y, 28, "under a titlebar of 28 units");

    let picture = session.draw();
    // To the right of the title, and below the rounded corner.
    let bar = picture.at(190, 30);
    assert_ne!(bar, RED);
    assert_ne!(bar, BACKGROUND);
    assert_eq!(picture.at(190, 53), bar, "the titlebar is 56 pixels tall");
    assert_eq!(picture.at(190, 62), RED, "and the window begins under it");
    // The border is one unit wide, so two pixels.
    let border = picture.at(0, 80);
    assert_ne!(border, RED);
    assert_eq!(picture.at(1, 80), border);
    assert_eq!(picture.at(6, 80), RED);
}

#[test]
fn the_pointer_moves_in_units_and_is_drawn_at_the_scale() {
    let mut session = at_scale(2.0);
    let mut client = session.connect();
    session.open(&mut client, "app", (100, 50), RED);
    client.take_events();
    session.compositor_mut().show_pointer();

    session
        .compositor_mut()
        .input(Input::Motion { x: 60.0, y: 20.0 }, 1);
    session.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .contains(&Event::PointerEnter { x: 60.0, y: 20.0 }),
        "the application is told where in its own units"
    );
    let picture = session.draw();
    assert_eq!(
        picture.at(123, 46),
        WHITE,
        "the arrow, at twice the position"
    );
    assert_eq!(picture.at(123, 49), WHITE, "and twice the size");
    assert_eq!(picture.at(110, 46), RED);

    // The edge it stops at is the screen's, in units.
    session.compositor_mut().input(
        Input::MotionBy {
            dx: 9000.0,
            dy: 9000.0,
        },
        2,
    );
    session.compositor_mut().input(
        Input::MotionBy {
            dx: -5.0,
            dy: -20.0,
        },
        3,
    );
    assert_eq!(
        session.draw().at(WIDTH - 2 - 8, HEIGHT - 2 - 40 + 12),
        WHITE,
        "five units, so ten pixels, from the last column"
    );
}

#[test]
fn what_is_already_on_screen_is_repainted_when_the_scale_changes() {
    let mut session = Session::with_config(&[("theme.yaml", "colors: { accent: nope }\n")]);
    session.compositor_mut().show_pointer();
    session
        .compositor_mut()
        .input(Input::Motion { x: 40.0, y: 40.0 }, 1);
    let menu = serde_json::from_value(serde_json::json!("show_menu")).unwrap();
    session.compositor_mut().perform(&menu);
    session.step();
    let before = session.draw();
    // The menu opens at the pointer and is 220 wide; the notice of the
    // broken theme is at the top right.
    assert_ne!(before.at(200, 120), BACKGROUND, "the menu");
    assert_eq!(before.at(300, 120), BACKGROUND, "beside the menu");
    assert_ne!(before.at(WIDTH - 20, 20), BACKGROUND, "the notice");
    assert_eq!(before.at(60, 20), BACKGROUND, "left of the notice");

    session.suggest_scale(Scale::new(2.0));
    session.step();
    let after = session.draw();
    assert_ne!(after.at(300, 200), BACKGROUND, "the menu, twice as wide");
    assert_eq!(after.at(60, 200), BACKGROUND, "left of where it now starts");
    assert_ne!(
        after.at(60, 40),
        BACKGROUND,
        "the notice, now most of the width"
    );
    assert_eq!(after.at(83, 86), WHITE, "the pointer, twice as large");
}
