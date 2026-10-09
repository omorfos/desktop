//! Pointer and keyboard input, and key bindings.

use std::time::{Duration, Instant};

use omorfos::Input;
use smithay::input::keyboard::Keycode;

use crate::common::{Event, HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, RED};

const LEFT_BUTTON: u32 = 0x110;

fn press(session: &mut Session, code: Keycode) {
    session.compositor_mut().input(
        Input::Key {
            code,
            pressed: true,
        },
        0,
    );
}

fn release(session: &mut Session, code: Keycode) {
    session.compositor_mut().input(
        Input::Key {
            code,
            pressed: false,
        },
        0,
    );
}

/// Presses and releases `code` while holding Super.
fn super_tap(session: &mut Session, code: Keycode) {
    press(session, key::SUPER);
    press(session, code);
    release(session, code);
    release(session, key::SUPER);
}

/// What the client's keyboard was sent: evdev codes, `true` for presses.
fn keys(events: &[Event]) -> Vec<(u32, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Key { code, pressed } => Some((*code, *pressed)),
            _ => None,
        })
        .collect()
}

fn focused_app(session: &Session) -> Option<String> {
    let desktop = session.compositor().desktop();
    desktop
        .focused()
        .and_then(|id| desktop.window(id))
        .map(|window| window.app_id.clone())
}

#[test]
fn the_pointer_enters_moves_within_and_leaves_a_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 20.0 }, 1);
    session
        .compositor_mut()
        .input(Input::Motion { x: 15.0, y: 25.0 }, 2);
    session
        .compositor_mut()
        .input(Input::Motion { x: 500.0, y: 400.0 }, 3);
    session.roundtrip(&mut client);

    let events = client.take_events();
    assert!(
        events.contains(&Event::PointerEnter { x: 10.0, y: 20.0 }),
        "{events:?}"
    );
    assert!(
        events.contains(&Event::PointerMotion { x: 15.0, y: 25.0 }),
        "{events:?}"
    );
    assert_eq!(events.last(), Some(&Event::PointerLeave));
}

#[test]
fn a_mouse_moves_the_pointer_by_a_distance_and_stops_at_the_edges() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 20.0 }, 1);
    session.roundtrip(&mut client);
    client.take_events();

    session
        .compositor_mut()
        .input(Input::MotionBy { dx: 5.0, dy: -3.0 }, 2);
    session.compositor_mut().input(
        Input::MotionBy {
            dx: -9000.0,
            dy: -9000.0,
        },
        3,
    );
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::PointerMotion { x: 15.0, y: 17.0 }),
        "{events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&Event::PointerMotion { x: 0.0, y: 0.0 }),
        "stopped at the top-left corner"
    );

    // The far corner is the last pixel, not one past it.
    session.compositor_mut().show_pointer();
    session.compositor_mut().input(
        Input::MotionBy {
            dx: 9000.0,
            dy: 9000.0,
        },
        4,
    );
    session.roundtrip(&mut client);
    assert_eq!(client.take_events().last(), Some(&Event::PointerLeave));
    session.compositor_mut().input(
        Input::MotionBy {
            dx: -5.0,
            dy: -20.0,
        },
        5,
    );
    assert_eq!(
        session.draw().at(WIDTH - 5, HEIGHT - 17),
        0xffff_ffff,
        "the pointer is five to the left of the last column and twenty above the last row"
    );
}

#[test]
fn the_pointer_is_drawn_only_for_a_backend_that_asks() {
    let mut session = Session::new();
    session
        .compositor_mut()
        .input(Input::Motion { x: 300.0, y: 200.0 }, 1);
    let background = session.draw().at(301, 203);

    session.compositor_mut().show_pointer();
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(301, 203), 0xffff_ffff, "inside the arrow");
    assert_eq!(picture.at(299, 203), background, "left of its tip");
    assert_eq!(picture.at(330, 203), background, "right of it");

    // It follows the pointer.
    session
        .compositor_mut()
        .input(Input::MotionBy { dx: 50.0, dy: 0.0 }, 2);
    let picture = session.draw();
    assert_eq!(picture.at(301, 203), background);
    assert_eq!(picture.at(351, 203), 0xffff_ffff);
}

#[test]
fn the_keys_that_switch_virtual_terminal_are_kept_for_the_backend() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    session.roundtrip(&mut client);
    client.take_events();
    assert_eq!(session.compositor_mut().take_terminal_switch(), None);

    press(&mut session, key::CONTROL);
    press(&mut session, key::ALT);
    press(&mut session, key::F2);
    release(&mut session, key::F2);
    release(&mut session, key::ALT);
    release(&mut session, key::CONTROL);
    session.roundtrip(&mut client);

    assert_eq!(session.compositor_mut().take_terminal_switch(), Some(2));
    assert_eq!(
        session.compositor_mut().take_terminal_switch(),
        None,
        "asked for once"
    );
    let f2 = key::F2.raw() - 8;
    assert!(
        keys(&client.take_events())
            .iter()
            .all(|(code, _)| *code != f2),
        "the application does not see the key"
    );
}

#[test]
fn the_pointer_is_where_it_looks_on_a_window_that_draws_its_own_shadow() {
    let mut session = Session::new();
    let mut client = session.connect();
    // The window proper is the middle of a larger surface, as it is for an
    // application that draws a shadow around itself.
    let window = client.toplevel("app", "app");
    session.roundtrip(&mut client);
    window.xdg_surface.set_window_geometry(30, 20, 200, 100);
    client.fill(&window.surface, (260, 160), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    client.take_events();
    // The window is placed at the corner, so its shadow is off the screen
    // above and to the left.
    assert_eq!(
        session.draw().at(229, 139),
        RED,
        "the far corner of the shadow"
    );
    assert_eq!(session.draw().at(231, 141), BACKGROUND);

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 5.0 }, 1);
    session.roundtrip(&mut client);
    assert_eq!(
        client.take_events(),
        [Event::PointerEnter { x: 40.0, y: 25.0 }],
        "counted from the corner of the surface, shadow included"
    );

    // The shadow below and to the right belongs to the window as well.
    session
        .compositor_mut()
        .input(Input::Motion { x: 225.0, y: 135.0 }, 2);
    session.roundtrip(&mut client);
    assert_eq!(
        client.take_events(),
        [Event::PointerMotion { x: 255.0, y: 155.0 }]
    );
    session
        .compositor_mut()
        .input(Input::Motion { x: 235.0, y: 145.0 }, 3);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::PointerLeave]);
}

#[test]
fn clicking_a_window_focuses_and_raises_it_and_the_click_is_delivered() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "below", (200, 100), RED);
    session.open(&mut client, "above", (200, 100), BLUE);
    assert_eq!(session.draw().at(40, 40), BLUE);
    client.take_events();

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 1);
    session.compositor_mut().input(
        Input::Button {
            button: LEFT_BUTTON,
            pressed: true,
        },
        2,
    );
    session.compositor_mut().input(
        Input::Button {
            button: LEFT_BUTTON,
            pressed: false,
        },
        3,
    );
    session.roundtrip(&mut client);

    assert_eq!(focused_app(&session).as_deref(), Some("below"));
    assert_eq!(
        session.draw().at(40, 40),
        RED,
        "the clicked window is now on top"
    );
    let events = client.take_events();
    assert!(events.contains(&Event::PointerButton {
        button: LEFT_BUTTON,
        pressed: true
    }));
    assert!(events.contains(&Event::PointerButton {
        button: LEFT_BUTTON,
        pressed: false
    }));
}

#[test]
fn clicking_the_desktop_changes_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    session
        .compositor_mut()
        .input(Input::Motion { x: 500.0, y: 400.0 }, 1);
    session.compositor_mut().input(
        Input::Button {
            button: LEFT_BUTTON,
            pressed: true,
        },
        2,
    );
    session.roundtrip(&mut client);

    assert_eq!(focused_app(&session).as_deref(), Some("app"));
}

#[test]
fn hovering_does_not_focus_under_click_to_focus() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "below", (200, 100), RED);
    session.open(&mut client, "above", (200, 100), BLUE);

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 1);
    session.roundtrip(&mut client);

    assert_eq!(focused_app(&session).as_deref(), Some("above"));
}

#[test]
fn hovering_focuses_when_focus_follows_the_pointer() {
    let mut session = Session::with_config(&[("config.yaml", "focus: { model: sloppy }\n")]);
    let mut client = session.connect();
    session.open(&mut client, "below", (200, 100), RED);
    session.open(&mut client, "above", (200, 100), BLUE);

    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 1);
    session
        .compositor_mut()
        .input(Input::Motion { x: 11.0, y: 10.0 }, 2);
    session.roundtrip(&mut client);

    assert_eq!(focused_app(&session).as_deref(), Some("below"));
}

#[test]
fn scrolling_is_delivered_to_the_window_under_the_pointer() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 10.0, y: 10.0 }, 1);
    client.take_events();

    session.compositor_mut().input(
        Input::Scroll {
            horizontal: 0.0,
            vertical: 15.0,
        },
        2,
    );
    session.roundtrip(&mut client);

    let events = client.take_events();
    assert!(
        events.contains(&Event::PointerScroll {
            vertical: true,
            value: 15.0
        }),
        "{events:?}"
    );
}

#[test]
fn keys_go_to_the_focused_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    press(&mut session, key::A);
    release(&mut session, key::A);
    session.roundtrip(&mut client);

    assert_eq!(keys(&client.take_events()), [(30, true), (30, false)]);
}

#[test]
fn a_bound_key_triggers_its_action_and_is_kept_from_the_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    // Super+q is bound to `close` by default.
    super_tap(&mut session, key::Q);
    session.roundtrip(&mut client);

    let events = client.take_events();
    assert!(events.contains(&Event::Close), "{events:?}");
    assert_eq!(
        keys(&events),
        [(125, true), (125, false)],
        "the window sees Super, but neither the press nor the release of q"
    );
}

#[test]
fn shift_selects_a_different_binding_on_the_same_key() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    // Super+2 switches workspace; Super+Shift+2 would move the window.
    super_tap(&mut session, key::TWO);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);

    super_tap(&mut session, key::ONE);
    press(&mut session, key::SHIFT);
    super_tap(&mut session, key::TWO);
    release(&mut session, key::SHIFT);
    session.roundtrip(&mut client);

    let desktop = session.compositor().desktop();
    assert_eq!(desktop.active_workspace(), 0);
    assert_eq!(
        desktop.frames().count(),
        0,
        "the window was sent to workspace 2"
    );
}

#[test]
fn a_chain_triggers_after_its_last_key_and_swallows_every_step() {
    let bindings = "bindings:\n  - { keys: Super+x t, action: { workspace: 2 } }\n";
    let mut session = Session::with_config(&[("keys.yaml", bindings)]);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    super_tap(&mut session, key::X);
    assert_eq!(
        session.compositor().desktop().active_workspace(),
        0,
        "the chain is not complete"
    );
    press(&mut session, key::T);
    release(&mut session, key::T);
    session.roundtrip(&mut client);

    assert_eq!(session.compositor().desktop().active_workspace(), 1);
    assert_eq!(keys(&client.take_events()), [(125, true), (125, false)]);
}

#[test]
fn a_wrong_key_abandons_a_chain_without_reaching_the_window() {
    let bindings = "bindings:\n  - { keys: Super+x t, action: { workspace: 2 } }\n";
    let mut session = Session::with_config(&[("keys.yaml", bindings)]);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    super_tap(&mut session, key::X);
    press(&mut session, key::A);
    release(&mut session, key::A);
    press(&mut session, key::T);
    release(&mut session, key::T);
    session.roundtrip(&mut client);

    assert_eq!(session.compositor().desktop().active_workspace(), 0);
    assert_eq!(
        keys(&client.take_events()),
        [(125, true), (125, false), (20, true), (20, false)],
        "the stray `a` is swallowed; the later `t` is an ordinary key again"
    );
}

#[test]
fn a_binding_can_start_a_program_that_can_find_the_compositor() {
    let mut session = Session::new();
    let marker = session.config.path().join("spawned");
    let script = format!("printf %s \"$WAYLAND_DISPLAY\" > '{}'", marker.display());
    session.write_config(
        "keys.yaml",
        &format!(
            "bindings:\n  - keys: Super+Return\n    action: {{ spawn: [sh, -c, \"{}\"] }}\n",
            script.replace('"', "\\\"")
        ),
    );
    session.compositor_mut().reload_config();
    assert_eq!(session.compositor().problems(), [""; 0]);
    session.compositor_mut().set_socket_name("wayland-test");

    super_tap(&mut session, key::RETURN);

    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    session.step();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "wayland-test");
}
