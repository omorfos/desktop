//! Layer-shell: panels, docks, launchers and wallpapers.

use omorfos::Input;
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use std::time::Duration;

use crate::common::{Event, HEIGHT, LayerShape, Session, WIDTH, between, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
const YELLOW: u32 = 0xffff_ff00;

/// A 30 pixel panel across the top that reserves its height.
fn panel() -> LayerShape {
    LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    }
}

/// A surface covering the whole output on `layer`, reserving nothing.
fn covering(layer: Layer) -> LayerShape {
    LayerShape {
        layer,
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        size: (0, 0),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    }
}

/// A 100 by 50 box in the top-left corner on `layer`.
fn corner(layer: Layer) -> LayerShape {
    LayerShape {
        layer,
        anchor: Anchor::Top | Anchor::Left,
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    }
}

fn move_to(session: &mut Session, x: i32, y: i32) {
    session.compositor_mut().input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
}

fn click(session: &mut Session, button: u32) {
    session.compositor_mut().input(
        Input::Button {
            button,
            pressed: true,
        },
        0,
    );
    session.compositor_mut().input(
        Input::Button {
            button,
            pressed: false,
        },
        0,
    );
    session.step();
}

#[test]
fn a_panel_is_given_the_width_of_the_edge_it_is_anchored_to_and_drawn_there() {
    let mut session = Session::new();
    let mut client = session.connect();

    let (surface, _role) = client.layer(&panel());
    session.roundtrip(&mut client);
    let width = u32::try_from(WIDTH).unwrap();
    assert!(
        client
            .take_events()
            .contains(&Event::LayerConfigure { size: (width, 30) })
    );

    client.fill(&surface, (WIDTH, 30), YELLOW);
    session.roundtrip(&mut client);
    let picture = session.draw();
    assert_eq!(picture.at(0, 0), YELLOW);
    assert_eq!(picture.at(WIDTH - 1, 29), YELLOW);
    assert_eq!(picture.at(100, 30), BACKGROUND);
}

#[test]
fn a_panel_of_another_program_is_there_at_once_whatever_the_theme_says() {
    let theme = "panel, window, menu { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();

    session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);

    assert_eq!(session.draw().at(100, 10), YELLOW);
}

#[test]
fn a_panel_named_after_a_part_comes_and_goes_over_the_time_that_part_is_given() {
    let theme = "panel { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut client = session.connect();
    client.layer_name = "panel";
    let bar = |session: &mut Session| session.draw().at(100, 10);

    let (surface, role) = session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);
    assert_eq!(bar(&mut session), BACKGROUND, "not there yet");
    session.advance(Duration::from_millis(100));
    assert_eq!(bar(&mut session), YELLOW);

    role.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    assert_eq!(bar(&mut session), YELLOW, "still all there");
    session.advance(Duration::from_millis(50));
    let half = bar(&mut session);
    assert!(between(half, BACKGROUND, YELLOW), "{half:#x}");
    session.advance(Duration::from_millis(50));
    assert_eq!(bar(&mut session), BACKGROUND, "gone");
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");
}

#[test]
fn windows_keep_clear_of_a_panel() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);

    let window = session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(
        session.draw().at(5, 35),
        RED,
        "a new window opens below the panel"
    );
    assert_eq!(session.draw().at(5, 5), YELLOW);

    client.take_events();
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        (
            (WIDTH, HEIGHT - 30),
            vec![State::Maximized, State::Activated]
        )
    );
}

#[test]
fn the_space_is_given_back_when_the_panel_goes_away() {
    let mut session = Session::new();
    let mut client = session.connect();
    let (surface, role) = session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);
    let window = session.open(&mut client, "app", (200, 100), RED);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    client.take_events();

    role.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    assert_eq!(client.last_configure().0, (WIDTH, HEIGHT));
    assert_eq!(
        session.draw().at(WIDTH - 1, 5),
        BACKGROUND,
        "the panel is gone"
    );
}

#[test]
fn a_panel_follows_the_output_when_it_is_resized() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);
    client.take_events();

    session.resize_output((800, 600));
    session.roundtrip(&mut client);

    assert!(
        client
            .take_events()
            .contains(&Event::LayerConfigure { size: (800, 30) })
    );
}

#[test]
fn layers_stack_around_windows_in_order() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(
        &mut client,
        &covering(Layer::Background),
        (WIDTH, HEIGHT),
        GREEN,
    );
    session.open_layer(&mut client, &corner(Layer::Bottom), (100, 50), YELLOW);
    session.open(&mut client, "app", (60, 30), RED);
    session.open_layer(&mut client, &corner(Layer::Top), (20, 10), BLUE);

    let picture = session.draw();
    assert_eq!(picture.at(5, 5), BLUE, "the top layer is above the window");
    assert_eq!(
        picture.at(40, 20),
        RED,
        "the window is above the bottom layer"
    );
    assert_eq!(
        picture.at(90, 40),
        YELLOW,
        "the bottom layer is above the wallpaper"
    );
    assert_eq!(
        picture.at(300, 300),
        GREEN,
        "the wallpaper is behind everything"
    );
}

#[test]
fn a_fullscreen_window_covers_panels_but_not_overlays() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &corner(Layer::Top), (100, 50), YELLOW);
    session.open_layer(&mut client, &corner(Layer::Overlay), (20, 10), BLUE);
    let window = session.open(&mut client, "video", (200, 100), RED);
    window.role.set_fullscreen(None);
    session.roundtrip(&mut client);
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let picture = session.draw();
    assert_eq!(picture.at(5, 5), BLUE, "the overlay stays on top");
    assert_eq!(
        picture.at(60, 30),
        RED,
        "the fullscreen window covers the panel"
    );
}

#[test]
fn the_pointer_reaches_layer_surfaces() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    move_to(&mut session, 300, 12);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);

    let events = client.take_events();
    assert!(
        events.contains(&Event::PointerEnter { x: 300.0, y: 12.0 }),
        "{events:?}"
    );
    assert!(
        events.contains(&Event::PointerButton {
            button: LEFT,
            pressed: true
        }),
        "{events:?}"
    );
}

#[test]
fn the_pointer_reaches_a_fullscreen_window_under_an_overlay_elsewhere() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &corner(Layer::Overlay), (20, 10), BLUE);
    session.open_layer(&mut client, &corner(Layer::Bottom), (100, 50), YELLOW);
    let window = session.open(&mut client, "video", (200, 100), RED);
    window.role.set_fullscreen(None);
    session.roundtrip(&mut client);
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
    session.roundtrip(&mut client);
    client.take_events();

    move_to(&mut session, 400, 300);
    session.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .contains(&Event::PointerEnter { x: 400.0, y: 300.0 })
    );
}

#[test]
fn a_bottom_layer_gets_the_pointer_where_no_window_covers_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(&mut client, &corner(Layer::Bottom), (100, 50), YELLOW);
    client.take_events();

    move_to(&mut session, 50, 25);
    session.roundtrip(&mut client);

    assert!(
        client
            .take_events()
            .contains(&Event::PointerEnter { x: 50.0, y: 25.0 })
    );
}

#[test]
fn a_right_click_on_the_wallpaper_opens_the_root_menu() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(
        &mut client,
        &covering(Layer::Background),
        (WIDTH, HEIGHT),
        GREEN,
    );
    client.take_events();

    move_to(&mut session, 300, 200);
    click(&mut session, RIGHT);
    session.roundtrip(&mut client);

    assert_ne!(
        session.draw().at(400, 220),
        GREEN,
        "the menu is drawn over the wallpaper"
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
fn a_left_click_on_the_wallpaper_goes_to_the_wallpaper() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open_layer(
        &mut client,
        &covering(Layer::Background),
        (WIDTH, HEIGHT),
        GREEN,
    );
    client.take_events();

    move_to(&mut session, 300, 200);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);

    assert!(client.take_events().contains(&Event::PointerButton {
        button: LEFT,
        pressed: true
    }));
}

#[test]
fn a_launcher_that_demands_the_keyboard_has_it_until_it_closes() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    let launcher = LayerShape {
        keyboard: KeyboardInteractivity::Exclusive,
        ..corner(Layer::Overlay)
    };
    let (surface, role) = session.open_layer(&mut client, &launcher, (100, 50), BLUE);
    let events = client.take_events();
    assert!(
        events.contains(&Event::KeyboardLeave),
        "the window loses the keyboard: {events:?}"
    );
    assert!(
        events.contains(&Event::KeyboardEnter),
        "the launcher gains it: {events:?}"
    );

    session.compositor_mut().input(
        Input::Key {
            code: key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::Key {
        code: 30,
        pressed: true
    }));

    role.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::KeyboardEnter),
        "the window has the keyboard back: {events:?}"
    );
}

#[test]
fn a_panel_that_does_not_want_the_keyboard_leaves_it_with_the_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    session.open_layer(&mut client, &panel(), (WIDTH, 30), YELLOW);

    assert!(!client.take_events().contains(&Event::KeyboardLeave));
}

#[test]
fn a_layer_surface_lets_the_pointer_through_outside_its_input_region() {
    let mut session = Session::new();
    let mut client = session.connect();
    let (surface, _role) = session.open_layer(&mut client, &corner(Layer::Top), (100, 50), YELLOW);
    // Only the left half of the box takes the pointer.
    client.input_region(&surface, (0, 0, 50, 50));
    session.roundtrip(&mut client);
    client.take_events();

    move_to(&mut session, 75, 25);
    session.roundtrip(&mut client);
    assert!(
        !client
            .take_events()
            .iter()
            .any(|event| matches!(event, Event::PointerEnter { .. }))
    );

    move_to(&mut session, 25, 25);
    session.roundtrip(&mut client);
    assert!(
        client
            .take_events()
            .contains(&Event::PointerEnter { x: 25.0, y: 25.0 })
    );
}

#[test]
fn only_layers_above_the_windows_can_demand_the_keyboard() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    let greedy = LayerShape {
        keyboard: KeyboardInteractivity::Exclusive,
        ..corner(Layer::Bottom)
    };
    session.open_layer(&mut client, &greedy, (100, 50), BLUE);

    assert!(
        !client.take_events().contains(&Event::KeyboardLeave),
        "a desktop widget is not a launcher"
    );
}

#[test]
fn a_panel_that_accepts_the_keyboard_has_it_from_a_click_until_a_click_elsewhere() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let search = LayerShape {
        keyboard: KeyboardInteractivity::OnDemand,
        ..panel()
    };
    let (surface, role) = session.open_layer(&mut client, &search, (WIDTH, 30), YELLOW);
    assert!(
        !client.take_events().contains(&Event::KeyboardLeave),
        "not until it is clicked"
    );

    // The panel reserved the top 30 pixels; the window opened below it.
    move_to(&mut session, 300, 12);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::KeyboardLeave) && events.contains(&Event::KeyboardEnter),
        "{events:?}"
    );

    move_to(&mut session, 50, 60);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);
    assert!(
        client.take_events().contains(&Event::KeyboardLeave),
        "back to the window"
    );

    // Clicked again, then closed: the window gets the keyboard back.
    move_to(&mut session, 300, 12);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);
    client.take_events();
    role.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::KeyboardEnter));
}
