//! The sealed prompt: while a program with the `ask` role asks for a
//! secret on the overlay layer, the session is dimmed behind it, and the
//! keyboard and the pointer reach it alone.

use omorfos::{Input, Roles};
use omorfos_config::Action;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use crate::common::{Client, Event, HEIGHT, LayerShape, Session, WIDTH, key};
use crate::windows::{BACKGROUND, GREEN, RED};

/// A prompt, `(100, 50)` in the middle of the screen, that wants the
/// keyboard.
fn prompt_shape() -> LayerShape {
    LayerShape {
        layer: Layer::Overlay,
        anchor: Anchor::empty(),
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::Exclusive,
    }
}

/// Where the prompt is: the middle of the screen.
const PROMPT: (i32, i32) = ((WIDTH - 100) / 2 + 50, (HEIGHT - 50) / 2 + 25);

/// The red of a window as it shows through the dim: darker, and red still.
fn dimmed(pixel: u32) -> bool {
    let (red, green, blue) = ((pixel >> 16) & 0xff, (pixel >> 8) & 0xff, pixel & 0xff);
    (70..100).contains(&red) && green == 0 && blue == 0
}

/// A session with a window that has the keyboard, the pointer over it.
fn busy() -> (Session, Client) {
    let mut session = Session::new();
    let mut app = session.connect_plain();
    session.open(&mut app, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut app);
    app.take_events();
    (session, app)
}

/// A prompt that is up: its client, its surface, and the role that makes it
/// a panel's.
type Asking = (
    Client,
    wayland_client::protocol::wl_surface::WlSurface,
    ZwlrLayerSurfaceV1,
);

/// Takes the prompt down, as a program does once it has its answer.
fn answered(session: &mut Session, (mut asker, surface, role): Asking) {
    role.destroy();
    surface.destroy();
    session.roundtrip(&mut asker);
    session.step();
}

/// Puts up the prompt, as a program with `roles` would.
fn ask(session: &mut Session, roles: Roles) -> Asking {
    let mut asker = session.connect_as(roles);
    let (surface, role) = session.open_layer(&mut asker, &prompt_shape(), (100, 50), GREEN);
    (asker, surface, role)
}

#[test]
fn a_prompt_that_asks_for_a_secret_dims_the_rest_of_the_screen() {
    let (mut session, _app) = busy();
    assert_eq!(session.draw().at(50, 50), RED);
    let asking = ask(&mut session, Roles::ASK);
    let picture = session.draw();
    assert_eq!(
        picture.at(PROMPT.0, PROMPT.1),
        GREEN,
        "the prompt is not dimmed"
    );
    assert!(dimmed(picture.at(50, 50)), "{:08x}", picture.at(50, 50));
    assert_ne!(
        picture.at(WIDTH - 5, HEIGHT - 5),
        BACKGROUND,
        "nor the desktop"
    );

    // Gone, the screen is as it was.
    answered(&mut session, asking);
    assert_eq!(session.draw().at(50, 50), RED);
}

#[test]
fn an_overlay_without_the_role_dims_nothing() {
    let (mut session, _app) = busy();
    let _overlay = ask(&mut session, Roles::OVERLAY);
    let picture = session.draw();
    assert_eq!(picture.at(PROMPT.0, PROMPT.1), GREEN);
    assert_eq!(picture.at(50, 50), RED);
}

#[test]
fn every_screen_is_dimmed_and_the_prompt_is_on_its_own() {
    let (mut session, _app) = busy();
    session.add_output((WIDTH, HEIGHT));
    session.step();
    let _asking = ask(&mut session, Roles::ASK);
    let other = session.draw_on(1);
    let dim = other.at(PROMPT.0, PROMPT.1);
    assert_ne!(dim, GREEN, "the prompt is not here");
    assert_ne!(dim, BACKGROUND, "and this screen is dimmed");
}

#[test]
fn the_keyboard_is_the_prompts_alone_and_no_binding_is_acted_on() {
    let keys = "bindings:\n  - { keys: a, action: close }\n";
    let mut session = Session::with_config(&[("keys.yaml", keys)]);
    let mut app = session.connect_plain();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let (mut asker, _surface, _role) = ask(&mut session, Roles::ASK);
    session.roundtrip(&mut asker);
    assert!(asker.take_events().contains(&Event::KeyboardEnter));
    session.roundtrip(&mut app);
    assert!(app.take_events().contains(&Event::KeyboardLeave));

    for pressed in [true, false] {
        let key = Input::Key {
            code: key::A,
            pressed,
        };
        session.compositor_mut().input(key, 0);
    }
    session.roundtrip(&mut asker);
    session.roundtrip(&mut app);
    let a = key::A.raw() - 8;
    assert!(asker.take_events().contains(&Event::Key {
        code: a,
        pressed: true
    }));
    let told = app.take_events();
    assert!(
        !told.contains(&Event::Close),
        "the binding was not acted on"
    );
    assert!(!told.iter().any(|event| matches!(event, Event::Key { .. })));
    drop(window);
}

#[test]
fn the_pointer_reaches_the_prompt_and_nothing_else() {
    let (mut session, mut app) = busy();
    let (mut asker, _surface, _role) = ask(&mut session, Roles::ASK);
    let motion = |session: &mut Session, (x, y): (i32, i32)| {
        let at = Input::Motion {
            x: f64::from(x),
            y: f64::from(y),
        };
        session.compositor_mut().input(at, 0);
    };
    motion(&mut session, (60, 60));
    for pressed in [true, false] {
        let click = Input::Button {
            button: 0x110,
            pressed,
        };
        session.compositor_mut().input(click, 0);
    }
    session.roundtrip(&mut app);
    let told = app.take_events();
    assert!(
        !told
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "{told:?}"
    );

    motion(&mut session, PROMPT);
    session.roundtrip(&mut asker);
    let told = asker.take_events();
    assert!(
        told.iter()
            .any(|event| matches!(event, Event::PointerEnter { .. })),
        "{told:?}"
    );
}

#[test]
fn the_switcher_and_the_menu_are_closed_under_a_prompt() {
    let (mut session, _app) = busy();
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    let asking = ask(&mut session, Roles::ASK);
    session.step();
    answered(&mut session, asking);
    // Nothing of the menu came back with the prompt gone.
    let picture = session.draw();
    assert_eq!(picture.at(50, 50), RED);
}
