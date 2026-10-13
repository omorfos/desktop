//! Who may do what: a program has what an ordinary one has unless the
//! compositor started it for more.

use omorfos::Roles;
use smithay::wayland::selection::ext_data_control::DataControlHandler;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, LayerShape, Session};
use crate::windows::{BACKGROUND, GREEN, RED};

/// A surface 20 high along the top of the screen, on a layer.
fn strip(layer: Layer, keyboard: KeyboardInteractivity) -> LayerShape {
    LayerShape {
        layer,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 0,
        keyboard,
    }
}

/// Has a client ask for a surface on a layer, and says whether it was
/// refused: told to close it without ever having been told its size.
fn refused(session: &mut Session, client: &mut Client, layer: Layer) -> bool {
    client.take_events();
    let _held = client.layer(&strip(layer, KeyboardInteractivity::None));
    session.roundtrip(client);
    session.roundtrip(client);
    client.take_events().contains(&Event::LayerClosed)
}

/// What the user has been told has happened.
fn told(session: &Session) -> Vec<String> {
    session.compositor().events().map(str::to_owned).collect()
}

#[test]
fn an_ordinary_program_cannot_put_a_panel_on_the_screen() {
    let mut session = Session::new();
    let mut client = session.connect_plain();
    assert!(refused(&mut session, &mut client, Layer::Top));
    assert!(refused(&mut session, &mut client, Layer::Background));
    session.step();
    assert_eq!(session.draw().at(300, 10), BACKGROUND, "nothing was put up");

    // The user is told what asked and how it could be let, once.
    let said = told(&session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("asked to put a panel on the screen"),
        "{said:?}"
    );
    assert!(said[0].contains("may: [panel]"), "{said:?}");

    // An ordinary window is still what any program may have.
    session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(session.draw().at(10, 10), RED);
}

#[test]
fn a_program_let_put_up_panels_may_put_them_on_any_layer() {
    let mut session = Session::new();
    let mut panel = session.connect_as(Roles::PANEL);
    for layer in [Layer::Background, Layer::Top, Layer::Overlay] {
        assert!(!refused(&mut session, &mut panel, layer));
    }
    // So may one that may take the keyboard, which is the more.
    let mut launcher = session.connect_as(Roles::OVERLAY);
    assert!(!refused(&mut session, &mut launcher, Layer::Overlay));
    assert_eq!(told(&session), [""; 0]);
}

#[test]
fn only_what_is_let_takes_the_keyboard_from_a_window() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let focused = |session: &Session| session.compositor().desktop().focused().is_some();
    assert!(focused(&session));
    app.take_events();

    // A panel that asks for every key is a panel all the same.
    let mut panel = session.connect_as(Roles::PANEL);
    let grabbing = strip(Layer::Top, KeyboardInteractivity::Exclusive);
    session.open_layer(&mut panel, &grabbing, (640, 20), GREEN);
    session.roundtrip(&mut app);
    let events = app.take_events();
    assert!(!events.contains(&Event::KeyboardLeave), "{events:?}");

    let mut launcher = session.connect_as(Roles::OVERLAY);
    session.open_layer(&mut launcher, &grabbing, (640, 20), GREEN);
    session.roundtrip(&mut app);
    let events = app.take_events();
    assert!(events.contains(&Event::KeyboardLeave), "{events:?}");
}

#[test]
fn an_ordinary_program_cannot_lock_the_session() {
    let mut session = Session::new();
    let mut client = session.connect_plain();
    let _lock = client.lock();
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::LockRefused));
    assert!(!session.compositor().is_locked());
    session.step();
    let said = told(&session);
    assert!(said[0].contains("asked to lock the session"), "{said:?}");
    assert!(said[0].contains("may: [lock]"), "{said:?}");

    let mut locker = session.connect_as(Roles::LOCK);
    let _lock = locker.lock();
    session.roundtrip(&mut locker);
    assert!(session.compositor().is_locked());
}

#[test]
fn an_ordinary_program_is_not_told_what_windows_there_are() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "secret", (200, 100), RED);

    let mut plain = session.connect_plain();
    session.roundtrip(&mut plain);
    assert!(plain.state.taskbar.is_empty(), "no list is offered to it");
    let mut panel = session.connect_as(Roles::PANEL);
    session.roundtrip(&mut panel);
    assert_eq!(panel.state.taskbar.len(), 1);
}

#[test]
fn a_program_the_configuration_starts_with_a_role_is_handed_its_connection() {
    // Nothing is to be seen of a program that does nothing; that it is
    // started, and with what, is all that is asked here.
    let config = "autostart:\n  - { run: ['true'], may: [panel, lock] }\n  - ['true']\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.compositor_mut().autostart();
    session.step();
    assert_eq!(session.compositor().problems(), [""; 0]);
}

#[test]
fn the_lock_action_starts_the_program_named_to_lock_with() {
    // With none named, the user is told how to name one, and only once.
    let mut session = Session::new();
    let lock = omorfos_config::Action::Lock;
    session.compositor_mut().perform(&lock);
    session.compositor_mut().perform(&lock);
    session.step();
    let said = told(&session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("locker: [swaylock, -f]"), "{said:?}");

    // One that is named is started, and one that cannot be is a problem.
    let mut session = Session::with_config(&[("config.yaml", "locker: ['true']\n")]);
    session.compositor_mut().perform(&lock);
    session.step();
    assert_eq!(session.compositor().problems(), [""; 0]);
    assert_eq!(told(&session), [""; 0]);
    let missing = "locker: [/nowhere/omorfos-no-such-locker]\n";
    let mut session = Session::with_config(&[("config.yaml", missing)]);
    session.compositor_mut().perform(&lock);
    let problems = session.compositor().problems().join("\n");
    assert!(problems.contains("cannot start"), "{problems}");
}

/// The things only a role allows, which a program in a sandbox is not to
/// be shown, and the way a sandbox is made.
const WITHHELD: [&str; 7] = [
    "ext_data_control_manager_v1",
    "ext_image_copy_capture_manager_v1",
    "ext_output_image_capture_source_manager_v1",
    "zwlr_layer_shell_v1",
    "ext_session_lock_manager_v1",
    "ext_foreign_toplevel_list_v1",
    "wp_security_context_manager_v1",
];

#[test]
fn a_program_in_a_sandbox_is_not_shown_what_only_a_role_allows() {
    let mut session = Session::new();
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("sandbox");

    // An ordinary program is shown all but the list of windows, and is
    // refused what it asks for, so that the user can be told.
    let mut flatpak = session.connect_plain();
    let shown = |client: &Client, name: &str| client.state.offered.iter().any(|it| it == name);
    assert!(shown(&flatpak, "zwlr_layer_shell_v1"));
    assert!(shown(&flatpak, "wp_security_context_manager_v1"));

    let open = flatpak.sandbox(&path);
    session.roundtrip(&mut flatpak);

    let stream = std::os::unix::net::UnixStream::connect(&path).unwrap();
    let mut inside = session.adopt(stream);
    for name in WITHHELD {
        assert!(!shown(&inside, name), "{name} is shown in a sandbox");
    }

    // It is a program like any other for the rest, and nothing is said of
    // it: it asked for nothing it could not have.
    assert!(shown(&inside, "xdg_wm_base"));
    session.open(&mut inside, "app", (200, 100), GREEN);
    assert_eq!(session.draw().at(10, 10), GREEN);
    assert_eq!(told(&session), Vec::<String>::new());

    // When the sandbox ends, nothing more can come in that way.
    drop(open);
    session.eventually(|_| std::os::unix::net::UnixStream::connect(&path).is_err());
}

#[test]
fn only_a_program_that_may_is_given_the_clipboard_at_any_time() {
    let mut session = Session::new();
    let clipboard = "ext_data_control_manager_v1";
    let shown = |client: &Client| client.state.offered.iter().any(|it| it == clipboard);
    assert!(!shown(&session.connect_plain()), "offering it is giving it");
    assert!(!shown(&session.connect_as(Roles::PANEL)));
    assert!(shown(&session.connect_as(Roles::CLIPBOARD)));

    // Smithay has the compositor say where it keeps this, and never asks.
    // It is asked here, so that what it would be told is known to be so.
    let kept = session.compositor().data_control_state();
    assert_eq!(kept.global(), kept.global());
}
