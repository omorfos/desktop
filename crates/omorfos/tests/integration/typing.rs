//! Typing through an input method, as one that writes a language with
//! more characters than keys does: the field says it is being typed into,
//! the input method types into it, takes the keys meanwhile, and shows
//! what it could mean beside the text.

use omorfos::{Input, Roles};
use omorfos_config::Action;

use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, HEIGHT, LayerShape, Session, WIDTH, key};
use crate::windows::{BLUE, GREEN, RED};

/// Says the field is being typed into, with the text at `cursor` on the
/// surface that has the keyboard.
fn enable(field: &ZwpTextInputV3, cursor: (i32, i32, i32, i32)) {
    field.enable();
    field.set_cursor_rectangle(cursor.0, cursor.1, cursor.2, cursor.3);
    field.commit();
}

/// Says the field is no longer being typed into.
fn disable(field: &ZwpTextInputV3) {
    field.disable();
    field.commit();
}

/// Where the first pixel of `color` is, reading from the top left: the
/// corner of something filled with it.
fn corner(session: &mut Session, color: u32) -> (i32, i32) {
    let picture = session.draw();
    let found = (0..HEIGHT).find_map(|y| {
        (0..WIDTH)
            .find(|x| picture.at(*x, y) == color)
            .map(|x| (x, y))
    });
    found.unwrap_or_else(|| unreachable!("nothing is that colour"))
}

/// A session with an input method and a window.
fn typing() -> (Session, Client, (Client, ZwpInputMethodV2)) {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let method = typist.input_method();
    session.roundtrip(&mut typist);
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    (session, app, (typist, method))
}

#[test]
fn what_an_input_method_types_goes_into_the_field_being_typed_in() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let method = typist.input_method();
    session.roundtrip(&mut typist);
    let mut app = session.connect_plain();
    let field = app.text_input();
    session.open(&mut app, "app", (200, 100), RED);
    assert!(app.take_events().contains(&Event::TextEnter));

    enable(&field, (10, 10, 2, 16));
    session.roundtrip(&mut app);
    session.roundtrip(&mut typist);
    let told = typist.take_events();
    assert!(told.contains(&Event::TypingOn), "{told:?}");
    assert!(told.contains(&Event::TypingDone), "{told:?}");

    typist.type_preedit(&method, "nihon");
    session.roundtrip(&mut typist);
    session.roundtrip(&mut app);
    let shown = app.take_events();
    assert!(
        shown.contains(&Event::TextPreedit("nihon".to_owned())),
        "{shown:?}"
    );

    typist.type_text(&method, "日本");
    session.roundtrip(&mut typist);
    session.roundtrip(&mut app);
    let typed = app.take_events();
    assert!(
        typed.contains(&Event::TextCommitted("日本".to_owned())),
        "{typed:?}"
    );
    assert!(typed.contains(&Event::TextDone), "{typed:?}");

    // Done with the field, the input method is told so.
    disable(&field);
    session.roundtrip(&mut app);
    session.roundtrip(&mut typist);
    assert!(typist.take_events().contains(&Event::TypingOff));
}

#[test]
fn only_a_program_given_the_input_role_may_be_an_input_method_or_type_as_a_keyboard() {
    let mut session = Session::new();
    let plain = session.connect_plain();
    let offered = &plain.state.offered;
    assert!(
        offered
            .iter()
            .any(|name| name == "zwp_text_input_manager_v3"),
        "fields are anyone's"
    );
    for kept in [
        "zwp_input_method_manager_v2",
        "zwp_virtual_keyboard_manager_v1",
    ] {
        assert!(!offered.iter().any(|name| name == kept), "{kept}");
    }
    let typist = session.connect_as(Roles::INPUT);
    for given in [
        "zwp_input_method_manager_v2",
        "zwp_virtual_keyboard_manager_v1",
    ] {
        assert!(
            typist.state.offered.iter().any(|name| name == given),
            "{given}"
        );
    }
}

#[test]
fn an_input_method_started_again_after_it_stopped_is_the_one() {
    let (mut session, mut app, (typist, _method)) = typing();
    drop(typist);
    let mut again = session.connect_as(Roles::INPUT);
    let _method = again.input_method();
    session.roundtrip(&mut again);
    let field = app.text_input();
    session.roundtrip(&mut app);
    enable(&field, (10, 10, 2, 16));
    session.eventually(|session| {
        session.roundtrip(&mut app);
        session.roundtrip(&mut again);
        again.state.events.contains(&Event::TypingOn)
    });
}

#[test]
fn while_it_has_the_keyboard_keys_go_to_the_input_method_and_what_it_passes_on_to_the_window() {
    let (mut session, mut app, (mut typist, method)) = typing();
    let field = app.text_input();
    session.roundtrip(&mut app);
    enable(&field, (10, 10, 2, 16));
    session.roundtrip(&mut app);
    let _grab = typist.grab_keyboard(&method);
    session.roundtrip(&mut typist);
    app.take_events();
    typist.take_events();

    for pressed in [true, false] {
        let key = Input::Key {
            code: key::A,
            pressed,
        };
        session.compositor_mut().input(key, 0);
    }
    session.roundtrip(&mut typist);
    session.roundtrip(&mut app);
    let a = key::A.raw() - 8;
    let taken = typist.take_events();
    assert!(
        taken.contains(&Event::GrabbedKey {
            code: a,
            pressed: true
        }),
        "{taken:?}"
    );
    let keys = |events: &[Event]| {
        let key = |event: &&Event| matches!(event, Event::Key { .. });
        events.iter().filter(key).count()
    };
    assert_eq!(keys(&app.take_events()), 0, "the window is not given it");

    // What it has no use for, it types as a keyboard would.
    let keyboard = typist.virtual_keyboard();
    keyboard.key(0, a, 1);
    keyboard.key(0, a, 0);
    session.roundtrip(&mut typist);
    session.roundtrip(&mut app);
    let passed = app.take_events();
    assert!(
        passed.contains(&Event::Key {
            code: a,
            pressed: true
        }),
        "{passed:?}"
    );
}

#[test]
fn a_locked_session_is_typed_into_without_the_input_method_seeing_it() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let method = typist.input_method();
    session.roundtrip(&mut typist);
    let _grab = typist.grab_keyboard(&method);
    session.roundtrip(&mut typist);

    let (mut locker, _lock, _surface) = crate::lock::lock(&mut session);
    locker.take_events();
    typist.take_events();
    session.compositor_mut().input(
        Input::Key {
            code: key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut locker);
    session.roundtrip(&mut typist);
    let a = key::A.raw() - 8;
    assert!(locker.take_events().contains(&Event::Key {
        code: a,
        pressed: true
    }));
    let seen = typist.take_events();
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, Event::GrabbedKey { .. })),
        "{seen:?}"
    );
}

/// Puts up an input method's popup, `size` and blue, for a field whose
/// text is at `cursor` on the window's surface.
fn popup_for(
    session: &mut Session,
    (app, typist): (&mut Client, &mut Client),
    cursor: (i32, i32, i32, i32),
    size: (i32, i32),
) -> ZwpTextInputV3 {
    let method = typist.input_method();
    session.roundtrip(typist);
    let field = app.text_input();
    session.roundtrip(app);
    enable(&field, cursor);
    session.roundtrip(app);
    let popup = typist.input_popup(&method);
    typist.fill(&popup, size, BLUE);
    session.roundtrip(typist);
    session.roundtrip(typist);
    field
}

#[test]
fn its_popup_is_shown_under_the_text_it_is_for() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    let (x, y) = corner(&mut session, RED);
    let field = popup_for(
        &mut session,
        (&mut app, &mut typist),
        (10, 10, 2, 16),
        (40, 20),
    );
    assert_eq!(corner(&mut session, BLUE), (x + 10, y + 26));

    // As the text goes on, the popup goes with it, and is told where.
    typist.take_events();
    enable(&field, (60, 30, 2, 16));
    session.roundtrip(&mut app);
    session.roundtrip(&mut typist);
    assert!(
        typist
            .take_events()
            .contains(&Event::PopupText(60, 30, 2, 16))
    );
    assert_eq!(corner(&mut session, BLUE), (x + 60, y + 46));
}

#[test]
fn a_popup_with_no_room_under_the_text_goes_above_it_and_stays_on_the_screen() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    let (x, y) = corner(&mut session, RED);
    // Text at the very bottom of the screen, and off its right edge.
    let below = HEIGHT - y - 16;
    let beyond = WIDTH - x;
    popup_for(
        &mut session,
        (&mut app, &mut typist),
        (beyond, below, 2, 16),
        (40, 20),
    );
    assert_eq!(corner(&mut session, BLUE), (WIDTH - 40, HEIGHT - 16 - 20));
}

#[test]
fn a_popup_for_a_field_on_a_panel_is_shown_under_it() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let mut panel = session.connect();
    let shape = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top,
        size: (100, 40),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::Exclusive,
    };
    session.open_layer(&mut panel, &shape, (100, 40), GREEN);
    let (x, y) = corner(&mut session, GREEN);
    popup_for(
        &mut session,
        (&mut panel, &mut typist),
        (4, 4, 2, 16),
        (40, 20),
    );
    assert_eq!(corner(&mut session, BLUE), (x + 4, y + 20));
}

/// Whether anything blue is on the screen.
fn blue_shown(session: &mut Session) -> bool {
    let picture = session.draw();
    (0..HEIGHT).any(|y| (0..WIDTH).any(|x| picture.at(x, y) == BLUE))
}

#[test]
fn a_popup_for_text_that_is_not_shown_is_not_shown() {
    let mut session = Session::new();
    let mut typist = session.connect_as(Roles::INPUT);
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    popup_for(
        &mut session,
        (&mut app, &mut typist),
        (10, 10, 2, 16),
        (40, 20),
    );
    assert!(blue_shown(&mut session), "shown while the window is");
    session.compositor_mut().perform(&Action::Minimize);
    session.step();
    assert!(!blue_shown(&mut session), "and not once it is not");
}

#[test]
fn the_next_layout_is_typed_with_and_after_the_last_the_first() {
    let config = "keyboard: { layout: 'us,de' }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    assert_eq!(app.state.layout, 0);
    session.compositor_mut().perform(&Action::NextLayout);
    session.roundtrip(&mut app);
    assert_eq!(app.state.layout, 1);
    session.compositor_mut().perform(&Action::NextLayout);
    session.roundtrip(&mut app);
    assert_eq!(app.state.layout, 0);
}
