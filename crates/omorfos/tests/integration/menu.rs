//! The root menu.

use omorfos::Input;
use omorfos_config::Action;

use std::time::Duration;

use smithay::input::keyboard::Keycode;

use crate::common::{Event, Session, WIDTH, between, half_way, key};
use crate::windows::{BACKGROUND, RED, shaded};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;

/// Colours of the default theme, as `0xAARRGGBB`.
/// A row under the pointer, raised.
const RAISED: u32 = 0xff24_3364;
const PANEL: u32 = 0xff14_1c38;

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
/// "Second workspace" at y 201..233, the separator, and "More" at 242..274.
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
    assert_eq!(picture.at(530, 220), PANEL, "a panel 240 pixels wide");
    for x in [541, 299] {
        let beside = picture.at(x, 220);
        assert!(shaded(beside, BACKGROUND), "at {x}: {beside:#x}");
    }
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

    assert_eq!(session.draw().at(510, 218), RAISED);
}

#[test]
fn the_first_and_last_rows_reach_the_menus_border_unless_the_theme_pads_it() {
    // The built-in theme gives the menu a border of 1 and no padding, so
    // "Second workspace" is at 201..233 and "More" at 242..274, in a
    // panel 200..275.
    let mut session = session();
    open_menu(&mut session);
    move_to(&mut session, 400, 201);
    session.step();
    assert_eq!(session.draw().at(510, 201), RAISED, "under the top border");
    move_to(&mut session, 400, 273);
    session.step();
    assert_eq!(session.draw().at(510, 273), RAISED, "over the bottom one");
    assert_ne!(session.draw().at(510, 274), RAISED, "which is not a row");

    // Padded, the rows begin and end that far in, at the sides too.
    let theme = "menu { padding: 6px 8px 10px; }";
    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    open_menu(&mut session);
    move_to(&mut session, 400, 206);
    session.step();
    assert_eq!(session.draw().at(510, 206), PANEL, "in the padding");
    move_to(&mut session, 400, 207);
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(510, 207), RAISED, "the first row");
    assert_eq!(picture.at(304, 207), PANEL, "but for the padding beside it");
    move_to(&mut session, 400, 279);
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(510, 279), RAISED, "the last row");
    assert_eq!(picture.at(510, 280), PANEL, "and the padding below it");
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
fn the_menu_grows_into_its_place_as_it_comes_and_shrinks_as_it_goes() {
    // Seen whole as it comes and goes, so that only its size changes, and
    // with no shadow, so that its edge is its own.
    let theme = "menu { transition: 100ms linear; box-shadow: none; }\n\
                 menu:closing { opacity: 1; scale: 50%; }\n\
                 @starting-style { menu { opacity: 1; } }\n";
    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    open_menu(&mut session);
    session.step();
    session.advance(Duration::from_millis(100));
    // The last column of the menu, once it is there, along the separator.
    let picture = session.draw();
    let edge = (300..WIDTH)
        .take_while(|x| picture.at(*x, 233) != BACKGROUND)
        .last()
        .unwrap();
    let middle = i32::midpoint(300, edge);
    assert!(edge > 400, "{edge}");

    let mut session = Session::with_config(&[("menu.yaml", MENU), ("theme.css", theme)]);
    open_menu(&mut session);
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(edge - 2, 233), BACKGROUND, "96% of its size");
    assert_ne!(picture.at(middle, 233), BACKGROUND);
    session.advance(Duration::from_millis(100));
    assert_ne!(session.draw().at(edge - 2, 233), BACKGROUND, "all of it");

    for pressed in [true, false] {
        let code = key::ESCAPE;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    // Three quarters as far from its middle.
    let shrunk = edge - (edge - middle) / 4;
    assert_eq!(picture.at(shrunk + 3, 233), BACKGROUND, "75% of it");
    assert_ne!(picture.at(shrunk - 3, 233), BACKGROUND);
    assert_ne!(picture.at(middle, 233), BACKGROUND);
    session.advance(Duration::from_millis(50));
    assert_eq!(session.draw().at(middle, 233), BACKGROUND, "gone");
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

    // The screen is 640 wide and the menu ends at 540, so there is no room
    // for another 240 pixel panel on the right; it opens on the left.
    move_to(&mut session, 400, 255);
    session.step();
    assert_eq!(
        session.draw().at(290, 255),
        PANEL,
        "a second panel beside the row"
    );
    let beside = session.draw().at(50, 255);
    assert!(shaded(beside, BACKGROUND), "{beside:#x}");

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
    assert_eq!(session.draw().at(510, 218), RAISED, "the first row");

    press(&mut session, &[key::DOWN]);
    let picture = session.draw();
    assert_eq!(picture.at(510, 218), PANEL);
    assert_eq!(picture.at(510, 255), RAISED, "past the separator");

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
    assert_eq!(picture.at(290, 255), RAISED, "its first row is highlighted");
    assert_eq!(picture.at(510, 255), RAISED, "and the row it belongs to");

    press(&mut session, &[key::LEFT]);
    let picture = session.draw();
    let gone = picture.at(290, 255);
    assert!(shaded(gone, BACKGROUND), "the submenu is gone: {gone:#x}");
    assert_eq!(picture.at(510, 255), RAISED);

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

#[test]
fn an_action_from_a_key_closes_the_menu_and_does_not_carry_it_along() {
    let mut session = session();
    open_menu(&mut session);
    assert_eq!(session.draw().at(510, 218), PANEL);

    // Going to another workspace, by its key say, leaves the menu behind.
    session.compositor_mut().perform(&Action::NewWorkspace);
    session.step();
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
    assert_eq!(session.draw().at(510, 218), BACKGROUND, "the menu is gone");

    // Asked for again there, it opens as it does anywhere.
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    assert_ne!(session.draw().at(510, 218), BACKGROUND);
}

#[test]
fn moving_from_row_to_row_draws_the_menu_again_and_not_its_shadow() {
    // The built-in theme's menu casts a shadow 32 pixels soft, round the
    // panel, which is from 300 to 540 across; its rows are 32 high.
    let plain = "items:\n  - { label: One, action: quit }\n  - { label: Two, action: quit }\n";
    let mut session = Session::with_config(&[("menu.yaml", plain)]);
    open_menu(&mut session);
    move_to(&mut session, 400, 218);
    session.step();
    drop(session.draw());

    move_to(&mut session, 400, 246);
    session.step();
    let picture = session.draw();
    assert!(picture.damaged, "the rows are drawn again");
    for damage in &picture.damage {
        let (left, right) = (damage.loc.x, damage.loc.x + damage.size.w);
        assert!(left >= 300 && right <= 540, "{damage:?}");
    }
}

/// An entry for each kind of thing that can stand before a label.
const MARKED: &str = "
items:
  - { label: Terminal, action: { spawn: [/usr/bin/foot] } }
  - { label: Files, action: { spawn: [nautilus] }, icon: org.example.Files }
  - { label: Run, action: show_launcher }
  - { label: Stay awake, action: stay_awake }
  - { label: Second workspace, action: { workspace: 2 } }
";

/// How many pixels of where a sign goes, before the label of the row
/// `which`, are not the menu's own colour. The menu opened by `open_menu`
/// has a border of 1 and its rows, 32 tall, a padding of 8 at their start,
/// so the sign is drawn at 309 to 325 across, and 8 below the top of its
/// row.
fn signed(picture: &crate::common::Picture, which: i32) -> usize {
    let top = 201 + which * 32 + 8;
    let place = (309..325).flat_map(|x| (top..top + 16).map(move |y| (x, y)));
    place.filter(|&(x, y)| picture.at(x, y) != PANEL).count()
}

/// The colour just inside the square round the sign of the row `which`,
/// which the theme reaches 3 out from the sign.
fn pressed(picture: &crate::common::Picture, which: i32) -> u32 {
    picture.at(307, 201 + which * 32 + 16)
}

#[test]
fn each_entry_has_a_sign_of_what_it_does_or_the_icon_of_the_program_it_starts() {
    let (yellow, cyan) = (0xffff_ff00, 0xff00_ffff);
    let icons = [("org.example.Foot", yellow), ("org.example.Files", cyan)];
    let data = crate::switcher::installed(&icons, &[("foot", "org.example.Foot")]);
    let mut session = Session::with_config(&[("menu.yaml", MARKED)]);
    session
        .compositor_mut()
        .set_data_directories(vec![data.path().to_owned()]);
    open_menu(&mut session);

    let picture = session.draw();
    assert_eq!(
        picture.at(317, 217),
        yellow,
        "the icon the terminal's desktop entry names, found by the program's name"
    );
    assert_eq!(picture.at(317, 249), cyan, "the icon the menu names");
    assert!(signed(&picture, 2) > 10, "a sign for the launcher");
    assert!(signed(&picture, 3) > 10, "and for staying awake");
    assert_eq!(
        pressed(&picture, 3),
        PANEL,
        "not pressed in while it may sleep"
    );
    assert_eq!(signed(&picture, 4), 0, "and nothing where there is no sign");
    let gap = (325..333).flat_map(|x| (201..361).map(move |y| (x, y)));
    let gap: Vec<u32> = gap.map(|(x, y)| picture.at(x, y)).collect();
    assert!(
        gap.iter().all(|&pixel| pixel == PANEL),
        "room before labels"
    );

    // Kept awake, the entry that lets it sleep again is pressed in.
    press(&mut session, &[key::ESCAPE]);
    session.compositor_mut().perform(&Action::StayAwake);
    open_menu(&mut session);
    assert_ne!(pressed(&session.draw(), 3), PANEL, "pressed in");
}
