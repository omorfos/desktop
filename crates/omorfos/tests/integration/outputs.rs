//! Several screens: side by side in one space, each with its own size and
//! scale, coming and going.

use std::time::Duration;

use omorfos::{Input, Scale};
use omorfos_config::{Action, WorkspaceNumber};
use smithay::output::Output;
use wayland_protocols::xdg::shell::client::xdg_toplevel::State;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Event, HEIGHT, LayerShape, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const BLACK: u32 = 0xff00_0000;
const WHITE: u32 = 0xffff_ffff;
const LEFT_BUTTON: u32 = 0x110;

/// A session with a second output, 800 by 600, to the right of the first.
fn two_screens() -> (Session, Output) {
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    (session, second)
}

fn pointer_to(session: &mut Session, x: i32, y: i32) {
    let position = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(position, 0);
}

fn perform(session: &mut Session, action: &Action) {
    session.compositor_mut().perform(action);
}

fn panel() -> LayerShape {
    LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    }
}

/// Where the frames are, as left, top, width and height.
fn frames(session: &Session) -> Vec<(i32, i32, u32, u32)> {
    let frames = session.compositor().desktop().frames();
    frames
        .map(|frame| {
            let area = frame.geometry;
            (
                area.origin.x,
                area.origin.y,
                area.size.width,
                area.size.height,
            )
        })
        .collect()
}

#[test]
fn outputs_stand_side_by_side_in_the_order_they_were_added() {
    let (session, second) = two_screens();
    let compositor = session.compositor();
    let first = compositor.area_of(&session.output());
    let second = compositor.area_of(&second);
    assert_eq!((first.loc.x, first.loc.y), (0, 0));
    assert_eq!((first.size.w, first.size.h), (WIDTH, HEIGHT));
    assert_eq!((second.loc.x, second.loc.y), (WIDTH, 0));
    assert_eq!((second.size.w, second.size.h), (800, 600));
}

#[test]
fn clients_are_offered_each_output() {
    let (mut session, second) = two_screens();
    let mut client = session.connect();
    assert_eq!(client.state.outputs, 2);

    session.compositor_mut().remove_output(&second);
    let third = session.add_output((320, 240));
    session.roundtrip(&mut client);
    assert_eq!(client.state.outputs, 3, "told of the new one as it appears");
    let area = session.compositor().area_of(&third);
    assert_eq!(area.loc.x, WIDTH, "where the one unplugged was");
}

#[test]
fn a_window_opens_on_the_output_the_pointer_is_on_and_is_drawn_there() {
    let (mut session, _second) = two_screens();
    pointer_to(&mut session, WIDTH + 300, 200);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    assert_eq!(frames(&session), [(WIDTH, 0, 200, 100)]);
    assert_eq!(
        session.draw_on(1).at(100, 50),
        RED,
        "counted from its corner"
    );
    assert_eq!(session.draw_on(1).at(250, 50), BACKGROUND);
    assert_eq!(
        session.draw_on(0).at(100, 50),
        BACKGROUND,
        "not on the other"
    );
}

#[test]
fn a_window_across_two_outputs_is_drawn_on_both() {
    let (mut session, _second) = two_screens();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    // Hold Alt and drag it until it straddles the edge between them.
    let key = |pressed| Input::Key {
        code: key::ALT,
        pressed,
    };
    let button = |pressed| Input::Button {
        button: LEFT_BUTTON,
        pressed,
    };
    session.compositor_mut().input(key(true), 0);
    pointer_to(&mut session, 100, 50);
    session.compositor_mut().input(button(true), 0);
    pointer_to(&mut session, WIDTH, 250);
    session.compositor_mut().input(button(false), 0);
    session.compositor_mut().input(key(false), 0);
    session.roundtrip(&mut client);

    assert_eq!(frames(&session), [(WIDTH - 100, 200, 200, 100)]);
    assert_eq!(session.draw_on(0).at(WIDTH - 50, 250), RED, "its left half");
    assert_eq!(session.draw_on(1).at(50, 250), RED, "its right half");
    assert_eq!(session.draw_on(1).at(150, 250), BACKGROUND);
}

#[test]
fn a_maximized_window_fills_the_output_it_is_on() {
    let (mut session, _second) = two_screens();
    pointer_to(&mut session, WIDTH + 300, 200);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);

    perform(&mut session, &Action::Maximize);
    session.roundtrip(&mut client);
    let (size, states) = client.last_configure();
    assert_eq!(size, (800, 600));
    assert!(states.contains(&State::Maximized));
    assert_eq!(frames(&session), [(WIDTH, 0, 800, 600)]);
}

#[test]
fn the_pointer_crosses_where_outputs_meet_and_stops_at_their_other_edges() {
    let (mut session, _second) = two_screens();
    session.compositor_mut().show_pointer();
    let by = |session: &mut Session, dx: f64, dy: f64| {
        session
            .compositor_mut()
            .input(Input::MotionBy { dx, dy }, 0);
        session.step();
    };
    // The painted arrow is white a little inside its tip.
    let arrow_at = |session: &mut Session, screen, x, y| session.draw_on(screen).at(x + 1, y + 3);

    pointer_to(&mut session, WIDTH - 20, 100);
    by(&mut session, 50.0, 0.0);
    assert_eq!(arrow_at(&mut session, 1, 30, 100), WHITE, "onto the second");
    assert_eq!(session.draw_on(0).at(WIDTH - 19, 103), BACKGROUND);

    by(&mut session, 5000.0, 5000.0);
    assert_eq!(
        arrow_at(&mut session, 1, 799 - 4, 599 - 4),
        BACKGROUND,
        "the arrow hangs off the corner it stopped at"
    );
    by(&mut session, -10.0, -10.0);
    assert_eq!(arrow_at(&mut session, 1, 789, 589), WHITE);

    // The second output is taller. Below the first there is nothing, so
    // the pointer runs along the edge instead of leaving.
    by(&mut session, -5000.0, 0.0);
    assert_eq!(
        arrow_at(&mut session, 1, 0, 589),
        WHITE,
        "stopped at the edge"
    );
    by(&mut session, 0.0, -200.0);
    by(&mut session, -30.0, 0.0);
    assert_eq!(
        arrow_at(&mut session, 0, WIDTH - 30, 389),
        WHITE,
        "and across"
    );
}

#[test]
fn each_output_has_its_own_scale() {
    let (mut session, second) = two_screens();
    session
        .compositor_mut()
        .suggest_scale(&second, Scale::new(2.0));
    let compositor = session.compositor();
    assert_eq!(compositor.scale_of(&second), Scale::new(2.0));
    assert_eq!(compositor.scale_of(&session.output()), Scale::ONE);
    let area = compositor.area_of(&second);
    assert_eq!((area.loc.x, area.size.w, area.size.h), (WIDTH, 400, 300));

    // A window on it is told its scale and drawn at it, decoration and all.
    pointer_to(&mut session, WIDTH + 100, 100);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (100, 50), RED);
    assert_eq!(client.state.told.buffer_scale, Some(2));
    let picture = session.draw_on(1);
    assert_eq!(picture.at(100, 100), RED, "under a titlebar 56 pixels tall");
    assert_ne!(picture.at(190, 40), RED);
    assert_ne!(picture.at(190, 40), BACKGROUND);
    assert_eq!(picture.at(100, 170), BACKGROUND, "below the window");

    // Moved to the first output, it is told that one's.
    perform(&mut session, &Action::Maximize);
    session.compositor_mut().suggest_scale(&second, Scale::ONE);
    session.roundtrip(&mut client);
    assert_eq!(client.state.told.buffer_scale, Some(1));
}

#[test]
fn a_panel_goes_on_the_output_asked_for_or_else_the_pointers() {
    let (mut session, _second) = two_screens();
    let mut client = session.connect();

    pointer_to(&mut session, WIDTH + 100, 100);
    session.open_layer(&mut client, &panel(), (800, 30), GREEN);
    assert_eq!(session.draw_on(1).at(790, 20), GREEN, "across the second");
    assert_eq!(session.draw_on(0).at(100, 20), BACKGROUND);

    let (surface, _role) = client.layer_on(&panel(), Some(0));
    session.roundtrip(&mut client);
    client.fill(&surface, (WIDTH, 30), BLUE);
    client.request_frame(&surface);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(session.draw_on(0).at(100, 20), BLUE, "the one it named");

    // Each reserves room on its own output, and each is told when a frame
    // is shown there.
    session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(frames(&session), [(WIDTH, 30, 200, 100)]);
    client.take_events();
    let second = session.compositor().area_of(&session.output());
    assert_eq!(second.size.h, HEIGHT);
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::FrameDone));

    // The pointer reaches the one under it.
    pointer_to(&mut session, WIDTH + 400, 10);
    session.roundtrip(&mut client);
    let events = client.take_events();
    assert!(
        events.contains(&Event::PointerEnter { x: 400.0, y: 10.0 }),
        "{events:?}"
    );
}

#[test]
fn a_window_draws_at_the_pace_of_the_output_it_is_on() {
    let (mut session, second) = two_screens();
    pointer_to(&mut session, WIDTH + 300, 200);
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    client.take_events();

    client.request_frame(&window.surface);
    session.roundtrip(&mut client);
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [], "the first output is not its");

    session
        .compositor_mut()
        .send_frames(&second, Duration::from_millis(16));
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::FrameDone]);
}

#[test]
fn the_notice_is_on_the_first_output_and_the_menu_on_the_pointers() {
    let mut session = Session::with_config(&[("theme.yaml", "colors: { accent: nope }\n")]);
    session.add_output((800, 600));
    session.step();
    assert_ne!(
        session.draw_on(0).at(WIDTH - 20, 20),
        BACKGROUND,
        "the notice"
    );
    assert_eq!(session.draw_on(1).at(780, 20), BACKGROUND);

    // Opened near the far corner of the second output, the menu is kept
    // within it.
    pointer_to(&mut session, WIDTH + 790, 590);
    perform(&mut session, &Action::ShowMenu);
    session.step();
    assert_ne!(session.draw_on(1).at(700, 560), BACKGROUND, "the menu");
    assert_eq!(session.draw_on(0).at(600, 440), BACKGROUND);
}

#[test]
fn unplugging_an_output_moves_what_was_on_it_to_the_first() {
    let (mut session, second) = two_screens();
    session.compositor_mut().show_pointer();
    pointer_to(&mut session, WIDTH + 300, 200);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    session.open_layer(&mut client, &panel(), (800, 30), GREEN);
    client.take_events();

    session.compositor_mut().remove_output(&second);
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::LayerClosed));
    assert_eq!(frames(&session), [(0, 0, 200, 100)]);
    session.step();
    assert_eq!(session.draw().at(150, 80), RED);
    // The pointer came too, to the nearest point: the right-hand edge.
    session
        .compositor_mut()
        .input(Input::MotionBy { dx: -10.0, dy: 0.0 }, 0);
    assert_eq!(session.draw().at(WIDTH - 11 + 1, 200 + 3), WHITE);

    // What is said about an output that has gone changes nothing.
    let compositor = session.compositor_mut();
    compositor.remove_output(&second);
    compositor.resize_output(&second, (100, 100));
    compositor.suggest_scale(&second, Scale::new(2.0));
    compositor.send_frames(&second, Duration::ZERO);
    assert_eq!(compositor.scale_of(&second), Scale::ONE);
    assert_eq!(compositor.area_of(&second).size.w, 0);
}

#[test]
fn a_session_carries_on_with_no_output_until_one_is_plugged_in() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let only = session.output();
    session.compositor_mut().remove_output(&only);

    // Nothing to point at, nowhere to put a panel, and nothing breaks.
    session
        .compositor_mut()
        .input(Input::MotionBy { dx: 50.0, dy: 50.0 }, 0);
    pointer_to(&mut session, 10, 10);
    perform(&mut session, &Action::ShowMenu);
    client.layer(&panel());
    session.roundtrip(&mut client);
    assert!(client.take_events().contains(&Event::LayerClosed));
    let later = client.toplevel("later", "later");
    session.roundtrip(&mut client);
    client.fill(&later.surface, (100, 50), BLUE);
    session.roundtrip(&mut client);

    // Drawing on an output that has gone shows what is there, unscaled.
    assert_eq!(session.draw().at(400, 400), BACKGROUND);

    // Escape closes the menu, which would otherwise be over the windows.
    for pressed in [true, false] {
        let escape = Input::Key {
            code: key::ESCAPE,
            pressed,
        };
        session.compositor_mut().input(escape, 0);
    }
    session.add_output((WIDTH, HEIGHT));
    session.roundtrip(&mut client);
    assert_eq!(session.draw_on(1).at(150, 80), RED, "the windows are back");
}

#[test]
fn a_locked_session_shows_nothing_on_any_output() {
    let (mut session, second) = two_screens();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    pointer_to(&mut session, WIDTH + 300, 200);
    session.open(&mut app, "other", (200, 100), BLUE);

    let mut locker = session.connect();
    let lock = locker.lock();
    session.roundtrip(&mut locker);
    let frame = Duration::from_millis(16);

    // One output going blank is not the session being locked.
    assert_eq!(session.draw_on(0).at(50, 50), BLACK);
    session.send_frames(frame);
    session.roundtrip(&mut locker);
    assert_eq!(locker.take_events(), [], "the other still shows a window");
    assert_eq!(session.draw_on(1).at(50, 50), BLACK);
    session.compositor_mut().send_frames(&second, frame);
    session.roundtrip(&mut locker);
    assert_eq!(locker.take_events(), [Event::Locked]);

    // The locker covers each output with a surface of its size.
    let first_surface = locker.lock_surface_on(&lock, 0);
    let second_surface = locker.lock_surface_on(&lock, 1);
    session.roundtrip(&mut locker);
    let sizes = locker.take_events();
    assert!(sizes.contains(&Event::LockSize(640, 480)), "{sizes:?}");
    assert!(sizes.contains(&Event::LockSize(800, 600)), "{sizes:?}");
    locker.fill(&first_surface, (WIDTH, HEIGHT), GREEN);
    locker.fill(&second_surface, (800, 600), GREEN);
    locker.request_frame(&second_surface);
    session.roundtrip(&mut locker);
    assert_eq!(session.draw_on(0).at(600, 400), GREEN);
    assert_eq!(session.draw_on(1).at(700, 500), GREEN);
    session.compositor_mut().send_frames(&second, frame);

    // The pointer is on the second output, so its surface has the input.
    locker.take_events();
    pointer_to(&mut session, WIDTH + 310, 210);
    session.roundtrip(&mut locker);
    let events = locker.take_events();
    assert!(
        events.contains(&Event::PointerEnter { x: 310.0, y: 210.0 }),
        "{events:?}"
    );
    assert!(events.contains(&Event::FrameDone));

    // An output unplugged while locked takes its surface with it, and the
    // keyboard goes to the one that is left.
    session.compositor_mut().remove_output(&second);
    session.roundtrip(&mut locker);
    session.compositor_mut().input(
        Input::Key {
            code: key::A,
            pressed: true,
        },
        0,
    );
    session.roundtrip(&mut locker);
    let heard = locker.take_events();
    assert!(
        heard.iter().any(|event| matches!(event, Event::Key { .. })),
        "{heard:?}"
    );
    assert_eq!(session.draw_on(0).at(600, 400), GREEN);
    assert_eq!(
        session.draw_on(1).at(700, 500),
        BLACK,
        "nothing is drawn where the output was"
    );

    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    assert_eq!(
        session.draw_on(0).at(50, 50),
        BLUE,
        "the window from the output that went, now on top"
    );
}

/// A client with one window on each of two outputs.
#[test]
fn a_window_that_is_not_shown_is_told_the_scale_of_the_first_output() {
    let (mut session, second) = two_screens();
    session
        .compositor_mut()
        .suggest_scale(&second, Scale::new(2.0));
    pointer_to(&mut session, WIDTH + 100, 100);
    let mut client: Client = session.connect();
    session.open(&mut client, "app", (100, 50), RED);
    assert_eq!(client.state.told.buffer_scale, Some(2));

    perform(
        &mut session,
        &Action::Workspace(WorkspaceNumber::clamped(2)),
    );
    session.roundtrip(&mut client);
    assert_eq!(client.state.told.buffer_scale, Some(1));
}

#[test]
fn a_locker_with_a_surface_on_one_output_only_still_has_the_keyboard() {
    let (mut session, _second) = two_screens();
    pointer_to(&mut session, WIDTH + 300, 200);
    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface_on(&lock, 0);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    locker.take_events();

    // The pointer is on the output it left bare.
    assert_eq!(session.draw_on(1).at(300, 200), BLACK);
    let key = |pressed| Input::Key {
        code: key::A,
        pressed,
    };
    session.compositor_mut().input(key(true), 0);
    session.compositor_mut().input(key(false), 0);
    session.roundtrip(&mut locker);
    let heard = locker.take_events();
    assert!(
        heard.iter().any(|event| matches!(event, Event::Key { .. })),
        "{heard:?}"
    );
}
