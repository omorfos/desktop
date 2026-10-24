//! The switcher on `Alt+Tab`: the windows in the order they were last
//! used, walked through while Alt is held and chosen when it is let go.

use omorfos::Input;
use omorfos_config::{Action, Part, State, States, Theme};
use omorfos_core::geometry::Rect;
use smithay::input::keyboard::Keycode;

use crate::common::{Client, Event, Session, Toplevel, key};
use crate::windows::{BLUE, GREEN, RED};

const LEFT_BUTTON: u32 = 0x110;

fn press(session: &mut Session, code: Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

fn tap(session: &mut Session, code: Keycode) {
    press(session, code, true);
    press(session, code, false);
}

/// Holds Alt, taps Tab `times`, and lets go of Alt if `let_go`.
fn alt_tab(session: &mut Session, times: usize, let_go: bool) {
    press(session, key::ALT, true);
    for _ in 0..times {
        tap(session, key::TAB);
    }
    if let_go {
        press(session, key::ALT, false);
    }
    session.step();
}

fn focused(session: &Session) -> Option<String> {
    let desktop = session.compositor().desktop();
    let window = desktop.focused().and_then(|id| desktop.window(id));
    window.map(|window| window.app_id.clone())
}

/// The windows the switcher shows, by name, and the one picked.
fn shown(session: &Session) -> Option<(Vec<String>, String)> {
    let layout = session.compositor().switcher_layout()?;
    let desktop = session.compositor().desktop();
    let name = |id| {
        desktop
            .window(id)
            .map(|window| window.app_id.clone())
            .unwrap_or_default()
    };
    let names = layout.items.iter().map(|(id, _, _)| name(*id)).collect();
    let picked = layout
        .items
        .iter()
        .find(|(_, _, picked)| *picked)
        .map(|(id, _, _)| name(*id))?;
    Some((names, picked))
}

/// Three windows, opened in turn, the last with focus.
fn three() -> (Session, Client) {
    let mut session = Session::new();
    let mut client = session.connect();
    for (name, colour) in [("one", RED), ("two", BLUE), ("three", GREEN)] {
        session.open(&mut client, name, (200, 100), colour);
    }
    session.step();
    (session, client)
}

#[test]
fn one_alt_tab_goes_back_to_the_window_before_and_another_goes_back_again() {
    let (mut session, _client) = three();
    assert_eq!(focused(&session).as_deref(), Some("three"));
    alt_tab(&mut session, 1, true);
    assert_eq!(focused(&session).as_deref(), Some("two"));
    assert_eq!(shown(&session), None, "and the switcher has gone");
    alt_tab(&mut session, 1, true);
    assert_eq!(focused(&session).as_deref(), Some("three"));
}

#[test]
fn while_alt_is_held_tab_walks_every_window_and_the_order_stands_still() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    let names = |names: [&str; 3]| names.map(str::to_owned).to_vec();
    assert_eq!(
        shown(&session),
        Some((names(["three", "two", "one"]), "two".to_owned()))
    );
    tap(&mut session, key::TAB);
    session.step();
    assert_eq!(
        shown(&session),
        Some((names(["three", "two", "one"]), "one".to_owned()))
    );
    tap(&mut session, key::TAB);
    session.step();
    assert_eq!(
        shown(&session).map(|(_, picked)| picked).as_deref(),
        Some("three"),
        "round"
    );
    tap(&mut session, key::TAB);
    press(&mut session, key::ALT, false);
    session.step();
    // "two", passed over, was not used: going back once more is "three".
    assert_eq!(focused(&session).as_deref(), Some("two"));
    alt_tab(&mut session, 2, true);
    assert_eq!(focused(&session).as_deref(), Some("one"));
    alt_tab(&mut session, 1, true);
    assert_eq!(focused(&session).as_deref(), Some("two"));
}

#[test]
fn with_shift_it_opens_at_the_window_used_longest_ago_and_goes_back() {
    let (mut session, _client) = three();
    press(&mut session, key::ALT, true);
    press(&mut session, key::SHIFT, true);
    tap(&mut session, key::TAB);
    session.step();
    assert_eq!(
        shown(&session).map(|(_, picked)| picked).as_deref(),
        Some("one")
    );
    tap(&mut session, key::TAB);
    // Shift let go first leaves it open; Alt chooses.
    press(&mut session, key::SHIFT, false);
    session.step();
    assert_eq!(
        shown(&session).map(|(_, picked)| picked).as_deref(),
        Some("two")
    );
    press(&mut session, key::ALT, false);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("two"));
}

#[test]
fn escape_closes_it_as_it_was_and_the_arrows_and_enter_steer_it() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    tap(&mut session, key::ESCAPE);
    press(&mut session, key::ALT, false);
    session.step();
    assert_eq!(
        focused(&session).as_deref(),
        Some("three"),
        "nothing chosen"
    );
    assert_eq!(shown(&session), None);

    alt_tab(&mut session, 1, false);
    for (code, picked) in [
        (key::RIGHT, "one"),
        (key::DOWN, "three"),
        (key::LEFT, "one"),
        (key::UP, "two"),
    ] {
        tap(&mut session, code);
        session.step();
        assert_eq!(
            shown(&session).map(|(_, picked)| picked).as_deref(),
            Some(picked)
        );
    }
    tap(&mut session, key::RIGHT);
    tap(&mut session, key::RETURN);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("one"), "chosen by Enter");
    // Letting go of Alt afterwards does nothing more.
    press(&mut session, key::ALT, false);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("one"));
}

#[test]
fn opened_by_something_other_than_a_key_it_waits_for_enter() {
    let (mut session, _client) = three();
    session.compositor_mut().perform(&Action::SwitchWindows);
    tap(&mut session, key::ALT);
    session.step();
    assert_eq!(
        shown(&session).map(|(_, picked)| picked).as_deref(),
        Some("two"),
        "still open"
    );
    session.compositor_mut().perform(&Action::SwitchWindowsBack);
    tap(&mut session, key::RETURN);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("three"));
}

#[test]
fn with_no_windows_there_is_nothing_to_open() {
    let mut session = Session::new();
    alt_tab(&mut session, 1, false);
    assert_eq!(shown(&session), None);
    press(&mut session, key::ALT, false);
}

/// The middle of where a window's picture goes in its place.
fn picture(place: Rect, theme: &Theme) -> (i32, i32) {
    let style = theme.style(Part::SwitcherItem, States::NONE);
    let inset = style.inset();
    let width = place.size.width - inset.across();
    let tall = width * 10 / 16;
    let x = place.origin.x + (inset.left + width / 2).cast_signed();
    let y = place.origin.y + (inset.top + tall / 2).cast_signed();
    (x, y)
}

#[test]
fn each_window_is_shown_small_in_its_place_and_the_one_picked_is_set_apart() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    let layout = session.compositor().switcher_layout().unwrap();
    let frame = session.draw();
    let theme = Theme::default();
    for ((_, place, _), colour) in layout.items.iter().zip([GREEN, BLUE, RED]) {
        let (x, y) = picture(*place, &theme);
        assert_eq!(frame.at(x, y), colour, "{place:?}");
    }
    let picked = layout
        .items
        .iter()
        .find(|(_, _, picked)| *picked)
        .unwrap()
        .1;
    let accent = theme
        .style(Part::SwitcherItem, States::NONE.with(State::Active))
        .border
        .left
        .color;
    let edge = frame.at(
        picked.origin.x,
        picked.origin.y + picked.size.height.cast_signed() / 2,
    );
    let argb = u32::from_be_bytes([accent.alpha, accent.red, accent.green, accent.blue]);
    assert_eq!(edge, argb, "the one picked has the accent round it");
    // Its picture is kept up to date while it is shown.
    session.send_frames(std::time::Duration::from_millis(16));
    press(&mut session, key::ALT, false);
}

#[test]
fn a_private_window_is_shown_by_its_title_and_not_its_picture() {
    let rules = "rules:\n  - match: { app_id: two }\n    private: true\n";
    let mut session = Session::with_config(&[("rules.yaml", rules)]);
    let mut client = session.connect();
    session.open(&mut client, "one", (200, 100), RED);
    session.open(&mut client, "two", (200, 100), BLUE);
    alt_tab(&mut session, 1, false);
    let layout = session.compositor().switcher_layout().unwrap();
    let frame = session.draw();
    let theme = Theme::default();
    let colours: Vec<u32> = layout
        .items
        .iter()
        .map(|(_, place, _)| {
            let (x, y) = picture(*place, &theme);
            frame.at(x, y)
        })
        .collect();
    assert_eq!(colours[1], RED, "the other is shown");
    assert_ne!(colours[0], BLUE, "the private one is not");
}

#[test]
fn a_click_on_a_window_chooses_it_and_anywhere_else_closes_the_switcher() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    let layout = session.compositor().switcher_layout().unwrap();
    let last = layout.items[2].1;
    let click = |session: &mut Session, (x, y): (i32, i32)| {
        let compositor = session.compositor_mut();
        compositor.input(
            Input::Motion {
                x: f64::from(x),
                y: f64::from(y),
            },
            0,
        );
        compositor.input(
            Input::Button {
                button: LEFT_BUTTON,
                pressed: true,
            },
            0,
        );
        compositor.input(
            Input::Button {
                button: LEFT_BUTTON,
                pressed: false,
            },
            0,
        );
        session.step();
    };
    click(&mut session, (last.origin.x + 20, last.origin.y + 20));
    assert_eq!(focused(&session).as_deref(), Some("one"));
    press(&mut session, key::ALT, false);

    alt_tab(&mut session, 1, false);
    click(&mut session, (2, 2));
    assert_eq!(shown(&session), None);
    assert_eq!(focused(&session).as_deref(), Some("one"), "nothing chosen");
    press(&mut session, key::ALT, false);
}

#[test]
fn a_window_that_closes_while_it_is_open_is_taken_out() {
    let mut session = Session::new();
    let mut client = session.connect();
    let one = session.open(&mut client, "one", (200, 100), RED);
    let two = session.open(&mut client, "two", (200, 100), BLUE);
    alt_tab(&mut session, 1, false);
    let close = |session: &mut Session, client: &mut Client, window: Toplevel| {
        window.role.destroy();
        window.xdg_surface.destroy();
        window.surface.destroy();
        session.roundtrip(client);
        session.step();
    };
    close(&mut session, &mut client, one);
    assert_eq!(
        shown(&session),
        Some((vec!["two".to_owned()], "two".to_owned()))
    );
    close(&mut session, &mut client, two);
    assert_eq!(shown(&session), None, "none left");
    press(&mut session, key::ALT, false);
}

#[test]
fn a_minimized_window_chosen_comes_back_with_focus() {
    let (mut session, _client) = three();
    session.compositor_mut().perform(&Action::Minimize);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("two"));
    // "three" was used before "two" took over.
    alt_tab(&mut session, 1, true);
    assert_eq!(focused(&session).as_deref(), Some("three"));
}

#[test]
fn locking_the_session_closes_the_switcher() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    let _locked = crate::lock::lock(&mut session);
    session.step();
    assert_eq!(shown(&session), None);
}

#[test]
fn a_button_let_go_while_it_is_open_is_its_and_leaves_it_open() {
    let (mut session, _client) = three();
    let button = |pressed| Input::Button {
        button: LEFT_BUTTON,
        pressed,
    };
    session.compositor_mut().input(button(true), 0);
    alt_tab(&mut session, 1, false);
    session.compositor_mut().input(button(false), 0);
    session.step();
    let picked = shown(&session).map(|(_, picked)| picked);
    assert_eq!(picked.as_deref(), Some("two"));
    press(&mut session, key::ALT, false);
}

#[test]
fn shift_pressed_and_let_go_while_alt_is_held_turns_it_round_each_time() {
    let (mut session, _client) = three();
    alt_tab(&mut session, 1, false);
    let picked = |session: &Session| shown(session).map(|(_, picked)| picked);
    assert_eq!(picked(&session).as_deref(), Some("two"));
    press(&mut session, key::SHIFT, true);
    tap(&mut session, key::TAB);
    session.step();
    assert_eq!(picked(&session).as_deref(), Some("three"), "back");
    press(&mut session, key::SHIFT, false);
    tap(&mut session, key::TAB);
    tap(&mut session, key::TAB);
    session.step();
    assert_eq!(picked(&session).as_deref(), Some("one"), "on again");
    press(&mut session, key::ALT, false);
    session.step();
    assert_eq!(focused(&session).as_deref(), Some("one"));
}

#[test]
fn while_it_is_open_no_window_has_the_keyboard_so_none_sees_alt_let_go() {
    let (mut session, mut client) = three();
    session.roundtrip(&mut client);
    client.take_events();
    alt_tab(&mut session, 1, false);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(events.contains(&Event::KeyboardLeave), "{events:?}");
    press(&mut session, key::ALT, false);
    session.roundtrip(&mut client);
    let events = client.take_events();
    // Alt went down for the window that had focus, and only the window
    // chosen hears of the keyboard again, with nothing held.
    let alt = key::ALT.raw() - 8;
    assert!(
        !events.contains(&Event::Key {
            code: alt,
            pressed: false
        }),
        "{events:?}"
    );
    assert!(events.contains(&Event::KeyboardEnter), "{events:?}");
    assert_eq!(focused(&session).as_deref(), Some("two"));
}

const YELLOW: u32 = 0xffff_ff00;
const CYAN: u32 = 0xff00_ffff;
const MAGENTA: u32 = 0xffff_00ff;

/// Where programs install what they share, with an icon of each of
/// `icons`, by name and colour, and a desktop entry for each of `entries`,
/// by identifier and the icon it names.
#[expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]
pub(crate) fn installed(icons: &[(&str, u32)], entries: &[(&str, &str)]) -> tempfile::TempDir {
    let data = tempfile::tempdir().unwrap();
    let apps = data.path().join("icons/hicolor/scalable/apps");
    std::fs::create_dir_all(&apps).unwrap();
    for (name, colour) in icons {
        let fill = format!("#{:06x}", colour & 0x00ff_ffff);
        let drawing = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\">\
             <rect width=\"16\" height=\"16\" fill=\"{fill}\"/></svg>"
        );
        std::fs::write(apps.join(format!("{name}.svg")), drawing).unwrap();
    }
    let applications = data.path().join("applications");
    std::fs::create_dir_all(&applications).unwrap();
    for (app_id, icon) in entries {
        let entry = format!("[Desktop Entry]\nName={app_id}\nIcon={icon}\n");
        std::fs::write(applications.join(format!("{app_id}.desktop")), entry).unwrap();
    }
    data
}

/// The middle of where a window's icon goes beside its title.
fn beside_title(place: Rect, theme: &Theme) -> (i32, i32) {
    let style = theme.style(Part::SwitcherItem, States::NONE);
    let inset = style.inset();
    let tall = theme.font(Part::SwitcherTitle, States::NONE).size.get() * 5 / 3;
    let x = place.origin.x + (inset.left + tall / 2).cast_signed();
    let bottom = place.origin.y + (place.size.height - inset.bottom).cast_signed();
    (x, bottom - (tall / 2).cast_signed())
}

/// The colours where each window's icon goes beside its title, and where
/// its picture goes, in the order the switcher shows them.
#[expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]
fn icons_shown(session: &mut Session) -> Vec<(u32, u32)> {
    let layout = session.compositor().switcher_layout().unwrap();
    let frame = session.draw();
    let theme = Theme::default();
    let at = |(x, y)| frame.at(x, y);
    let places = layout.items.iter().map(|(_, place, _)| *place);
    places
        .map(|place| (at(beside_title(place, &theme)), at(picture(place, &theme))))
        .collect()
}

#[test]
fn a_window_has_its_applications_icon_from_its_desktop_entry_or_by_its_name() {
    let data = installed(
        &[("org.example.One", YELLOW), ("two", CYAN)],
        &[("one", "org.example.One")],
    );
    let (mut session, _client) = three();
    session
        .compositor_mut()
        .set_data_directories(vec![data.path().to_owned()]);
    alt_tab(&mut session, 1, false);
    // In the order of use: three, two, one. Three has none.
    let shown = icons_shown(&mut session);
    assert_eq!(shown[1], (CYAN, BLUE), "by its own name");
    assert_eq!(shown[2], (YELLOW, RED), "named by its desktop entry");
    assert_ne!(shown[0].0, YELLOW);
    assert_ne!(shown[0].0, CYAN);
    press(&mut session, key::ALT, false);
}

#[test]
fn a_program_gives_its_window_an_icon_by_name_or_as_pixels_but_not_as_a_path() {
    let data = installed(&[("org.example.Given", YELLOW), ("one", CYAN)], &[]);
    let path = data
        .path()
        .join("icons/hicolor/scalable/apps/org.example.Given.svg");
    let mut session = Session::new();
    session
        .compositor_mut()
        .set_data_directories(vec![data.path().to_owned()]);
    let mut client = session.connect();
    session.roundtrip(&mut client);
    assert_eq!(
        client.state.icon_sizes,
        [64],
        "the size it is drawn largest"
    );
    let one = session.open(&mut client, "one", (200, 100), RED);
    let two = session.open(&mut client, "two", (200, 100), BLUE);
    let three = session.open(&mut client, "three", (200, 100), GREEN);
    // A name a theme has is used before the pixels; a name it has not, or
    // a path, is passed over for them.
    client.set_icon(&one, Some("org.example.Given"), Some((32, MAGENTA)));
    client.set_icon(&two, Some("org.example.Missing"), Some((32, MAGENTA)));
    client.set_icon(&three, path.to_str(), Some((48, CYAN)));
    session.roundtrip(&mut client);
    alt_tab(&mut session, 1, false);
    let shown = icons_shown(&mut session);
    assert_eq!(
        shown[0],
        (CYAN, GREEN),
        "three, its pixels and not the file"
    );
    assert_eq!(shown[1], (MAGENTA, BLUE), "two");
    assert_eq!(shown[2], (YELLOW, RED), "one, by name");
    // An icon taken back leaves the application's.
    client.set_icon(&one, None, None);
    session.roundtrip(&mut client);
    session.step();
    assert_eq!(icons_shown(&mut session)[2], (CYAN, RED));
    press(&mut session, key::ALT, false);
}

#[test]
fn a_program_that_destroys_its_icons_pixels_keeps_its_window_with_its_applications_icon() {
    let data = installed(&[("one", CYAN)], &[]);
    let mut session = Session::new();
    session
        .compositor_mut()
        .set_data_directories(vec![data.path().to_owned()]);
    let mut client = session.connect();
    let one = session.open(&mut client, "one", (200, 100), RED);
    let two = session.open(&mut client, "two", (200, 100), BLUE);
    // As programs built on Chromium do: the buffer destroyed before the
    // icon, which is kept. The second of a size takes the first's place.
    let (manager, icon) = client.icon();
    let (first, second) = (
        client.buffer((32, 32), MAGENTA),
        client.buffer((32, 32), YELLOW),
    );
    icon.add_buffer(&first, 1);
    icon.add_buffer(&second, 1);
    manager.set_icon(&one.role, Some(&icon));
    one.surface.commit();
    session.roundtrip(&mut client);
    alt_tab(&mut session, 1, false);
    assert_eq!(icons_shown(&mut session)[1], (YELLOW, RED), "the second");
    press(&mut session, key::ALT, false);
    first.destroy();
    second.destroy();
    session.roundtrip(&mut client);
    alt_tab(&mut session, 1, false);
    assert_eq!(
        icons_shown(&mut session)[0],
        (CYAN, RED),
        "its application's"
    );
    press(&mut session, key::ALT, false);
    // An icon taken back with none, and the protocol let go of.
    manager.set_icon(&two.role, None);
    two.surface.commit();
    manager.destroy();
    session.roundtrip(&mut client);
    assert!(
        session.compositor().desktop().frames().count() == 2,
        "still there"
    );
}

#[test]
fn an_icon_changed_once_set_or_given_pixels_not_shared_or_not_square_is_refused() {
    let mut session = Session::new();
    let changed = session.connect();
    let window = changed.toplevel("one", "One");
    let (manager, icon) = changed.icon();
    manager.set_icon(&window.role, Some(&icon));
    icon.set_name("later".to_owned());
    let skewed = session.connect();
    let (_, icon) = skewed.icon();
    icon.add_buffer(&skewed.buffer((32, 16), RED), 1);
    let elsewhere = session.connect();
    let (_, icon) = elsewhere.icon();
    icon.add_buffer(&elsewhere.single_pixel(), 1);
    let mut logged = Vec::new();
    for client in [&changed, &skewed, &elsewhere] {
        client.flush();
    }
    session.eventually(|session| {
        logged.extend(session.compositor_mut().take_logged());
        logged.len() == 3
    });
    assert!(
        logged.iter().any(|said| said.contains("already been set")),
        "{logged:?}"
    );
    assert!(
        logged.iter().any(|said| said.contains("square")),
        "{logged:?}"
    );
}

#[test]
fn a_private_window_is_shown_by_its_icon_large_where_its_picture_would_be() {
    let data = installed(&[("one", YELLOW)], &[]);
    let rules = "rules:\n  - match: { app_id: one }\n    private: true\n  - match: { app_id: two }\n    private: true\n";
    let mut session = Session::with_config(&[("rules.yaml", rules)]);
    session
        .compositor_mut()
        .set_data_directories(vec![data.path().to_owned()]);
    let mut client = session.connect();
    session.open(&mut client, "one", (200, 100), RED);
    let two = session.open(&mut client, "two", (200, 100), BLUE);
    client.set_icon(&two, None, Some((32, MAGENTA)));
    session.roundtrip(&mut client);
    alt_tab(&mut session, 1, false);
    let shown = icons_shown(&mut session);
    assert_eq!(shown, [(MAGENTA, MAGENTA), (YELLOW, YELLOW)]);
    press(&mut session, key::ALT, false);
}
