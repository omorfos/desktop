//! A session with nothing changing: the third priority, no work when
//! nothing changes, held as a gate.

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;

use crate::common::{Client, Session, Toplevel, WIDTH};
use crate::cursors::theme;
use crate::layers::panel;

/// The most a turn of a session with nothing changing may allocate, which
/// is what it allocates today, with one window in a frame of ours whose
/// titlebar is solid: 15 in drawing, what is drawn being made anew each
/// time, much of it Smithay's elements for what the program drew; and 2
/// in Smithay's refreshing of the space. Each is let go of within the turn. It is to come down to
/// nothing, and is lowered as each goes; never raised.
pub(crate) const CEILING: u64 = 17;

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
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    let mut client = session.connect();
    let _window = session.open_decorated(&mut client, "app", (200, 100), 0xff00_ff00);
    let told = session.ask(&mut peer, &omorfos_ipc::Request::Subscribe);
    assert!(matches!(told, omorfos_ipc::Reply::State(_)), "{told:?}");
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
