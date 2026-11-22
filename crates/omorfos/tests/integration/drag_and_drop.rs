//! Dragging something from one application and dropping it on another.

use omorfos::Input;
use omorfos_config::{Action, Half};

use crate::common::{Client, Event, Session, read_text};
use crate::windows::{BLUE, RED};

const GREEN: u32 = 0xff00_ff00;

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
    from.drag(&window.surface, None, "dragged from the editor");
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
    to.accept_drag(false);
    session.roundtrip(&mut to);

    // Let go there, it is dropped there, and what was dragged arrives.
    button(&mut session, false);
    session.roundtrip(&mut to);
    assert!(to.take_events().contains(&Event::Dropped));
    let pipe = to.take_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);
    let dropped = read_text(pipe);
    assert_eq!(dropped, "dragged from the editor");

    // Once the other says it has it, the first is told it was taken.
    to.finish_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);
    let told = from.take_events();
    assert!(told.contains(&Event::DropPerformed), "{told:?}");
    assert!(told.contains(&Event::DragFinished), "{told:?}");
    assert!(!told.contains(&Event::DragMoves(true)), "copied: {told:?}");
}

#[test]
fn what_is_dropped_where_it_is_to_be_moved_is_told_to_its_program_as_moved() {
    let mut session = Session::new();
    let (mut from, mut to, window) = side_by_side(&mut session);
    start_dragging(&mut session, &mut from, &window);
    move_to(&mut session, 420, 50);
    session.roundtrip(&mut to);
    // Taken to be moved, as a file dragged to another folder is.
    to.accept_drag(true);
    session.roundtrip(&mut to);
    button(&mut session, false);
    session.roundtrip(&mut to);
    let pipe = to.take_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);
    // Read to its end, as a program does before it says it is done.
    let _ = read_text(pipe);
    to.finish_drop();
    session.roundtrip(&mut to);
    session.roundtrip(&mut from);

    // Its program is told so, for it to let go of what it moved.
    let told = from.take_events();
    assert!(told.contains(&Event::DragMoves(true)), "{told:?}");
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

#[test]
fn what_is_dragged_is_shown_by_its_icon_under_the_pointer_until_it_is_let_go() {
    let mut session = Session::new();
    let (mut from, _to, window) = side_by_side(&mut session);
    move_to(&mut session, 100, 50);
    button(&mut session, true);
    session.roundtrip(&mut from);
    let icon = from.surface();
    from.drag(&window.surface, Some(&icon), "dragged with an icon");
    from.fill(&icon, (40, 40), GREEN);
    session.roundtrip(&mut from);

    // Over the empty part of the right half, the icon hangs from the
    // pointer, beyond where the pointer itself is drawn.
    move_to(&mut session, 420, 300);
    assert_eq!(session.draw().at(452, 332), GREEN);
    // Its program may have it hang elsewhere: here, up and to the left.
    from.fill_moved(&icon, (40, 40), GREEN, (-30, -30));
    session.roundtrip(&mut from);
    let picture = session.draw();
    assert_eq!(picture.at(400, 280), GREEN, "moved up and to the left");
    assert_ne!(picture.at(452, 332), GREEN, "and no longer down there");
    // A picture that moves is told when to draw the next, as a window is.
    from.take_events();
    from.request_frame(&icon);
    session.roundtrip(&mut from);
    session.send_frames(std::time::Duration::from_millis(16));
    session.roundtrip(&mut from);
    assert!(from.take_events().contains(&Event::FrameDone));
    button(&mut session, false);
    session.roundtrip(&mut from);
    assert_ne!(session.draw().at(400, 280), GREEN, "gone once let go");
}
