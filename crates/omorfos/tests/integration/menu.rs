//! The root menu.

use omorfos::Input;
use omorfos_config::Action;

use std::time::Duration;

use smithay::input::keyboard::Keycode;

use crate::common::{Event, Session, between, half_way, key};
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
fn a_row_gets_to_the_colour_of_being_pointed_at_over_the_time_the_theme_gives() {
    let theme = "menu-item { background: #ff0000; transition: 100ms linear; }\n\
                 menu-item:hover { background: #0000ff; }\n";
    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    let (red, blue) = (0xffff_0000, 0xff00_00ff);
    open_menu(&mut session);
    session.step();
    assert_eq!(session.draw().at(510, 218), red);

    let start = session.now;
    move_to(&mut session, 400, 218);
    session.step();
    assert_eq!(session.draw().at(510, 218), red, "setting out");

    session.now = start + Duration::from_millis(50);
    session.step();
    let row = session.draw().at(510, 218);
    assert!(half_way(row), "{row:#x}");
    assert_eq!(
        session.draw().at(510, 255),
        red,
        "the other row is as it was"
    );

    session.now = start + Duration::from_millis(100);
    session.step();
    assert_eq!(session.draw().at(510, 218), blue, "arrived");
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");
}

#[test]
fn the_menu_comes_onto_the_screen_over_the_time_the_theme_gives() {
    let theme = "menu { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    open_menu(&mut session);
    session.step();
    assert_eq!(session.draw().at(510, 255), BACKGROUND, "not there yet");

    session.advance(Duration::from_millis(50));
    let half = session.draw().at(510, 255);
    assert!(between(half, BACKGROUND, PANEL), "{half:#x}");

    session.advance(Duration::from_millis(50));
    assert_eq!(session.draw().at(510, 255), PANEL);
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");
}

#[test]
fn the_menu_goes_from_the_screen_over_the_time_the_theme_gives() {
    let theme = "menu { transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    open_menu(&mut session);
    session.step();
    session.advance(Duration::from_millis(100));
    assert_eq!(session.draw().at(510, 255), PANEL);

    for pressed in [true, false] {
        let code = key::ESCAPE;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    assert_eq!(session.draw().at(510, 255), PANEL, "still all there");

    session.advance(Duration::from_millis(50));
    let half = session.draw().at(510, 255);
    assert!(between(half, BACKGROUND, PANEL), "{half:#x}");

    session.advance(Duration::from_millis(50));
    assert_eq!(session.draw().at(510, 255), BACKGROUND, "gone");
    session.step();
    assert!(!session.draw().damaged, "and not drawn again");
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

fn press(session: &mut Session, codes: &[Keycode]) {
    for code in codes {
        for pressed in [true, false] {
            let code = *code;
            session
                .compositor_mut()
                .input(Input::Key { code, pressed }, 0);
        }
    }
    session.step();
}

#[test]
fn the_arrows_go_through_the_rows_and_enter_chooses_one() {
    let mut session = session();
    open_menu(&mut session);
    assert_eq!(session.draw().at(510, 218), PANEL, "nothing is highlighted");

    press(&mut session, &[key::DOWN]);
    assert_eq!(session.draw().at(510, 218), ACCENT, "the first row");

    press(&mut session, &[key::DOWN]);
    let picture = session.draw();
    assert_eq!(picture.at(510, 218), PANEL);
    assert_eq!(picture.at(510, 255), ACCENT, "past the separator");

    press(&mut session, &[key::UP, key::RETURN]);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
    assert_eq!(session.draw().at(510, 218), BACKGROUND, "the menu is gone");
}

#[test]
fn right_goes_into_a_submenu_and_left_comes_back_out() {
    let mut session = session();
    open_menu(&mut session);

    press(&mut session, &[key::UP, key::RIGHT]);
    let picture = session.draw();
    assert_eq!(picture.at(290, 255), ACCENT, "its first row is highlighted");
    assert_eq!(picture.at(510, 255), ACCENT, "and the row it belongs to");

    press(&mut session, &[key::LEFT]);
    let picture = session.draw();
    assert_eq!(picture.at(290, 255), BACKGROUND, "the submenu is gone");
    assert_eq!(picture.at(510, 255), ACCENT);

    press(&mut session, &[key::RETURN, key::RETURN]);
    assert!(!session.compositor().is_running(), "Quit was chosen");
}

#[test]
fn the_keys_that_steer_the_menu_are_kept_from_the_focused_window() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    client.take_events();

    press(&mut session, &[key::DOWN, key::RETURN]);
    session.roundtrip(&mut client);

    let events = client.take_events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Key { .. })),
        "{events:?}"
    );
}

#[test]
fn an_arrow_with_a_modifier_held_is_not_the_menus() {
    let mut session = session();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    open_menu(&mut session);
    client.take_events();

    session.compositor_mut().input(
        Input::Key {
            code: key::SHIFT,
            pressed: true,
        },
        0,
    );
    press(&mut session, &[key::DOWN]);
    session.roundtrip(&mut client);

    assert_eq!(session.draw().at(510, 218), PANEL, "nothing is highlighted");
    assert!(client.take_events().contains(&Event::Key {
        code: 108,
        pressed: true
    }));
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
