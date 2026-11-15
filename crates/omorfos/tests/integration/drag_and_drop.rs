//! Dragging something from one application and dropping it on another.

use std::io::Read;

use omorfos::Input;
use omorfos_config::{Action, Half};

use crate::common::{Client, Event, Session};
use crate::windows::{BLUE, RED};

const LEFT: u32 = 0x110;

/// Two applications side by side, the first on the left half of the
/// screen and the second on the right, each with a window 200 by 100 in
/// the top corner of its half; with the first's window.
fn side_by_side(session: &mut Session) -> (Client, Client, crate::common::Toplevel) {
    let mut from = session.connect();
    let window = session.open(&mut from, "editor", (200, 100), RED);
    session.compositor_mut().perform(&Action::Tile(Half::Left));
    session.roundtrip(&mut from);
    let mut to = session.connect();
    session.open(&mut to, "terminal", (200, 100), BLUE);
    (from, to, window)
}

fn move_to(session: &mut Session, x: i32, y: i32) {
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
}

fn button(session: &mut Session, pressed: bool) {
    let button = LEFT;
    session
        .compositor_mut()
        .input(Input::Button { button, pressed }, 0);
    session.step();
}

/// Presses the pointer on the first application's window and has it start
/// dragging `text` from there, as one does once the pointer moves.
fn start_dragging(session: &mut Session, from: &mut Client, window: &crate::common::Toplevel) {
    move_to(session, 100, 50);
    button(session, true);
    session.roundtrip(from);
    from.drag(&window.surface, "dragged from the editor");
    session.roundtrip(from);
}

#[test]
fn text_dragged_from_one_application_can_be_dropped_on_another() {
    let mut session = Session::new();
    let (mut from, mut to, window) = side_by_side(&mut session);
    start_dragging(&mut session, &mut from, &window);

    // Over the other, it is offered what is dragged, and takes it.
    move_to(&mut session, 420, 50);
    session.roundtrip(&mut to);
    assert!(to.take_events().contains(&Event::DragEntered));
    assert_eq!(to.state.formats, ["text/plain;charset=utf-8"]);
    to.accept_drag();
    session.roundtrip(&mut to);

    // Let go there, it is dropped there, and what was dragged arrives.
    button(&mut session, false);
    session.roundtrip(&mut to);
    assert!(to.take_events().contains(&Event::Dropped));
    let mut pipe = to.take_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);
    let mut dropped = String::new();
    pipe.read_to_string(&mut dropped).unwrap();
    assert_eq!(dropped, "dragged from the editor");

    // Once the other says it has it, the first is told it was taken.
    to.finish_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);
    let told = from.take_events();
    assert!(told.contains(&Event::DropPerformed), "{told:?}");
    assert!(told.contains(&Event::DragFinished), "{told:?}");
}

#[test]
fn what_is_let_go_where_nothing_takes_it_is_given_back() {
    let mut session = Session::new();
    let (mut from, mut to, window) = side_by_side(&mut session);
    start_dragging(&mut session, &mut from, &window);

    // Over the other, which does not take it, and let go there.
    move_to(&mut session, 420, 50);
    session.roundtrip(&mut to);
    assert!(to.take_events().contains(&Event::DragEntered));
    button(&mut session, false);
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);

    assert!(!to.take_events().contains(&Event::Dropped));
    let told = from.take_events();
    assert!(told.contains(&Event::SourceCancelled), "{told:?}");
    assert!(!told.contains(&Event::DropPerformed), "{told:?}");
}
