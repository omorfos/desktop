//! A session with nothing changing: the third priority, no work when
//! nothing changes, held as a gate.

use std::assert_matches;
use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;

use crate::common::{Client, Session, Toplevel, WIDTH};
use crate::cursors::theme;
use crate::layers::panel;

/// The most a turn of a session with nothing changing may allocate, which
/// is what it allocates today, with one window in a frame of ours whose
/// titlebar is solid: 10 in drawing, what is drawn being made anew each
/// time, much of it Smithay's elements for what the program drew; and 3
/// in the turn before it, Smithay's refreshing of the space among them.
/// Each is let go of within the turn. It is to come down to nothing, and
/// is lowered as each goes; never raised. It is what a turn allocates and
/// no more: with room to spare, what a change adds to every turn goes by
/// unseen, as three maps made anew each turn did on 23 November 2026.
pub(crate) const CEILING: u64 = 13;

/// What a titlebar that is glass adds to that, for the one window: 7, in
/// laying its sheet out, in looking at what is under it to see whether it
/// changed, and in cutting it to its corners. The built-in theme's is
/// glass (ADR 97), which this is the price of where nothing changes. It
/// too is to come down to nothing, and is never raised.
const GLASS: u64 = 7;

/// The most a turn may allocate with one window in a frame of ours as the
/// built-in theme draws it, its titlebar glass.
pub(crate) const GLAZED: u64 = CEILING + GLASS;

/// A theme whose titlebars are solid, as they were before they were
/// glass.
const SOLID: &str = "titlebar { background: var(--surface-dim); }\n\
                     titlebar:focus { background: var(--surface); }\n";

#[test]
fn a_session_with_nothing_changing_draws_nothing_wakes_for_nothing_and_allocates_little() {
    for (theme, ceiling) in [(SOLID, CEILING), ("", GLAZED)] {
        let mut session = Session::with_config(&[("theme.css", theme)]);
        session.blurring();
        let mut client = session.connect();
        let _window = session.open_decorated(&mut client, "app", (200, 100), 0xff00_ff00);
        // In a frame of ours, as most windows are. Whatever was coming in
        // has come in.
        session.eventually(|session| !session.render());

        for turn in 0..10 {
            // Nothing is due, so the loop would sleep until something
            // happens.
            assert_eq!(session.compositor().deadline(), None, "turn {turn}");
            let mut drawn = true;
            let made = allocation_counter::measure(|| {
                session.advance(Duration::from_millis(16));
                drawn = session.render();
            });
            assert!(!drawn, "turn {turn} drew");
            assert_eq!(made.count_current, 0, "turn {turn} kept what it made");
            assert!(
                made.count_total <= ceiling,
                "turn {turn} allocated {} times, more than {ceiling}",
                made.count_total
            );
        }
    }
}

#[test]
fn a_session_with_nothing_changing_allocates_no_more_for_a_peer_that_follows_it() {
    let mut session = Session::new();
    session.blurring();
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    let mut client = session.connect();
    let _window = session.open_decorated(&mut client, "app", (200, 100), 0xff00_ff00);
    let told = session.ask(&mut peer, &omorfos_ipc::Request::Subscribe);
    assert_matches!(told, omorfos_ipc::Reply::State(_));
    session.eventually(|session| !session.render());

    for turn in 0..10 {
        assert_eq!(session.compositor().deadline(), None, "turn {turn}");
        let made = allocation_counter::measure(|| {
            session.advance(Duration::from_millis(16));
            session.render();
        });
        assert_eq!(made.count_current, 0, "turn {turn} kept what it made");
        assert!(
            made.count_total <= GLAZED,
            "turn {turn} allocated {} times, more than {GLAZED}",
            made.count_total
        );
    }
}

/// How many times a turn with nothing changing allocates, at most over ten
/// turns, with `windows` windows open in frames of ours, cascaded, and
/// `theme`.
fn quiet_with(windows: usize, theme: &str) -> u64 {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut client = session.connect();
    let _windows: Vec<_> = (0..windows)
        .map(|each| {
            session.open_decorated(&mut client, &format!("app{each}"), (200, 100), 0xff00_ff00)
        })
        .collect();
    session.eventually(|session| !session.render());
    let mut most = 0;
    for _ in 0..10 {
        let made = allocation_counter::measure(|| {
            session.advance(Duration::from_millis(16));
            session.render();
        });
        most = most.max(made.count_total);
    }
    most
}

#[test]
fn a_session_with_nothing_changing_does_no_more_for_glass_the_more_windows_are_under_it() {
    // Each titlebar is glass over those before it, so what is under each
    // grows with the windows; with nothing changing, none of it is looked
    // at again, and what glass adds to a turn grows no faster than the
    // windows do.
    let glass = |windows| quiet_with(windows, "") - quiet_with(windows, SOLID);
    let (two, eight) = (glass(2), glass(8));
    assert!(
        eight <= 4 * two,
        "glass adds {eight} with eight windows, {two} with two"
    );
}

/// How long a turn with nothing changing takes, with 1 to 256 windows open,
/// in frames of ours and drawing their own: the least, and the middle, of
/// 41 times 200 turns. A measure, not a gate, as how long anything takes
/// depends on the machine and what else it is doing; run by hand in an
/// optimised build:
///
/// ```sh
/// cargo test --profile nightly -p omorfos --all-features --test integration \
///     how_long_a_turn -- --ignored --nocapture
/// ```
#[test]
#[ignore = "a measurement, run by hand in an optimised build"]
fn how_long_a_turn_with_nothing_changing_takes_as_windows_are_added() {
    for framed in [true, false] {
        for windows in [1_usize, 8, 32, 128, 256] {
            let mut session = Session::with_config(&[("theme.css", SOLID)]);
            let mut client = session.connect();
            let _windows: Vec<_> = (0..windows)
                .map(|each| {
                    let (app, size, color) = (format!("app{each}"), (200, 100), 0xff00_ff00);
                    if framed {
                        session.open_decorated(&mut client, &app, size, color)
                    } else {
                        session.open(&mut client, &app, size, color)
                    }
                })
                .collect();
            session.eventually(|session| !session.render());
            let mut samples = Vec::new();
            for _ in 0..41 {
                let started = std::time::Instant::now();
                for _ in 0..200 {
                    session.advance(Duration::from_millis(16));
                }
                samples.push(started.elapsed() / 200);
            }
            samples.sort();
            let said = format!(
                "{windows} windows, framed {framed}: least {:?}, middle {:?}\n",
                samples[0], samples[20]
            );
            std::io::Write::write_all(&mut std::io::stderr(), said.as_bytes()).unwrap();
        }
    }
}

/// Whether anything on the session's screen waits to be told that a frame
/// was shown there.
fn awaited(session: &Session) -> bool {
    session.compositor().awaits_frame(&session.output())
}

/// `session` with one window open and nothing left to draw or to tell.
fn settled_in(mut session: Session) -> (Session, Client, Toplevel) {
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), 0xff00_ff00);
    session.eventually(|session| !session.render());
    session.send_frames(Duration::from_millis(16));
    (session, client, window)
}

/// A session with one window open and nothing left to draw or to tell.
fn settled() -> (Session, Client, Toplevel) {
    settled_in(Session::new())
}

#[test]
fn a_frame_is_awaited_only_while_a_window_has_asked_to_be_told_of_one() {
    let (mut session, mut client, window) = settled();
    assert!(!awaited(&session), "nothing asked");

    // Asked with nothing changed: nothing is drawn, so no frame reaching
    // the screen will say so, and the backend has to, a frame later.
    client.request_frame(&window.surface);
    session.roundtrip(&mut client);
    assert!(!session.render(), "nothing changed");
    assert!(awaited(&session));

    session.send_frames(Duration::from_millis(32));
    assert!(!awaited(&session), "told, and nothing asked since");
}

#[test]
fn a_frame_a_window_on_a_workspace_not_shown_asked_for_is_not_awaited() {
    let (mut session, mut client, window) = settled_in(Session::with_workspaces());
    session.compositor_mut().perform(&Action::NextWorkspace);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
    // Once the workspaces have slid, and the window is off the screen.
    session.advance(Duration::from_secs(1));
    session.eventually(|session| !session.render());
    session.send_frames(Duration::from_millis(32));

    // It is told of no frame while it is not shown, so waiting for it would
    // be waiting for good.
    client.request_frame(&window.surface);
    session.roundtrip(&mut client);
    assert!(!awaited(&session));
}

#[test]
fn a_frame_a_panel_asked_for_is_awaited() {
    let (mut session, mut client, _window) = settled();
    let (panel, _role) = session.open_layer(&mut client, &panel(), (WIDTH, 30), 0xffff_ff00);
    session.eventually(|session| !session.render());
    session.send_frames(Duration::from_millis(32));
    assert!(!awaited(&session), "nothing asked");

    client.request_frame(&panel);
    session.roundtrip(&mut client);
    assert!(awaited(&session));
    session.send_frames(Duration::from_millis(48));
    assert!(!awaited(&session), "told");
}

#[test]
fn a_frame_the_picture_a_program_gave_the_pointer_asked_for_is_awaited() {
    let (mut session, mut client, _window) = settled();
    theme(&mut session);
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut client);
    let picture = client.surface();
    client.set_cursor(Some(&picture), (2, 2));
    client.fill(&picture, (6, 6), 0xffff_ffff);
    session.roundtrip(&mut client);
    session.eventually(|session| !session.render());
    session.send_frames(Duration::from_millis(32));
    assert!(!awaited(&session), "nothing asked");

    client.request_frame(&picture);
    session.roundtrip(&mut client);
    assert!(awaited(&session));
    session.send_frames(Duration::from_millis(48));
    assert!(!awaited(&session), "told");
}

#[test]
fn a_frame_asked_for_on_one_screen_is_not_awaited_on_another() {
    let (mut session, mut client, window) = settled();
    let second = session.add_output((800, 600));
    session.eventually(|session| !session.render());
    session.send_frames(Duration::from_millis(32));

    client.request_frame(&window.surface);
    session.roundtrip(&mut client);
    assert!(awaited(&session), "where the window is");
    assert!(!session.compositor().awaits_frame(&second));
}
