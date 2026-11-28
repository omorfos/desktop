//! Popups that grab: menus that close when the user clicks away.

use omorfos::Input;
use wayland_protocols::xdg::shell::client::xdg_positioner;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, HEIGHT, LayerShape, Session, Toplevel, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const LEFT: u32 = 0x110;

fn click_at(session: &mut Session, x: i32, y: i32) {
    session.compositor_mut().input(
        Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        },
        0,
    );
    session.compositor_mut().input(
        Input::Button {
            button: LEFT,
            pressed: true,
        },
        1,
    );
    session.compositor_mut().input(
        Input::Button {
            button: LEFT,
            pressed: false,
        },
        2,
    );
    session.step();
}

/// Clicks at (`x`, `y`), on one of `client`'s surfaces, and gives the
/// serial of the press, as a program opens a menu with.
fn press(session: &mut Session, client: &mut Client, x: i32, y: i32) -> u32 {
    click_at(session, x, y);
    session.roundtrip(client);
    client.take_events();
    client.state.pressed
}

/// Opens a 200 by 100 window with a 50 by 30 menu at (10, 20) that has
/// asked to be dismissed on a click elsewhere.
fn window_with_menu(session: &mut Session, client: &mut Client) -> Toplevel {
    let window = session.open(client, "app", (200, 100), RED);
    let pressed = press(session, client, 150, 80);
    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), pressed);
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
    let pressed = press(&mut session, &mut client, 100, 80);
    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), pressed);
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
    let pressed = press(&mut session, &mut client, 150, 80);

    let (surface, popup, _positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(&mut client);
    client.fill(&surface, (50, 30), GREEN);
    popup.grab(client.seat(), pressed);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::KeyboardLeave) && events.contains(&Event::KeyboardEnter),
        "{events:?}"
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
    // A menu opened from a menu, by the press that opened the first.
    popup.grab(client.seat(), client.state.pressed);
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
fn a_menu_opened_from_a_menu_is_drawn_over_it_and_both_over_the_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    let (menu, menu_xdg, _menu) = client.popup_of(&window.xdg_surface, (10, 20), (60, 40));
    session.roundtrip(&mut client);
    client.fill(&menu, (60, 40), GREEN);
    session.roundtrip(&mut client);
    // A submenu from the menu's right edge, partly over it.
    let (submenu, _, _submenu) = client.popup_of(&menu_xdg, (50, 10), (60, 40));
    session.roundtrip(&mut client);
    client.fill(&submenu, (60, 40), BLUE);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let frame = session.compositor().desktop().frames().next().unwrap();
    let origin = frame.client().origin;
    let at = |x: i32, y: i32| (origin.x + x, origin.y + y);
    let picture = session.draw();
    let (x, y) = at(15, 25);
    assert_eq!(picture.at(x, y), GREEN, "the menu, over the window");
    let (x, y) = at(65, 35);
    assert_eq!(picture.at(x, y), BLUE, "the submenu, over the menu");
    let (x, y) = at(115, 60);
    assert_eq!(picture.at(x, y), BLUE, "and over the window beyond it");
    let (x, y) = at(150, 90);
    assert_eq!(picture.at(x, y), RED, "the window, where neither is");
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
    let pressed = press(&mut session, &mut client, 110, 10);

    let (menu, popup) = client.layer_popup(&role, (100, 30), (80, 60));
    session.roundtrip(&mut client);
    client.fill(&menu, (80, 60), GREEN);
    popup.grab(client.seat(), pressed);
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

#[test]
fn a_menu_of_a_bar_along_the_bottom_is_turned_to_hang_above_it_and_slid_onto_the_screen() {
    let mut session = Session::new();
    let mut client = session.connect();
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let (_surface, role) = session.open_layer(&mut client, &panel, (WIDTH, 30), RED);
    let pressed = press(&mut session, &mut client, WIDTH - 20, HEIGHT - 10);

    // Hung below its bar it would be off the bottom of the screen, and to
    // the right of where it is asked for, off its right edge.
    let kept =
        xdg_positioner::ConstraintAdjustment::FlipY | xdg_positioner::ConstraintAdjustment::SlideX;
    let (menu, popup) = client.layer_popup_kept(&role, ((WIDTH - 40, 0), (80, 60)), kept);
    session.roundtrip(&mut client);
    client.fill(&menu, (80, 60), GREEN);
    popup.grab(client.seat(), pressed);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let picture = session.draw();
    assert_eq!(
        picture.at(WIDTH - 5, HEIGHT - 30 - 5),
        GREEN,
        "above the bar, at the edge"
    );
    assert_eq!(
        picture.at(WIDTH - 85, HEIGHT - 30 - 5),
        BACKGROUND,
        "and no further"
    );
}

#[test]
fn a_program_in_the_background_cannot_take_the_keyboard_with_a_menu() {
    let mut session = Session::new();
    let mut behind = session.connect();
    let hidden = session.open(&mut behind, "behind", (200, 100), RED);
    let mut typed_in = session.connect();
    session.open(&mut typed_in, "typed in", (100, 100), BLUE);
    typed_in.take_events();

    // A menu of a pixel, grabbing with a serial the compositor never gave
    // for a press to it.
    let (surface, popup, _positioner) = behind.popup(&hidden, (0, 0), (1, 1));
    session.roundtrip(&mut behind);
    behind.fill(&surface, (1, 1), GREEN);
    popup.grab(behind.seat(), 0);
    session.roundtrip(&mut behind);
    session.roundtrip(&mut behind);
    assert!(dismissed(&mut behind), "the menu is closed at once");

    session.compositor_mut().input(
        Input::Key {
            code: key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut behind);
    session.roundtrip(&mut typed_in);
    let pressed = Event::Key {
        code: 30,
        pressed: true,
    };
    assert!(!behind.take_events().contains(&pressed));
    assert!(typed_in.take_events().contains(&pressed));
}

#[test]
fn a_tooltip_that_will_not_go_takes_only_one_escape_and_only_a_panels_takes_any() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let bar = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    let escape = |session: &mut Session, app: &mut Client| {
        app.take_events();
        for pressed in [true, false] {
            let code = key::ESCAPE;
            session
                .compositor_mut()
                .input(Input::Key { code, pressed }, 0);
        }
        session.step();
        session.roundtrip(app);
        let code = key::ESCAPE.raw() - 8;
        app.take_events().contains(&Event::Key {
            code,
            pressed: true,
        })
    };
    // A program without the panel's role shows what it likes; Escape is
    // still the window's.
    let mut plain = session.connect_as(omorfos::Roles::NONE.with(omorfos::Roles::OVERLAY));
    let (_surface, role) = session.open_layer(&mut plain, &bar, (WIDTH, 30), BLUE);
    let (tip, _popup) = plain.layer_popup(&role, (100, 30), (80, 20));
    session.roundtrip(&mut plain);
    plain.fill(&tip, (80, 20), GREEN);
    session.roundtrip(&mut plain);
    assert!(escape(&mut session, &mut app), "the window's");
    // A panel's tooltip, which stays though it is told to go, takes the
    // first Escape and no more.
    let mut panel = session.connect_as(omorfos::Roles::PANEL);
    let (_surface, role) = session.open_layer(&mut panel, &bar, (WIDTH, 30), BLUE);
    let (tip, _popup) = panel.layer_popup(&role, (100, 30), (80, 20));
    session.roundtrip(&mut panel);
    panel.fill(&tip, (80, 20), GREEN);
    session.roundtrip(&mut panel);
    assert!(!escape(&mut session, &mut app), "taken for the tooltip");
    session.roundtrip(&mut panel);
    assert!(
        panel.take_events().contains(&Event::PopupDone),
        "told to go"
    );
    assert!(escape(&mut session, &mut app), "the window's again");
}
