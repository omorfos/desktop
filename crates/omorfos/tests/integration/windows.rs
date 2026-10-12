//! Windows: mapping, state changes requested by clients, and drawing.

use std::time::Duration;

use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use crate::common::{Event, HEIGHT, Session, WIDTH};

pub(crate) const RED: u32 = 0xffff_0000;
pub(crate) const BLUE: u32 = 0xff00_00ff;
pub(crate) const GREEN: u32 = 0xff00_ff00;
/// The default theme's background, `#14161b`.
pub(crate) const BACKGROUND: u32 = 0xff14_161b;

#[test]
fn a_new_window_is_shown_focused_and_told_so() {
    let mut session = Session::new();
    let mut client = session.connect();

    let window = client.toplevel("foot", "~");
    session.roundtrip(&mut client);
    assert_eq!(
        client.take_events(),
        [Event::Configure {
            size: (0, 0),
            states: vec![]
        }],
        "the first configure lets the client choose its size"
    );

    client.fill(&window.surface, (200, 100), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let desktop = session.compositor().desktop();
    let focused = desktop.focused().and_then(|id| desktop.window(id)).unwrap();
    assert_eq!(
        (focused.app_id.as_str(), focused.title.as_str()),
        ("foot", "~")
    );
    let events = client.take_events();
    assert!(
        events.contains(&Event::Configure {
            size: (200, 100),
            states: vec![State::Activated]
        }),
        "{events:?}"
    );
    assert!(events.contains(&Event::KeyboardEnter), "{events:?}");
}

#[test]
fn a_window_without_content_is_not_shown() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel("slow", "");
    session.roundtrip(&mut client);

    // Committing again without a buffer still shows nothing.
    window.surface.commit();
    session.roundtrip(&mut client);

    assert_eq!(session.compositor().desktop().frames().count(), 0);
}

#[test]
fn opening_a_second_window_moves_focus_to_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    client.take_events();

    session.open(&mut client, "second", (200, 100), BLUE);

    let events = client.take_events();
    assert!(events.contains(&Event::KeyboardLeave), "{events:?}");
    assert!(
        events.contains(&Event::Configure {
            size: (200, 100),
            states: vec![]
        }),
        "the first window is told it is no longer active: {events:?}"
    );
    let desktop = session.compositor().desktop();
    let focused = desktop.focused().and_then(|id| desktop.window(id)).unwrap();
    assert_eq!(focused.app_id, "second");
}

#[test]
fn windows_are_drawn_where_the_desktop_places_them_over_the_background() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.open(&mut client, "second", (200, 100), BLUE);

    let picture = session.draw();
    assert!(picture.damaged);
    assert_eq!(picture.at(5, 5), RED, "the first window is at the corner");
    assert_eq!(picture.at(40, 40), BLUE, "the second cascades over it");
    assert_eq!(picture.at(231, 131), BLUE);
    assert_eq!(picture.at(300, 300), BACKGROUND);
    assert_eq!(picture.at(WIDTH - 1, HEIGHT - 1), BACKGROUND);
}

#[test]
fn nothing_is_redrawn_when_nothing_changed() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    assert!(session.draw().damaged);
    assert!(!session.draw().damaged);
}

#[test]
fn a_title_change_reaches_the_desktop() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "editor", (200, 100), RED);

    window.role.set_title("notes.txt".to_owned());
    session.roundtrip(&mut client);

    let desktop = session.compositor().desktop();
    let title = desktop
        .focused()
        .and_then(|id| desktop.window(id))
        .map(|w| w.title.clone());
    assert_eq!(title.as_deref(), Some("notes.txt"));
}

#[test]
fn a_client_can_maximize_and_restore_its_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((WIDTH, HEIGHT), vec![State::Maximized, State::Activated])
    );

    window.role.unset_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((200, 100), vec![State::Activated])
    );
}

#[test]
fn a_client_can_enter_and_leave_fullscreen() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "player", (200, 100), RED);
    client.take_events();

    window.role.set_fullscreen(None);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((WIDTH, HEIGHT), vec![State::Fullscreen, State::Activated])
    );

    window.role.unset_fullscreen();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((200, 100), vec![State::Activated])
    );
}

#[test]
fn a_maximized_window_follows_the_output_when_it_is_resized() {
    let mut session = Session::new();
    let mut client = session.connect();
    assert_eq!(client.state.outputs, 1);
    let window = session.open(&mut client, "app", (200, 100), RED);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    client.take_events();

    session.resize_output((800, 600));
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let area = session.compositor().area_of(&session.output());
    assert_eq!(area.size, (800, 600).into());
    assert_eq!(client.last_configure().0, (800, 600));
}

#[test]
fn a_minimized_window_is_hidden_and_loses_focus() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(session.draw().at(5, 5), RED);

    window.role.set_minimized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    assert_eq!(session.compositor().desktop().focused(), None);
    assert_eq!(session.draw().at(5, 5), BACKGROUND);
    let events = client.take_events();
    assert!(events.contains(&Event::KeyboardLeave), "{events:?}");
}

#[test]
fn closing_a_window_removes_it_and_focuses_the_one_below() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "below", (200, 100), RED);
    let window = session.open(&mut client, "above", (200, 100), BLUE);
    assert_eq!(session.draw().at(40, 40), BLUE);

    window.role.destroy();
    window.xdg_surface.destroy();
    window.surface.destroy();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let desktop = session.compositor().desktop();
    let focused = desktop.focused().and_then(|id| desktop.window(id)).unwrap();
    assert_eq!(focused.app_id, "below");
    assert_eq!(desktop.frames().count(), 1);
    assert_eq!(session.draw().at(40, 40), RED);
}

#[test]
fn a_window_closed_before_it_showed_anything_leaves_no_trace() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel("flash", "");
    session.roundtrip(&mut client);

    window.role.destroy();
    window.xdg_surface.destroy();
    session.roundtrip(&mut client);

    // Content arriving for the dead window must not bring it back.
    client.fill(&window.surface, (10, 10), RED);
    session.roundtrip(&mut client);
    assert_eq!(session.compositor().desktop().frames().count(), 0);
}

#[test]
fn a_disconnected_client_takes_its_windows_with_it() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "doomed", (200, 100), RED);

    drop(client);
    session.eventually(|session| session.compositor().desktop().frames().count() == 0);

    assert_eq!(session.draw().at(5, 5), BACKGROUND);
}

#[test]
fn subsurfaces_are_drawn_with_their_window() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);

    client.subsurface(&window.surface, (50, 10), (20, 20), BLUE);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let picture = session.draw();
    assert_eq!(picture.at(55, 15), BLUE);
    assert_eq!(picture.at(45, 15), RED);
}

#[test]
fn popups_are_configured_where_the_client_asks_and_can_be_repositioned() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    let (surface, popup, positioner) = client.popup(&window, (10, 20), (50, 30));
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::PopupConfigure {
            position: (10, 20),
            size: (50, 30)
        }),
        "{events:?}"
    );

    client.fill(&surface, (50, 30), GREEN);
    session.roundtrip(&mut client);
    assert_eq!(session.draw().at(15, 25), GREEN);

    popup.grab(client.seat(), client.state.serial);
    positioner.set_anchor_rect(30, 40, 1, 1);
    popup.reposition(&positioner, 7);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(events.contains(&Event::PopupRepositioned(7)), "{events:?}");
    assert!(
        events.contains(&Event::PopupConfigure {
            position: (30, 40),
            size: (50, 30)
        }),
        "{events:?}"
    );
}

#[test]
fn clients_are_told_when_a_frame_has_been_shown() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    client.request_frame(&window.surface);
    session.roundtrip(&mut client);
    assert!(!client.take_events().contains(&Event::FrameDone));

    session.draw();
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::FrameDone));
}

#[test]
fn a_window_may_be_one_pixel_of_a_colour_stretched_to_its_size() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel("plain", "plain window");
    session.roundtrip(&mut client);

    client.fill_plain(&window.surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);

    let picture = session.draw();
    assert_eq!(picture.at(0, 0), GREEN);
    assert_eq!(picture.at(WIDTH - 1, HEIGHT - 1), GREEN);
}
