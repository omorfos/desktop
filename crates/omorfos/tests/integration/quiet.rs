//! A session with nothing changing: the third priority, no work when
//! nothing changes, held as a gate.

use std::time::Duration;

use crate::common::Session;

/// The most a turn of a session with nothing changing may allocate, which
/// is what it allocates today, with one window in a frame of ours: 6 in
/// `sync`, painting the frame's titlebar again; 23 in drawing, its frame,
/// its rounded corners and Smithay's elements for what the program drew;
/// and the rest in `reveal`, `update_seal` and Smithay's refreshing of the
/// space. Each is let go of within the turn. It is to come down to
/// nothing, and is lowered as each goes; never raised.
pub(crate) const CEILING: u64 = 34;

#[test]
fn a_session_with_nothing_changing_draws_nothing_wakes_for_nothing_and_allocates_little() {
    let mut session = Session::new();
    let mut client = session.connect();
    let _window = session.open_decorated(&mut client, "app", (200, 100), 0xff00_ff00);
    // In a frame of ours, as most windows are. Whatever was coming in has
    // come in.
    session.eventually(|session| !session.render());

    for turn in 0..10 {
        // Nothing is due, so the loop would sleep until something happens.
        assert_eq!(session.compositor().deadline(), None, "turn {turn}");
        let mut drawn = true;
        let made = allocation_counter::measure(|| {
            session.advance(Duration::from_millis(16));
            drawn = session.render();
        });
        assert!(!drawn, "turn {turn} drew");
        assert_eq!(made.count_current, 0, "turn {turn} kept what it made");
        assert!(
            made.count_total <= CEILING,
            "turn {turn} allocated {} times, more than {CEILING}",
            made.count_total
        );
    }
}
