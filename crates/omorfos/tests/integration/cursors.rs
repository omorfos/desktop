//! What the pointer looks like: shapes from the cursor theme, and what
//! applications ask for.

use omorfos::{Input, Scale};
use omorfos_config::Action;
use serde_json::json;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

use crate::common::{Client, Session, Toplevel};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const LEFT_BUTTON: u32 = 0x110;
const YELLOW: u32 = 0xffff_ff00;
const MAGENTA: u32 = 0xffff_00ff;
const CYAN: u32 = 0xff00_ffff;
const WHITE: u32 = 0xffff_ffff;
const ORANGE: u32 = 0xffff_8000;
const GREY: u32 = 0xff80_8080;

/// A session that draws its own pointer from a theme of coloured squares,
/// 8 pixels across: a different colour for each shape.
fn themed() -> Session {
    let mut session = Session::new();
    let themes = session.install_cursors(
        "default",
        &[
            ("default", 8, GREEN),
            ("text", 8, BLUE),
            ("ew-resize", 8, YELLOW),
            ("ns-resize", 8, MAGENTA),
            ("nwse-resize", 8, CYAN),
            ("nesw-resize", 8, ORANGE),
            ("grabbing", 8, GREY),
        ],
    );
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    session
}

/// Moves the pointer and returns the colour it is drawn in there.
fn pointer_at(session: &mut Session, x: i32, y: i32) -> u32 {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
    color_at(session, x, y)
}

/// The colour of the pointer drawn at a position: a little inside its
/// picture, whose corner is one pixel up and left of the position.
fn color_at(session: &mut Session, x: i32, y: i32) -> u32 {
    session.step();
    session.draw().at(x + 3, y + 3)
}

fn button(session: &mut Session, pressed: bool) {
    let input = Input::Button {
        button: LEFT_BUTTON,
        pressed,
    };
    session.compositor_mut().input(input, 0);
}

/// A window with the pointer over it, so that the client may say what the
/// pointer looks like.
fn hovered(session: &mut Session) -> (Client, Toplevel) {
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    (client, window)
}

#[test]
fn the_pointer_is_drawn_from_the_cursor_theme() {
    let mut session = themed();
    assert_eq!(pointer_at(&mut session, 300, 300), GREEN);
    let picture = session.draw();
    assert_eq!(picture.at(299, 299), GREEN, "its hotspot is one pixel in");
    assert_eq!(picture.at(306, 306), GREEN, "it is 8 pixels across");
    assert_eq!(picture.at(307, 307), BACKGROUND);
    assert_eq!(picture.at(298, 298), BACKGROUND);
}

#[test]
fn an_application_names_the_shape_of_the_pointer_over_it() {
    let mut session = themed();
    let (mut client, _window) = hovered(&mut session);
    assert_eq!(color_at(&mut session, 50, 50), GREEN, "until it says");

    client.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 50), BLUE);

    // Off the window the shape is not the application's to choose.
    assert_eq!(pointer_at(&mut session, 400, 300), GREEN);
}

#[test]
fn a_shape_the_theme_lacks_is_shown_as_the_ordinary_pointer() {
    let mut session = themed();
    let (mut client, _window) = hovered(&mut session);
    client.set_cursor_shape(Shape::Help);
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 50), GREEN);
}

#[test]
fn an_application_draws_the_pointer_itself_or_hides_it() {
    let mut session = themed();
    let (mut client, _window) = hovered(&mut session);

    let picture = client.surface();
    client.set_cursor(Some(&picture), (2, 2));
    client.fill(&picture, (6, 6), WHITE);
    client.request_frame(&picture);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let screen = session.draw();
    assert_eq!(screen.at(48, 48), WHITE, "its hotspot is at the position");
    assert_eq!(screen.at(53, 53), WHITE);
    assert_eq!(screen.at(54, 54), RED, "it is 6 pixels across");
    assert_eq!(screen.at(47, 47), RED);
    assert_eq!(
        client.state.told.entered, 2,
        "the picture is on the output too"
    );

    // The picture can be animated, so it is told when a frame is shown.
    client.take_events();
    session
        .compositor_mut()
        .send_frames(std::time::Duration::from_millis(16));
    session.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .contains(&crate::common::Event::FrameDone)
    );

    client.set_cursor(None, (0, 0));
    session.roundtrip(&mut client);
    assert_eq!(
        color_at(&mut session, 50, 50),
        RED,
        "nothing is drawn over the window"
    );
    assert_eq!(
        pointer_at(&mut session, 400, 300),
        GREEN,
        "but it is back off it"
    );
}

#[test]
fn a_picture_that_is_destroyed_gives_way_to_the_ordinary_pointer() {
    let mut session = themed();
    let (mut client, _window) = hovered(&mut session);
    let picture = client.surface();
    client.set_cursor(Some(&picture), (0, 0));
    client.fill(&picture, (6, 6), WHITE);
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 50), WHITE);

    picture.destroy();
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 50), GREEN);
}

#[test]
fn the_pointer_shows_which_way_a_border_resizes() {
    let mut session = themed();
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    // The frame is 202 by 129: a border of one and a titlebar of 28.
    assert_eq!(pointer_at(&mut session, 0, 70), YELLOW, "the left border");
    assert_eq!(
        pointer_at(&mut session, 201, 70),
        YELLOW,
        "the right border"
    );
    assert_eq!(pointer_at(&mut session, 100, 128), MAGENTA, "the bottom");
    assert_eq!(pointer_at(&mut session, 0, 128), ORANGE, "bottom left");
    assert_eq!(pointer_at(&mut session, 201, 128), CYAN, "bottom right");
    assert_eq!(pointer_at(&mut session, 100, 14), GREEN, "the titlebar");
    assert_eq!(pointer_at(&mut session, 100, 70), GREEN, "the window");
}

#[test]
fn the_pointer_shows_a_drag_for_as_long_as_it_lasts() {
    let mut session = themed();
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);

    // Moving by the titlebar.
    pointer_at(&mut session, 100, 14);
    button(&mut session, true);
    assert_eq!(
        pointer_at(&mut session, 300, 200),
        GREY,
        "holding the frame"
    );
    button(&mut session, false);
    assert_eq!(color_at(&mut session, 300, 200), GREEN, "let go");

    // Resizing by the bottom border, wherever the pointer then goes. The
    // frame was moved by 200 and 186.
    assert_eq!(pointer_at(&mut session, 300, 314), MAGENTA);
    button(&mut session, true);
    assert_eq!(
        pointer_at(&mut session, 500, 400),
        MAGENTA,
        "still resizing"
    );
    button(&mut session, false);
    assert_eq!(color_at(&mut session, 500, 400), GREEN);
}

#[test]
fn the_menu_and_the_notice_have_the_ordinary_pointer() {
    let mut session = Session::with_config(&[("theme.yaml", "colors: { accent: nope }\n")]);
    let themes = session.install_cursors("default", &[("default", 8, GREEN), ("text", 8, BLUE)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    let (mut client, _window) = hovered(&mut session);
    client.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 50), BLUE);

    // The notice of the broken theme covers the right of the window.
    assert_eq!(pointer_at(&mut session, 150, 30), GREEN, "on the notice");

    // An application says what it wants each time the pointer arrives.
    pointer_at(&mut session, 50, 90);
    session.roundtrip(&mut client);
    client.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut client);
    assert_eq!(color_at(&mut session, 50, 90), BLUE, "on the window again");
    session.compositor_mut().perform(&Action::ShowMenu);
    assert_eq!(color_at(&mut session, 50, 90), GREEN, "with a menu open");
}

#[test]
fn the_theme_and_size_come_from_the_configuration_and_the_scale() {
    let mut session = Session::new();
    session.install_cursors("default", &[("default", 24, GREEN)]);
    session.install_cursors("big", &[("default", 48, BLUE)]);
    let themes = session.install_cursors("fine", &[("default", 12, YELLOW)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    assert_eq!(pointer_at(&mut session, 300, 300), GREEN);
    assert_eq!(session.draw().at(321, 321), GREEN, "24 pixels across");

    session.write_config("theme.yaml", "cursor: { theme: fine }\n");
    session.compositor_mut().reload_config();
    assert_eq!(color_at(&mut session, 300, 300), YELLOW);
    assert_eq!(session.draw().at(312, 312), BACKGROUND, "12 pixels across");

    // At twice the scale the picture is the same size in pixels, since it
    // is the only one the theme has, and is drawn pixel for pixel.
    session.compositor_mut().suggest_scale(Scale::new(2.0));
    pointer_at(&mut session, 150, 150);
    let screen = session.draw();
    assert_eq!(screen.at(300, 300), YELLOW);
    assert_eq!(screen.at(309, 309), YELLOW);
    assert_eq!(screen.at(312, 312), BACKGROUND, "still 12 pixels across");
}

#[test]
fn programs_started_are_told_the_cursor_theme_and_size() {
    let mut session =
        Session::with_config(&[("theme.yaml", "cursor: { theme: Fancy, size: 32 }\n")]);
    let marker = session.config.path().join("spawned");
    let script = format!(
        "printf '%s %s' \"$XCURSOR_THEME\" \"$XCURSOR_SIZE\" > '{}'",
        marker.display()
    );
    let spawn: Action = serde_json::from_value(json!({ "spawn": ["sh", "-c", script] })).unwrap();
    session.compositor_mut().perform(&spawn);
    session.eventually(|_| std::fs::read_to_string(&marker).unwrap_or_default() == "Fancy 32");
}
