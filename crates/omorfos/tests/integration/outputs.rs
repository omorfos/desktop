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
    let mut session = Session::with_workspaces();
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
        session.compositor_mut().input(
            Input::MotionBy {
                dx,
                dy,
                raw_dx: dx,
                raw_dy: dy,
            },
            0,
        );
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
    let mut session = Session::with_config(&[("theme.css", ":root { --accent: nope; }\n")]);
    session.add_output((800, 600));
    session.settle();
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
    session.settle();
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
    session.compositor_mut().input(
        Input::MotionBy {
            dx: -10.0,
            dy: 0.0,
            raw_dx: -10.0,
            raw_dy: 0.0,
        },
        0,
    );
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
    session.compositor_mut().input(
        Input::MotionBy {
            dx: 50.0,
            dy: 50.0,
            raw_dx: 50.0,
            raw_dy: 50.0,
        },
        0,
    );
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

#[test]
fn a_window_that_is_not_shown_is_still_told_the_scale_of_its_own_output() {
    let (mut session, second) = two_screens();
    session
        .compositor_mut()
        .suggest_scale(&second, Scale::new(2.0));
    pointer_to(&mut session, WIDTH + 100, 100);
    let mut client: Client = session.connect();
    let window = session.open(&mut client, "app", (100, 50), RED);
    client.watch_scale(&window.surface);
    session.roundtrip(&mut client);
    assert_eq!(client.state.told.buffer_scale, Some(2));
    assert_eq!(client.state.told.scale, Some(240));

    // Hidden by a switch of workspace on its screen, it is not told the
    // first screen's scale: it would draw itself for a screen it is not
    // on, and a terminal changes size doing that, a little more each time.
    for _ in 0..3 {
        perform(
            &mut session,
            &Action::Workspace(WorkspaceNumber::clamped(2)),
        );
        session.roundtrip(&mut client);
        assert_eq!(client.state.told.buffer_scale, Some(2), "while hidden");
        assert_eq!(client.state.told.scale, Some(240));
        perform(
            &mut session,
            &Action::Workspace(WorkspaceNumber::clamped(1)),
        );
        session.roundtrip(&mut client);
        assert_eq!(client.state.told.scale, Some(240), "and shown again");
    }

    // Sent to the first screen, it is told that one's.
    perform(&mut session, &Action::MoveToNextOutput);
    session.roundtrip(&mut client);
    assert_eq!(client.state.told.buffer_scale, Some(1));
    assert_eq!(client.state.told.scale, Some(120));
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

/// Two screens with a red window on the first and a blue one on the second,
/// and the pointer left on the second.
fn a_window_on_each() -> (Session, Output, Client) {
    let (mut session, second) = two_screens();
    let mut client = session.connect();
    pointer_to(&mut session, 100, 100);
    session.open(&mut client, "left", (200, 100), RED);
    pointer_to(&mut session, WIDTH + 100, 100);
    session.open(&mut client, "right", (200, 100), BLUE);
    client.take_events();
    (session, second, client)
}

fn workspace(number: u32) -> WorkspaceNumber {
    WorkspaceNumber::clamped(number)
}

#[test]
fn switching_workspace_changes_only_the_screen_the_pointer_is_on() {
    let (mut session, _second, _client) = a_window_on_each();
    perform(&mut session, &Action::Workspace(workspace(2)));
    session.settle();
    assert_eq!(
        session.draw_on(1).at(100, 50),
        BACKGROUND,
        "the second moved on"
    );
    assert_eq!(session.draw_on(0).at(100, 50), RED, "the first did not");

    pointer_to(&mut session, 100, 100);
    perform(&mut session, &Action::NextWorkspace);
    session.settle();
    assert_eq!(session.draw_on(0).at(100, 50), BACKGROUND);
    perform(&mut session, &Action::PreviousWorkspace);
    session.settle();
    assert_eq!(session.draw_on(0).at(100, 50), RED);
    assert_eq!(
        session.draw_on(1).at(100, 50),
        BACKGROUND,
        "still on its second"
    );
}

#[test]
fn the_keyboard_goes_to_the_screen_whose_workspace_was_switched() {
    let (mut session, _second, mut client) = a_window_on_each();
    let focused = |session: &Session| {
        let desktop = session.compositor().desktop();
        let id = desktop.focused()?;
        desktop.window(id).map(|window| window.app_id.clone())
    };
    assert_eq!(focused(&session).as_deref(), Some("right"));

    // Moving the pointer over does not take the keyboard along.
    pointer_to(&mut session, 300, 300);
    assert_eq!(focused(&session).as_deref(), Some("right"));

    // Switching workspace there and back does.
    perform(&mut session, &Action::NextWorkspace);
    assert_eq!(focused(&session), None, "nothing on that workspace");
    perform(&mut session, &Action::PreviousWorkspace);
    session.roundtrip(&mut client);
    assert_eq!(focused(&session).as_deref(), Some("left"));
}

#[test]
fn a_window_can_be_sent_to_the_next_screen() {
    let (mut session, _second, _client) = a_window_on_each();
    // The blue window has the keyboard, and goes round to the first.
    perform(&mut session, &Action::MoveToNextOutput);
    session.step();
    assert_eq!(session.draw_on(1).at(100, 50), BACKGROUND);
    assert_eq!(
        session.draw_on(0).at(100, 50),
        BLUE,
        "on top of the red one"
    );

    perform(&mut session, &Action::MoveToNextOutput);
    session.step();
    assert_eq!(session.draw_on(0).at(100, 50), RED);
    assert_eq!(session.draw_on(1).at(100, 50), BLUE);
}

#[test]
fn a_window_from_an_unplugged_screen_keeps_its_workspace_number() {
    let (mut session, second, _client) = a_window_on_each();
    perform(&mut session, &Action::MoveToWorkspace(workspace(3)));
    session.compositor_mut().remove_output(&second);
    session.step();

    assert_eq!(
        session.draw().at(100, 50),
        RED,
        "the first shows what it did"
    );
    pointer_to(&mut session, 100, 100);
    perform(&mut session, &Action::Workspace(workspace(3)));
    session.settle();
    assert_eq!(session.draw().at(100, 50), BLUE, "on the first's third");
}

#[test]
fn the_state_says_which_screen_each_workspace_and_window_is_on() {
    use omorfos_ipc::{Client as Peer, Reply, Request};

    let (mut session, _second, _client) = a_window_on_each();
    perform(&mut session, &Action::Workspace(workspace(2)));
    let mut peer = Peer::connect(&session.listen()).unwrap();
    let Reply::State(state) = session.ask(&mut peer, &Request::State) else {
        unreachable!("expected the state")
    };

    let workspaces: Vec<(&str, u32, bool)> = state
        .workspaces
        .iter()
        .map(|workspace| {
            (
                workspace.output.as_str(),
                workspace.number,
                workspace.active,
            )
        })
        .collect();
    assert_eq!(
        workspaces,
        [
            ("test-0", 1, true),
            ("test-0", 2, false),
            ("test-0", 3, false),
            ("test-0", 4, false),
            ("test-1", 1, false),
            ("test-1", 2, true),
            ("test-1", 3, false),
            ("test-1", 4, false),
        ]
    );
    let windows: Vec<(&str, &str, u32)> = state
        .windows
        .iter()
        .map(|window| {
            (
                window.app_id.as_str(),
                window.output.as_str(),
                window.workspace,
            )
        })
        .collect();
    assert_eq!(windows, [("left", "test-0", 1), ("right", "test-1", 1)]);
}

#[test]
fn with_no_screen_the_state_still_lists_one_set_of_workspaces() {
    use omorfos_ipc::{Client as Peer, Reply, Request};

    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let only = session.output();
    session.compositor_mut().remove_output(&only);
    let mut peer = Peer::connect(&session.listen()).unwrap();
    let Reply::State(state) = session.ask(&mut peer, &Request::State) else {
        unreachable!("expected the state")
    };
    assert_eq!(state.workspaces.len(), 1);
    assert!(
        state
            .workspaces
            .iter()
            .all(|workspace| workspace.output.is_empty())
    );
    assert_eq!(state.windows[0].output, "");
}

/// The left edge of each output, in the order they were added.
fn lefts(session: &Session, second: &Output) -> (i32, i32) {
    let compositor = session.compositor();
    let first = compositor.area_of(&session.output()).loc.x;
    (first, compositor.area_of(second).loc.x)
}

#[test]
fn screens_stand_in_the_order_the_configuration_lists_them() {
    // The second screen to be plugged in is to stand on the left.
    let listed = "outputs:\n  - { name: test-1 }\n  - { name: not-plugged-in }\n";
    let mut session = Session::with_config(&[("config.yaml", listed)]);
    assert_eq!(session.compositor().area_of(&session.output()).loc.x, 0);
    let second = session.add_output((800, 600));
    assert_eq!(lefts(&session, &second), (800, 0), "the first moved over");

    // A window keeps its place on its screen when the screens change
    // places, and each screen keeps its own workspaces.
    let mut client = session.connect();
    pointer_to(&mut session, 800 + 100, 100);
    session.open(&mut client, "app", (200, 100), RED);
    assert_eq!(session.draw_on(0).at(100, 50), RED);

    session.write_config("config.yaml", "outputs: [{ name: test-0 }]\n");
    session.compositor_mut().reload_config();
    session.step();
    assert_eq!(lefts(&session, &second), (0, WIDTH));
    assert_eq!(session.draw_on(0).at(100, 50), RED, "still on the first");
    assert_eq!(session.draw_on(1).at(100, 50), BACKGROUND);
    assert_eq!(frames(&session), [(0, 0, 200, 100)]);

    // With none listed they stand in the order they were plugged in.
    session.write_config("config.yaml", "");
    session.compositor_mut().reload_config();
    assert_eq!(lefts(&session, &second), (0, WIDTH));
}

#[test]
fn a_screen_named_in_the_configuration_has_the_scale_given_for_it() {
    let scales =
        "output: { scale: 1.5 }\noutputs:\n  - { name: test-1, scale: 2 }\n  - { name: test-2 }\n";
    let mut session = Session::with_config(&[("config.yaml", scales)]);
    let second = session.add_output((800, 600));
    let third = session.add_output((800, 600));
    let scale = |session: &Session, output: &Output| session.compositor().scale_of(output);
    assert_eq!(
        scale(&session, &session.output()),
        Scale::new(1.5),
        "for all"
    );
    assert_eq!(scale(&session, &second), Scale::new(2.0), "its own");
    assert_eq!(
        scale(&session, &third),
        Scale::new(1.5),
        "named, but no scale"
    );

    // What the backend suggests still counts where the configuration
    // leaves it to the screen.
    session.write_config("config.yaml", "outputs: [{ name: test-1, scale: auto }]\n");
    session.compositor_mut().reload_config();
    session
        .compositor_mut()
        .suggest_scale(&second, Scale::new(1.25));
    assert_eq!(scale(&session, &second), Scale::new(1.25));
    assert_eq!(scale(&session, &third), Scale::ONE);
}

#[test]
fn what_is_wrong_with_how_a_screen_is_driven_is_said_until_it_is_put_right() {
    let mut session = Session::new();
    let wrong = "config.yaml: the screen on eDP-1 cannot be driven at 1920x1080".to_owned();
    session
        .compositor_mut()
        .screen_wrong("eDP-1", Some(wrong.clone()));
    assert!(session.compositor().problems().contains(&wrong));
    // Reading the configuration again, which says what else is wrong,
    // leaves it said.
    session.compositor_mut().reload_config();
    assert!(session.compositor().problems().contains(&wrong));
    // Something else wrong with it takes its place, and once nothing is,
    // nothing is said.
    let other = "config.yaml: the screen on eDP-1 cannot be drawn 75 times a second".to_owned();
    session
        .compositor_mut()
        .screen_wrong("eDP-1", Some(other.clone()));
    assert_eq!(session.compositor().problems(), [other]);
    session.compositor_mut().screen_wrong("eDP-1", None);
    assert_eq!(session.compositor().problems(), [] as [String; 0]);
}

#[test]
fn a_screen_stands_where_the_configuration_says_next_to_the_one_listed_before() {
    let listed = "outputs:\n  - { name: test-1 }\n  - { name: test-0, place: below }\n  - { name: test-2, place: left }\n";
    let mut session = Session::with_config(&[("config.yaml", listed)]);
    let second = session.add_output((800, 600));
    let third = session.add_output((320, 240));
    let at = |session: &Session, output: &Output| {
        let at = session.compositor().area_of(output).loc;
        (at.x, at.y)
    };
    let first = session.output();
    // Below the second, the middles in line, and the third to its left,
    // the tops in line: all from the top left corner of what they cover.
    assert_eq!(at(&session, &second), (240, 0));
    assert_eq!(at(&session, &first), (320, 600));
    assert_eq!(at(&session, &third), (0, 600));

    let above = "outputs:\n  - { name: test-0 }\n  - { name: test-1, place: above }\n  - { name: test-2, place: right }\n";
    session.write_config("config.yaml", above);
    session.compositor_mut().reload_config();
    assert_eq!(at(&session, &second), (0, 0));
    assert_eq!(at(&session, &first), (80, 600));
    assert_eq!(at(&session, &third), (800, 0));

    // One not listed is to the right of all the rest, at the top.
    let partly = "outputs:\n  - { name: test-0 }\n  - { name: test-1, place: below }\n";
    session.write_config("config.yaml", partly);
    session.compositor_mut().reload_config();
    assert_eq!(at(&session, &first), (80, 0));
    assert_eq!(at(&session, &second), (0, 480));
    assert_eq!(at(&session, &third), (800, 0));
}

#[test]
fn a_screen_is_lined_up_and_moved_along_the_edge_it_shares_with_the_one_before() {
    // The second, 800 by 600, first; the first, 640 by 480, below it at
    // the start and moved left by a tenth of its width; the third, 320 by
    // 240, to its right at the end and moved up 40.
    let listed = "outputs:\n  - { name: test-1 }\n  - { name: test-0, place: below, align: start, offset: -10% }\n  - { name: test-2, place: right, align: end, offset: -40 }\n";
    let mut session = Session::with_config(&[("config.yaml", listed)]);
    let second = session.add_output((800, 600));
    let third = session.add_output((320, 240));
    let at = |session: &Session, output: &Output| {
        let at = session.compositor().area_of(output).loc;
        (at.x, at.y)
    };
    let first = session.output();
    assert_eq!(at(&session, &second), (64, 0));
    assert_eq!(at(&session, &first), (0, 600));
    assert_eq!(at(&session, &third), (640, 600 + 480 - 240 - 40));
    // Lined up at the middle beside it, and at the end below the one
    // listed before, which is that one.
    let middle = "outputs:\n  - { name: test-1 }\n  - { name: test-2, align: middle }\n  - { name: test-0, place: below, align: end }\n";
    session.write_config("config.yaml", middle);
    session.compositor_mut().reload_config();
    assert_eq!(at(&session, &second), (0, 0));
    assert_eq!(at(&session, &third), (800, 180));
    assert_eq!(at(&session, &first), (480, 420));
    assert!(
        session.compositor().problems().is_empty(),
        "{:?}",
        session.compositor().problems()
    );

    // The first listed has none before it, which the user is told of.
    let first_placed = "outputs:\n  - { name: test-1, place: below }\n";
    session.write_config("config.yaml", first_placed);
    session.compositor_mut().reload_config();
    let problems = session.compositor().problems();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("`test-1` is the first screen listed")),
        "{problems:?}"
    );
}

#[test]
fn a_screen_drawn_into_a_window_is_not_turned_whatever_the_configuration_says() {
    // As one inside another desktop is drawn: into a window whose picture
    // is upside down already.
    let turned = "outputs: [{ name: nested, rotate: left }]\n";
    let mut session = Session::with_config(&[("config.yaml", turned)]);
    let nested = session.compositor_mut().add_output(
        "nested",
        (WIDTH, HEIGHT),
        smithay::utils::Transform::Flipped180,
    );
    let area = session.compositor().area_of(&nested);
    assert_eq!((area.size.w, area.size.h), (WIDTH, HEIGHT));
    assert_eq!(
        nested.current_transform(),
        smithay::utils::Transform::Flipped180
    );
}

#[test]
fn the_windows_of_a_screen_unplugged_go_to_the_one_that_comes_first() {
    let primary = "outputs:\n  - { name: test-0 }\n  - { name: test-1 }\n  - { name: test-2, primary: true }\n";
    let mut session = Session::with_config(&[("config.yaml", primary)]);
    session.add_output((800, 600));
    let third = session.add_output((320, 240));
    let mut client = session.connect();
    pointer_to(&mut session, 100, 100);
    session.open(&mut client, "app", (200, 100), RED);
    let output = session.output();
    session.compositor_mut().remove_output(&output);
    session.step();
    let area = session.compositor().area_of(&third);
    let (x, ..) = frames(&session)[0];
    assert!(
        (area.loc.x..area.loc.x + area.size.w).contains(&x),
        "on the third: {x} in {area:?}"
    );

    // Two may not come first.
    let two =
        "outputs:\n  - { name: test-1, primary: true }\n  - { name: test-2, primary: true }\n";
    session.write_config("config.yaml", two);
    session.compositor_mut().reload_config();
    let problems = session.compositor().problems();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("only one output may be `primary`")),
        "{problems:?}"
    );
}

#[test]
fn the_arrangement_for_the_screens_plugged_in_is_followed_as_they_come_and_go() {
    let arranged = "arrangements:\n  - [{ name: test-1 }, { name: test-0, place: below, scale: 2 }]\n  - [{ name: test-1, place: right }]\n";
    let mut session = Session::with_config(&[("config.yaml", arranged)]);
    let at = |session: &Session, output: &Output| {
        let area = session.compositor().area_of(output);
        (area.loc.x, area.loc.y, area.size.w, area.size.h)
    };
    let first = session.output();
    // Alone, no arrangement is for it: as `outputs` says, which is nothing.
    assert_eq!(at(&session, &first), (0, 0, WIDTH, HEIGHT));
    // With the second, both are as the arrangement for the two says.
    let second = session.add_output((800, 600));
    assert_eq!(at(&session, &second), (0, 0, 800, 600));
    assert_eq!(at(&session, &first), (240, 600, WIDTH / 2, HEIGHT / 2));
    // And as they were once it goes.
    session.compositor_mut().remove_output(&second);
    assert_eq!(at(&session, &first), (0, 0, WIDTH, HEIGHT));
    // The first of an arrangement given a place is said to have none.
    let problems = session.compositor().problems();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("`test-1` is the first screen listed")),
        "{problems:?}"
    );
}
