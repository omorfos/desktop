//! The root menu.

use omorfos::Input;
use omorfos_config::Action;

use crate::common::{Event, Session, key};
use crate::windows::{BACKGROUND, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;

/// Colours of the default theme, as `0xAARRGGBB`.
const ACCENT: u32 = 0xff5b_9cf5;
const PANEL: u32 = 0xff23_262e;

/// Second / ---- / More > (Quit)
const MENU: &str = "
items:
  - { label: Second workspace, action: { workspace: 2 } }
  - separator
  - label: More
    items:
      - { label: Quit, action: quit }
";

fn session() -> Session {
    Session::with_config(&[("menu.yaml", MENU)])
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

/// Opens the menu with its corner at (300, 200). Its rows are then:
/// "Second workspace" at y 204..232, the separator, and "More" at 241..269.
fn open_menu(session: &mut Session) {
    move_to(session, 300, 200);
    click(session, RIGHT);
}

#[test]
fn a_right_click_on_the_desktop_opens_the_menu_at_the_pointer() {
    let mut session = session();
    assert_eq!(session.draw().at(400, 220), BACKGROUND);

    open_menu(&mut session);

    let picture = session.draw();
    assert_eq!(picture.at(510, 220), PANEL, "a panel 220 pixels wide");
    assert_eq!(picture.at(521, 220), BACKGROUND);
    assert_eq!(picture.at(299, 220), BACKGROUND);
}

#[test]
fn the_show_menu_action_opens_it_too() {
    let mut session = session();
    move_to(&mut session, 100, 100);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    assert_eq!(session.draw().at(310, 110), PANEL);
}

#[test]
fn the_row_under_the_pointer_is_highlighted() {
    let mut session = session();
    open_menu(&mut session);

    move_to(&mut session, 400, 218);
    session.step();

    assert_eq!(session.draw().at(510, 218), ACCENT);
}

#[test]
fn choosing_an_entry_performs_its_action_and_closes_the_menu() {
    let mut session = session();
    open_menu(&mut session);

    move_to(&mut session, 400, 218);
    click(&mut session, LEFT);

    assert_eq!(session.compositor().desktop().active_workspace(), 1);
    assert_eq!(session.draw().at(510, 218), BACKGROUND, "the menu is gone");
}

#[test]
fn a_submenu_opens_beside_its_row_and_its_entries_can_be_chosen() {
    let mut session = session();
    open_menu(&mut session);

    // The screen is 640 wide and the menu ends at 520, so there is no room
    // for another 220 pixel panel on the right; it opens on the left.
    move_to(&mut session, 400, 255);
    session.step();
    assert_eq!(
        session.draw().at(290, 255),
        PANEL,
        "a second panel beside the row"
    );
    assert_eq!(session.draw().at(70, 255), BACKGROUND);

    move_to(&mut session, 150, 255);
    click(&mut session, LEFT);
    assert!(!session.compositor().is_running(), "Quit was chosen");
}

#[test]
fn a_click_elsewhere_dismisses_the_menu_without_reaching_a_window() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    client.take_events();

    move_to(&mut session, 50, 50);
    click(&mut session, LEFT);
    session.roundtrip(&mut client);

    assert_eq!(session.draw().at(510, 220), BACKGROUND);
    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "{events:?}"
    );
}

#[test]
fn clicking_a_separator_keeps_the_menu_open() {
    let mut session = session();
    open_menu(&mut session);
    move_to(&mut session, 400, 236);
    click(&mut session, LEFT);
    assert_eq!(session.draw().at(510, 220), PANEL);
}

#[test]
fn escape_closes_the_menu_and_is_kept_from_the_focused_window() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    client.take_events();

    session.compositor_mut().input(
        Input::Key {
            code: key::ESCAPE,
            pressed: true,
        },
        0,
    );
    session.compositor_mut().input(
        Input::Key {
            code: key::ESCAPE,
            pressed: false,
        },
        0,
    );
    session.roundtrip(&mut client);

    assert_eq!(session.draw().at(510, 220), BACKGROUND);
    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Key { .. })),
        "{events:?}"
    );
}

#[test]
fn other_keys_still_reach_the_focused_window_while_the_menu_is_open() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    client.take_events();

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
}

#[test]
fn a_right_click_on_a_window_goes_to_the_window_not_the_menu() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    move_to(&mut session, 50, 50);
    click(&mut session, RIGHT);
    session.roundtrip(&mut client);

    assert!(client.take_events().contains(&Event::PointerButton {
        button: RIGHT,
        pressed: true
    }));
    assert_eq!(session.draw().at(260, 60), BACKGROUND, "no menu opened");
}

#[test]
fn the_menu_follows_a_theme_change_while_open() {
    let mut session = session();
    open_menu(&mut session);
    session.write_config(
        "theme.css",
        "menu { background: #00ff00; color: #ffffff; }\n",
    );
    session.compositor_mut().reload_config();
    session.step();
    assert_eq!(session.draw().at(510, 220), 0xff00_ff00);
}
