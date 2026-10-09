//! Popups that grab: menus that close when the user clicks away.

use omorfos::Input;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session, Toplevel, WIDTH, key};
use crate::windows::{BLUE, GREEN, RED};

const LEFT: u32 = 0x110;

fn click_at(session: &mut Session, x: i32, y: i32) {
    session.compositor.input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
    session.compositor.input(
        Input::Button {
            button: LEFT,
            pressed: true,
        },
        1,
    );
    session.compositor.input(
        Input::Button {
            button: LEFT,
            pressed: false,
        },
        2,
    );
    session.step();
}

/// Opens a 200 by 100 window with a 50 by 30 menu at (10, 20) that has
/// asked to be dismissed on a click elsewhere.
fn window_with_menu(session: &mut Session, client: &mut Client) -> Toplevel {
    let window = session.open(client, "app", (200, 100), RED);
    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), client.state.serial);
    session.roundtrip(client);
    session.roundtrip(client);
    client.take_events();
    window
}

fn dismissed(client: &mut Client) -> bool {
    client.take_events().contains(&Event::PopupDone)
}

#[test]
fn a_click_on_the_desktop_dismisses_an_open_menu() {
    let mut session = Session::new();
    let mut client = session.connect();
    window_with_menu(&mut session, &mut client);

    click_at(&mut session, 500, 400);
    session.roundtrip(&mut client);

    assert!(dismissed(&mut client));
}

#[test]
fn a_click_on_another_applications_window_dismisses_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    window_with_menu(&mut session, &mut client);
    let mut other = session.connect();
    session.open(&mut other, "other", (100, 100), BLUE);
    client.take_events();

    // The other window cascades to (32, 32); click a part of it that is
    // clear of the first window.
    click_at(&mut session, 120, 125);
    session.roundtrip(&mut client);

    assert!(dismissed(&mut client));
}

#[test]
fn a_click_inside_the_menu_or_its_own_window_leaves_it_open() {
    let mut session = Session::new();
    let mut client = session.connect();
    window_with_menu(&mut session, &mut client);

    click_at(&mut session, 20, 30);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(!events.contains(&Event::PopupDone), "{events:?}");
    assert!(
        events.contains(&Event::PointerButton {
            button: LEFT,
            pressed: true
        }),
        "the menu gets the click"
    );

    click_at(&mut session, 150, 80);
    session.roundtrip(&mut client);
    assert!(
        !dismissed(&mut client),
        "the application decides what a click on its own window means"
    );
}

#[test]
fn a_click_on_a_titlebar_dismisses_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "app", (200, 100), RED);
    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), client.state.serial);
    session.roundtrip(&mut client);
    client.take_events();

    click_at(&mut session, 100, 10);
    session.roundtrip(&mut client);

    assert!(
        dismissed(&mut client),
        "the titlebar is ours, not the application's"
    );
}

#[test]
fn an_open_menu_has_the_keyboard_and_hands_it_back() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), client.state.serial);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::KeyboardLeave) && events.contains(&Event::KeyboardEnter),
        "{events:?}"
    );

    session.compositor.input(
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

    // The application closes its own menu; the window has the keyboard again.
    popup.destroy();
    surface.destroy();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::KeyboardEnter));

    // With no menu left, a click away dismisses nothing.
    click_at(&mut session, 500, 400);
    session.roundtrip(&mut client);
    assert!(!dismissed(&mut client));
}

#[test]
fn nested_menus_are_dismissed_together() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = window_with_menu(&mut session, &mut client);
    let (surface, popup, _positioner) = client.popup(&window, (70, 20), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), BLUE);
    popup.grab(client.seat(), client.state.serial);
    session.roundtrip(&mut client);
    client.take_events();

    click_at(&mut session, 500, 400);
    session.roundtrip(&mut client);

    let done = client
        .take_events()
        .into_iter()
        .filter(|event| *event == Event::PopupDone)
        .count();
    assert_eq!(done, 2);
}

#[test]
fn a_panels_menu_is_shown_and_dismissed_like_any_other() {
    let mut session = Session::new();
    let mut client = session.connect();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let (_surface, role) = session.open_layer(&mut client, &panel, (WIDTH, 30), RED);

    let (menu, popup) = client.layer_popup(&role, (100, 30), (80, 60));
    session.roundtrip(&mut client);
    client.fill(&menu, (80, 60), GREEN);
    popup.grab(client.seat(), client.state.serial);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        session.draw().at(110, 50),
        GREEN,
        "the menu hangs below the panel"
    );
    client.take_events();

    click_at(&mut session, 110, 50);
    session.roundtrip(&mut client);
    assert!(!dismissed(&mut client), "a click on the menu itself");

    click_at(&mut session, 500, 400);
    session.roundtrip(&mut client);
    assert!(dismissed(&mut client));
}
