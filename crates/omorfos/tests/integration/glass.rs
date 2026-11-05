//! Glass: what is behind a part, blurred, under it where its background
//! lets it be seen (`backdrop-filter`, ADR 33).

use omorfos::{Frost, Input};
use omorfos_config::Action;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, LayerShape, Session, Toplevel, WIDTH};
use crate::windows::{BACKGROUND, BLUE, RED};

/// A theme in which a panel is glass: white at half, over what is behind
/// it blurred by 8 pixels.
const GLASS: &str = "panel { background: rgba(255, 255, 255, 0.5); \
                     backdrop-filter: blur(8px); box-shadow: none; }\n";

/// White at half, premultiplied, as the panel draws its tint.
const TINT: u32 = 0x8080_8080;

/// A panel across the top of the screen, `height` high, over the windows
/// and keeping no room from them.
fn across(height: u32) -> LayerShape {
    LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, height),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    }
}

/// How red a pixel is.
fn red(pixel: u32) -> u32 {
    (pixel >> 16) & 0xff
}

/// A session with `theme`, a red window 200 by 100 in the top left corner,
/// and a panel of glass 30 high across the top, over it.
fn over_a_red_window(theme: &str) -> (Session, (Client, Toplevel), Client) {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut window = session.connect();
    let toplevel = session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    (session, (window, toplevel), panel)
}

#[test]
fn a_panel_of_glass_is_redder_over_a_red_window_and_only_there() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    let picture = session.draw();
    let (over, beside) = (picture.at(100, 15), picture.at(400, 15));
    assert!(red(over) > red(beside) + 64, "{over:#x} {beside:#x}");
    // To the edge of the screen, where what is beyond is taken to be what
    // is at the edge.
    assert_eq!(picture.at(100, 0), over, "as red at the top");
    // What is behind it is blurred: its edge is soft, where it is sharp
    // under it.
    assert!(red(picture.at(196, 15)) < red(over), "softened inside");
    assert!(red(picture.at(204, 15)) > red(beside), "and out");
    assert_eq!(picture.at(100, 60), RED, "the window, below it");
    assert_eq!(picture.at(400, 60), BACKGROUND, "the desktop, below it");
}

#[test]
fn glass_is_not_drawn_again_or_blurred_again_while_nothing_behind_it_changes() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    drop(session.draw());
    let blurs = session.compositor().blurs();
    assert!(blurs > 0);
    session.step();
    assert!(!session.draw().damaged, "not drawn again");
    assert_eq!(session.compositor().blurs(), blurs, "nor blurred again");
}

#[test]
fn glass_is_blurred_again_when_what_is_behind_it_changes() {
    let (mut session, (mut window, toplevel), _panel) = over_a_red_window(GLASS);
    let reddish = session.draw().at(100, 15);
    let blurs = session.compositor().blurs();
    window.fill(&toplevel.surface, (200, 100), BLUE);
    session.roundtrip(&mut window);
    session.roundtrip(&mut window);
    let bluish = session.draw().at(100, 15);
    assert!(session.compositor().blurs() > blurs, "blurred again");
    assert!(red(bluish) < red(reddish), "{bluish:#x} {reddish:#x}");
    assert!(bluish & 0xff > reddish & 0xff, "{bluish:#x} {reddish:#x}");
}

#[test]
fn glass_over_the_desktop_alone_takes_what_is_behind_it_from_one_blur_of_it() {
    // Along the bottom, and in the middle of the right side: clear of each
    // other and of where windows open, in the top left corner.
    let mut session = Session::with_config(&[("theme.css", GLASS)]);
    let mut bottom = session.connect();
    bottom.layer_name = "panel";
    let low = LayerShape {
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        ..across(40)
    };
    session.open_layer(&mut bottom, &low, (WIDTH, 40), TINT);
    let mut side = session.connect();
    side.layer_name = "panel";
    let right = LayerShape {
        anchor: Anchor::Right,
        size: (40, 100),
        ..across(40)
    };
    session.open_layer(&mut side, &right, (40, 100), TINT);
    let picture = session.draw();
    assert_eq!(picture.at(300, 470), picture.at(620, 240), "the same glass");
    assert_ne!(picture.at(300, 470), BACKGROUND, "tinted");
    assert_eq!(session.compositor().blurs(), 1, "one blur, of the desktop");
    // A window opening clear of them blurs nothing more.
    let mut window = session.connect();
    session.open(&mut window, "app", (100, 100), RED);
    session.step();
    drop(session.draw());
    assert_eq!(session.compositor().blurs(), 1);
}

#[test]
fn glass_where_blur_cannot_be_drawn_is_solid_in_its_colour() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    session.compositor_mut().set_frost(Frost::Solid);
    session.step();
    let picture = session.draw();
    // Its tint over white, which is white, over the window and beside it.
    assert_eq!(picture.at(100, 15), 0xffff_ffff);
    assert_eq!(picture.at(400, 15), 0xffff_ffff);
    assert_eq!(session.compositor().blurs(), 0, "and nothing is blurred");
}

#[test]
fn glass_has_its_corners_rounded_as_its_part_does() {
    let rounded = "panel { background: rgba(255, 255, 255, 0.5); border-radius: 12px; \
                   backdrop-filter: blur(8px); box-shadow: none; }\n";
    let (mut session, _window, _panel) = over_a_red_window(rounded);
    session.compositor_mut().set_frost(Frost::Solid);
    session.step();
    let picture = session.draw();
    // In the corner, nothing of the glass: the tint over the window as it
    // is. Within, the tint over white.
    assert_eq!(picture.at(0, 0), 0xffff_8080, "the tint over red");
    assert_eq!(picture.at(20, 15), 0xffff_ffff);
    assert_eq!(
        picture.at(WIDTH - 1, 0),
        0xff8a_8b8d,
        "the tint over the desktop"
    );
}

#[test]
fn a_part_whose_background_cannot_be_seen_through_has_no_glass() {
    let opaque = "panel { background: #ffffff; backdrop-filter: blur(8px); }\n";
    let (mut session, _window, _panel) = over_a_red_window(opaque);
    drop(session.draw());
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn glass_is_solid_where_transparency_is_to_be_reduced() {
    let mut session = Session::with_config(&[
        ("theme.css", GLASS),
        ("config.yaml", "appearance: { reduce_transparency: true }"),
    ]);
    let mut window = session.connect();
    session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    let picture = session.draw();
    assert_eq!(picture.at(100, 15), 0xffff_ffff);
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn a_menu_of_glass_over_a_red_window_is_red_there_and_not_beside_it() {
    let theme = "window, window:focus { box-shadow: none; }\n\
                 menu { background: rgba(255, 255, 255, 0.5); \
                 backdrop-filter: blur(8px); box-shadow: none; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 400), RED);
    // Its corner at (100, 100), 220 wide: half over the window.
    let motion = Input::Motion { x: 100.0, y: 100.0 };
    session.compositor_mut().input(motion, 0);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    let picture = session.draw();
    // Red more than green, whatever is written on it there.
    let cast = |pixel: u32| i64::from(red(pixel)) - i64::from((pixel >> 8) & 0xff);
    assert!(
        cast(picture.at(150, 104)) > 64,
        "{:#x}",
        picture.at(150, 104)
    );
    assert!(
        cast(picture.at(300, 104)).abs() < 16,
        "{:#x}",
        picture.at(300, 104)
    );
    assert!(session.compositor().blurs() > 0);
}
