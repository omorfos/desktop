//! Windows: mapping, state changes requested by clients, and drawing.

use std::time::Duration;

use wayland_protocols::xdg::shell::client::xdg_toplevel::State;

use omorfos_config::Action;

use crate::common::{Event, HEIGHT, Session, WIDTH};

pub(crate) const RED: u32 = 0xffff_0000;
pub(crate) const BLUE: u32 = 0xff00_00ff;
pub(crate) const GREEN: u32 = 0xff00_ff00;
/// The default theme's background, `#0c1226`.
pub(crate) const BACKGROUND: u32 = 0xff0c_1226;

/// Whether `pixel` is `color`, in the open or in the shadow of something
/// beside it: darkened evenly, as black over it darkens it, by no more than
/// a few shadows together, and never lighter or of another hue.
pub(crate) fn shaded(pixel: u32, color: u32) -> bool {
    let channels = |of: u32| [of >> 16, of >> 8, of].map(|channel| f64::from(channel & 0xff));
    let (seen, open) = (channels(pixel), channels(color));
    // How much of the colour's brightest channel is left.
    let brightest = seen.iter().zip(open).max_by(|a, b| a.1.total_cmp(&b.1));
    let share = brightest.map_or(0.0, |(seen, open)| seen / open);
    if pixel >> 24 != 0xff || !share.is_finite() {
        return pixel == color;
    }
    let mut pairs = seen.iter().zip(open);
    (0.25..=1.0).contains(&share)
        && pairs.all(|(seen, open)| *seen <= open && (seen - open * share).abs() <= 1.5)
}

#[test]
fn a_colour_in_a_shadow_is_darkened_evenly_and_nothing_else() {
    assert!(shaded(BACKGROUND, BACKGROUND), "in the open");
    assert!(shaded(0xff09_0e1c, BACKGROUND), "under a shadow");
    assert!(shaded(0xffbb_0000, RED));
    assert!(!shaded(0xff2b_3a6b, BACKGROUND), "lighter, as a border is");
    assert!(
        !shaded(0xff10_172f, BACKGROUND),
        "lighter, as a titlebar is"
    );
    assert!(
        !shaded(0xff0d_1327, BACKGROUND),
        "a little lighter, as the edge of a curve is"
    );
    assert!(!shaded(0xff0c_120c, BACKGROUND), "of another hue");
    assert!(
        !shaded(0xff00_0000, BACKGROUND),
        "darker than shadows make it"
    );
    assert!(!shaded(0x800c_1226, BACKGROUND), "see-through");
    assert!(!shaded(0xff10_0000, 0xff00_0000), "black is only black");
}

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
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);

    window.role.unset_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        client.last_configure(),
        ((200, 100), vec![State::Activated])
    );
}

#[test]
fn a_window_is_told_nothing_new_until_its_program_has_drawn_what_it_was_told_last() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let maximized = ((WIDTH, HEIGHT), vec![State::Maximized, State::Activated]);
    assert_eq!(client.last_configure(), maximized);

    // Restored before it has drawn itself maximized, the window is as it
    // was at once, and its program is not told yet.
    window.role.unset_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let frame = session.compositor().desktop().frames().next().unwrap();
    assert_eq!(
        frame.client().size,
        omorfos_core::geometry::Size::new(200, 100)
    );
    let events = client.take_events();
    let told = events
        .iter()
        .any(|event| matches!(event, Event::Configure { .. }));
    assert!(!told, "not told yet: {events:?}");

    // Once it has drawn what it was told, it is told how it is to be now.
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
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
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);

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
    client.fill(&window.surface, (WIDTH, HEIGHT), RED);
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
fn a_program_sent_away_for_breaking_the_protocol_is_said_to_have_been() {
    let mut session = Session::new();
    // One that leaves is nothing to say.
    let mut client = session.connect();
    session.open(&mut client, "polite", (200, 100), RED);
    drop(client);
    session.eventually(|session| session.compositor().desktop().frames().count() == 0);
    assert_eq!(session.compositor_mut().take_logged(), Vec::<String>::new());
    let client = session.connect();
    client.break_the_protocol();
    let mut logged = Vec::new();
    session.eventually(|session| {
        logged.extend(session.compositor_mut().take_logged());
        !logged.is_empty()
    });
    let said = &logged[0];
    assert!(
        said.starts_with("integration-"),
        "named by its file: {said}"
    );
    assert!(
        said.contains(&format!(", process {}", std::process::id())),
        "{said}"
    );
    assert!(said.contains("was disconnected for asking"), "{said}");
    assert!(said.contains(": xdg_wm_base@"), "{said}");
    assert_eq!(logged.len(), 1, "once");
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

/// A theme in which windows come and go over a time, and so change size
/// smoothly.
pub(crate) const MOVING: (&str, &str) = ("theme.css", "window { transition: 180ms ease-out; }\n");

/// How long a window takes to get where it was sent.
pub(crate) const SENT: Duration = Duration::from_millis(240);

#[test]
fn a_window_s_last_picture_is_stretched_to_its_frame_until_its_program_draws_the_new_size() {
    let mut session = Session::with_config(&[MOVING]);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    assert_eq!(session.draw().at(5, 5), RED, "come in");
    assert_eq!(session.draw().at(600, 400), BACKGROUND);

    // Maximized, and not yet drawn so, its picture fills its frame.
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    session.advance(SENT);
    let picture = session.draw();
    assert_eq!((picture.at(5, 5), picture.at(600, 400)), (RED, RED));

    // Once drawn, its new picture shows, and the stretched one fades into
    // it over the time the theme gives a window to change.
    client.fill(&window.surface, (WIDTH, HEIGHT), BLUE);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(
        session.draw().at(600, 400),
        RED,
        "setting out from the last"
    );
    session.advance(Duration::from_millis(90));
    let between = session.draw().at(600, 400);
    assert!(between != RED && between != BLUE, "{between:08x}");
    session.advance(Duration::from_millis(90));
    assert_eq!(session.draw().at(600, 400), BLUE);
    assert_eq!(session.compositor().deadline(), None);
}

#[test]
fn a_window_whose_program_draws_another_size_than_asked_is_shown_as_it_draws() {
    let mut session = Session::with_config(&[MOVING]);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    window.role.set_maximized();
    session.roundtrip(&mut client);

    // It has heard, and draws itself as large as it was all the same.
    window.surface.commit();
    session.roundtrip(&mut client);
    session.advance(SENT);
    let picture = session.draw();
    assert_eq!((picture.at(5, 5), picture.at(600, 400)), (RED, BACKGROUND));
}

#[test]
fn a_window_maximized_eases_there_and_its_program_is_told_only_where_it_goes() {
    let mut session = Session::with_config(&[MOVING]);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    client.take_events();

    // Told at once how it is to be, it sets out from where it was.
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let maximized = ((WIDTH, HEIGHT), vec![State::Maximized, State::Activated]);
    assert_eq!(client.last_configure(), maximized);
    let picture = session.draw();
    assert_eq!(
        (picture.at(195, 95), picture.at(260, 160)),
        (RED, BACKGROUND)
    );

    // Half way in half the time, its last picture stretched to fill it.
    session.advance(SENT / 2);
    let picture = session.draw();
    assert_eq!(
        (picture.at(400, 270), picture.at(440, 310)),
        (RED, BACKGROUND)
    );

    // Drawn as large on the way, it is told nothing more, and gets there.
    client.fill(&window.surface, (WIDTH, HEIGHT), BLUE);
    session.roundtrip(&mut client);
    session.advance(SENT / 4);
    let picture = session.draw();
    assert_eq!(
        (picture.at(500, 380), picture.at(630, 470)),
        (BLUE, BACKGROUND)
    );
    session.advance(SENT / 4);
    let picture = session.draw();
    assert_eq!((picture.at(5, 5), picture.at(635, 475)), (BLUE, BLUE));
    let events = client.take_events();
    let told = events
        .iter()
        .any(|event| matches!(event, Event::Configure { .. }));
    assert!(!told, "{events:?}");
    assert_eq!(session.compositor().deadline(), None);
}

#[test]
fn with_motion_reduced_a_window_is_where_it_was_sent_at_once() {
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let mut session = Session::with_config(&[MOVING, reduced]);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    session.advance(Duration::from_secs(1));
    window.role.set_maximized();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(session.draw().at(600, 400), RED);
}

#[test]
fn a_windows_title_and_its_programs_name_are_kept_no_longer_than_any_is_shown() {
    let mut session = Session::new();
    let mut client = session.connect();
    // As long as fits in a message of the protocol, and in letters of more
    // than one byte, so that what is kept is cut between letters.
    let (name, title) = ("n".repeat(1000), "ż".repeat(1100));
    session.open(&mut client, &name, (200, 100), RED);
    let window = session
        .compositor()
        .desktop()
        .windows()
        .next()
        .unwrap()
        .window
        .clone();
    assert_eq!(window.app_id.chars().count(), 255);
    let toplevel = client.toplevel("short", &title);
    session.roundtrip(&mut client);
    client.fill(&toplevel.surface, (200, 100), RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(title_of(&session, "short"), "ż".repeat(1024));
    // And as it changes.
    toplevel.role.set_title("ź".repeat(2000));
    session.roundtrip(&mut client);
    assert_eq!(title_of(&session, "short"), "ź".repeat(1024));
}

/// The title the desktop keeps for the window of the program `app_id`.
fn title_of(session: &Session, app_id: &str) -> String {
    let desktop = session.compositor().desktop();
    let mut windows = desktop.windows();
    let kept = windows.find(|listed| listed.window.app_id == app_id);
    kept.map_or_else(String::new, |kept| kept.window.title.clone())
}

#[test]
fn a_program_is_given_no_more_than_the_most_windows_at_once() {
    // The most, `MOST_WINDOWS` in the compositor's handlers.
    const MOST: usize = 256;
    // How many of them are shown; the rest wait to be.
    const SHOWN: usize = 4;
    let mut session = Session::new();
    let mut client = session.connect();
    let configured = |events: Vec<Event>| {
        let configures = events.iter();
        configures
            .filter(|event| matches!(event, Event::Configure { .. }))
            .count()
    };
    // Some shown, and the rest to the most waiting to be: both count.
    for _ in 0..SHOWN {
        session.open(&mut client, "app", (50, 50), RED);
    }
    client.take_events();
    let mut windows = Vec::new();
    for _ in SHOWN..MOST {
        let window = client.toplevel_uncommitted("app", "app");
        window.surface.commit();
        windows.push(window);
    }
    session.roundtrip(&mut client);
    assert_eq!(configured(client.take_events()), MOST - SHOWN);

    // One more is never configured, and so never shown.
    let past = client.toplevel_uncommitted("app", "past");
    past.surface.commit();
    session.roundtrip(&mut client);
    assert_eq!(configured(client.take_events()), 0);

    // Another program is not held to this one's.
    let mut other = session.connect();
    let theirs = other.toplevel_uncommitted("other", "other");
    theirs.surface.commit();
    session.roundtrip(&mut other);
    assert_eq!(configured(other.take_events()), 1);

    // One let go of makes room for another.
    let gone = windows.pop().unwrap();
    gone.role.destroy();
    gone.xdg_surface.destroy();
    let again = client.toplevel_uncommitted("app", "again");
    again.surface.commit();
    session.roundtrip(&mut client);
    assert_eq!(configured(client.take_events()), 1);
}

#[test]
fn a_program_that_places_its_parts_at_the_ends_of_the_numbers_stops_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open_decorated(&mut client, "app", (200, 100), RED);
    // Parts as far off as the numbers go, either way, one on another.
    let far = i32::MAX / 2 + 10;
    let one = client.subsurface(&window.surface, (far, far), (10, 10), BLUE);
    let _two = client.subsurface(&one, (far, far), (10, 10), BLUE);
    let _back = client.subsurface(&window.surface, (i32::MIN, i32::MIN), (10, 10), BLUE);
    let (popup, _, _) = client.popup(&window, (i32::MAX - 5, i32::MAX - 5), (10, 10));
    session.roundtrip(&mut client);
    client.fill(&popup, (10, 10), BLUE);
    // And where it says the window proper begins in what it shows.
    window
        .xdg_surface
        .set_window_geometry(i32::MAX - 5, i32::MIN + 5, 200, 100);
    window.surface.commit();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    drop(session.draw());

    // Pointed at, clicked, and shown in the overview and the switcher.
    for (x, y) in [(10.0, 10.0), (100.0, 50.0), (0.0, 0.0)] {
        let compositor = session.compositor_mut();
        compositor.input(omorfos::Input::Motion { x, y }, 0);
        compositor.input(
            omorfos::Input::Button {
                button: 0x110,
                pressed: true,
            },
            0,
        );
        compositor.input(
            omorfos::Input::Button {
                button: 0x110,
                pressed: false,
            },
            0,
        );
        session.step();
    }
    for action in [Action::Overview, Action::Overview, Action::SwitchWindows] {
        session.compositor_mut().perform(&action);
        session.advance(Duration::from_millis(500));
        drop(session.draw());
    }

    // The session goes on as before for the next program.
    drop((popup, one, window, client));
    // Once it is seen gone: a window shown while another is still there
    // is placed a step along from it, and stays there.
    session.eventually(|session| session.compositor().desktop().frames().count() == 0);
    let mut next = session.connect();
    session.open(&mut next, "next", (200, 100), GREEN);
    session.eventually(|session| session.draw().at(10, 10) == GREEN);
}
