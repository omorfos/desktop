//! A screen that is off: it stands nowhere, shows nothing, and programs
//! are not told of it; but not every screen plugged in is ever off.

use crate::common::{HEIGHT, Left, Session, WIDTH};
use crate::windows::{BACKGROUND, BLUE};

const BLACK: u32 = 0xff00_0000;

/// The second screen plugged in is off.
const OFF: &str = "outputs:\n  - { name: test-0 }\n  - { name: test-1, enabled: false }\n";

/// The screens as `omorfosctl screens` lists them: each name, and whether
/// it is off.
fn listed(session: &mut Session, peer: &mut omorfos_ipc::Client) -> Vec<(String, bool)> {
    let omorfos_ipc::Reply::State(state) = session.ask(peer, &omorfos_ipc::Request::State) else {
        unreachable!("the state was asked for");
    };
    let screens = state.screens.into_iter();
    screens.map(|screen| (screen.name, screen.off)).collect()
}

#[test]
fn a_screen_that_is_off_stands_nowhere_and_shows_nothing_until_it_is_on() {
    let mut session = Session::with_config(&[("config.yaml", OFF)]);
    let second = session.add_output((800, 600));
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    assert!(session.compositor().is_dark(&second));
    assert!(session.compositor().area_of(&second).is_empty());
    assert_eq!(
        session.connect().state.outputs,
        1,
        "programs are not told of it"
    );
    assert_eq!(session.draw_on(1).at(10, 10), BLACK);
    let names = [("test-0".to_owned(), false), ("test-1".to_owned(), true)];
    assert_eq!(listed(&mut session, &mut peer), names);

    session.write_config("config.yaml", "");
    session.eventually(|session| session.compositor().area_of(&second).loc.x == WIDTH);
    assert!(!session.compositor().is_dark(&second));
    assert_eq!(session.connect().state.outputs, 2);
    assert_eq!(session.draw_on(1).at(10, 10), BACKGROUND);

    // Unplugged while off, it is gone like any other.
    session.write_config("config.yaml", OFF);
    session.eventually(|session| session.compositor().is_dark(&second));
    session.compositor_mut().remove_output(&second);
    assert!(!session.compositor().is_dark(&second));
    assert_eq!(listed(&mut session, &mut peer).len(), 1);
}

#[test]
fn the_first_screen_plugged_in_is_left_on_rather_than_have_none() {
    let config = "outputs:\n  - { name: test-0, enabled: false }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    let first = session.output();
    assert!(!session.compositor().is_dark(&first));
    let area = session.compositor().area_of(&first);
    assert_eq!((area.size.w, area.size.h), (WIDTH, HEIGHT));
    session.step();
    let said = session.compositor().events().any(|event| {
        event.contains("every screen plugged in is to be off, so `test-0` is left on")
    });
    assert!(said);
    // With another plugged in, that one is on and this one can be off.
    let second = session.add_output((800, 600));
    assert!(session.compositor().is_dark(&first));
    assert!(!session.compositor().is_dark(&second));
    assert_eq!(session.draw().at(10, 10), BLACK);
    assert_eq!(session.draw_on(1).at(10, 10), BACKGROUND);
}

#[test]
fn a_session_is_locked_without_waiting_for_a_screen_that_is_off() {
    let mut session = Session::with_config(&[("config.yaml", OFF)]);
    session.add_output((800, 600));
    // Locked as on one screen: nothing is drawn on the other.
    let _locked = crate::lock::lock(&mut session);
}

#[test]
fn a_window_is_told_it_has_left_a_screen_before_the_screen_goes() {
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    let on = session.compositor().area_of(&second);
    let motion = omorfos::Input::Motion {
        x: f64::from(on.loc.x + 100),
        y: f64::from(on.loc.y + 100),
    };
    session.compositor_mut().input(motion, 0);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), BLUE);
    session.draw_all();
    session.write_config("config.yaml", OFF);
    session.eventually(|session| session.compositor().is_dark(&second));
    session.roundtrip(&mut client);
    let left = &client.state.outputs_left;
    assert_eq!(left.first(), Some(&Left::Surface), "{left:?}");
    assert!(left.contains(&Left::Gone), "{left:?}");
}
