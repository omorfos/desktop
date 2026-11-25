//! A text field's cursor blinking: shown and hidden by turns while the
//! field waits to be typed into, and steady once it has waited a while,
//! so that a field left alone costs nothing to keep drawn.
//!
//! It is shown the moment anything is typed, and blinks again from there.
//! Where motion is to be reduced it does not blink at all.

use std::time::{Duration, Instant};

/// How long the cursor is shown, and then hidden, each time.
pub const PHASE: Duration = Duration::from_millis(500);

/// How long it blinks after the last key before it is left shown, steady.
pub const STEADY_AFTER: Duration = Duration::from_secs(10);

/// A moment on whichever clock a program keeps: the time since it started,
/// or the system's own.
pub trait Moment: Copy {
    /// How long after `earlier` this is; none if it is not after it.
    fn since(self, earlier: Self) -> Duration;
    /// The moment `by` after this one.
    #[must_use]
    fn after(self, by: Duration) -> Self;
}

impl Moment for Duration {
    fn since(self, earlier: Self) -> Duration {
        self.saturating_sub(earlier)
    }

    fn after(self, by: Duration) -> Self {
        self.saturating_add(by)
    }
}

impl Moment for Instant {
    fn since(self, earlier: Self) -> Duration {
        self.saturating_duration_since(earlier)
    }

    fn after(self, by: Duration) -> Self {
        // So far from now that it is never reached either way.
        self.checked_add(by).unwrap_or(self)
    }
}

/// A cursor's blinking, from the last key typed, and whether it was drawn
/// last time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blink<T> {
    /// When it was last typed into, and began to blink again.
    since: T,
    /// Whether it does not blink at all, motion being reduced.
    still: bool,
    /// Whether it was shown as what it is in was last drawn.
    drawn: bool,
}

impl<T: Moment> Blink<T> {
    /// A cursor that begins to blink at `now`, or, if `still`, is shown
    /// and does not blink.
    #[must_use]
    pub const fn new(now: T, still: bool) -> Self {
        Self {
            since: now,
            still,
            drawn: true,
        }
    }

    /// Shows it at `now`, as a key typed does, and blinks again from there.
    pub const fn restart(&mut self, now: T) {
        self.since = now;
    }

    /// Whether it is shown at `now`.
    #[must_use]
    pub fn shown(&self, now: T) -> bool {
        let waited = now.since(self.since);
        let phases = waited.as_millis() / PHASE.as_millis();
        self.still || waited >= STEADY_AFTER || phases.is_multiple_of(2)
    }

    /// Whether it is to be drawn at `now`, noted as what was drawn.
    pub fn draw(&mut self, now: T) -> bool {
        self.drawn = self.shown(now);
        self.drawn
    }

    /// Whether what it is in is to be drawn again at `now`, the cursor
    /// shown where it was hidden as it was last drawn, or hidden where it
    /// was shown.
    #[must_use]
    pub fn changed(&self, now: T) -> bool {
        self.shown(now) != self.drawn
    }

    /// When it is next shown or hidden, after `now`; none once it is
    /// steady, or if it does not blink.
    #[must_use]
    pub fn deadline(&self, now: T) -> Option<T> {
        let waited = now.since(self.since);
        let phases = waited.as_millis() / PHASE.as_millis();
        let next = PHASE.saturating_mul(u32::try_from(phases + 1).unwrap_or(u32::MAX));
        let blinking = !self.still && waited < STEADY_AFTER;
        blinking.then(|| self.since.after(next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn at(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn the_cursor_is_shown_and_hidden_by_turns_from_the_last_key() {
        let mut blink = Blink::new(at(1000), false);
        assert!(blink.shown(at(1000)));
        assert!(blink.shown(at(1499)));
        assert!(!blink.shown(at(1500)));
        assert!(blink.shown(at(2000)));
        assert_eq!(blink.deadline(at(1000)), Some(at(1500)));
        assert_eq!(blink.deadline(at(1700)), Some(at(2000)));
        // Drawn hidden, it is to be drawn again once it is shown.
        assert!(!blink.draw(at(1700)));
        assert!(!blink.changed(at(1800)));
        assert!(blink.changed(at(2000)));
        // A key typed while it is hidden shows it at once.
        blink.restart(at(1600));
        assert!(blink.shown(at(1600)));
        assert_eq!(blink.deadline(at(1600)), Some(at(2100)));
    }

    #[test]
    fn left_alone_a_while_it_is_shown_steady_and_wakes_nothing() {
        let blink = Blink::new(at(0), false);
        let last = STEADY_AFTER.saturating_sub(PHASE);
        assert!(!blink.shown(last + at(1)), "still blinking");
        assert_eq!(blink.deadline(last + at(1)), Some(STEADY_AFTER));
        assert!(blink.shown(STEADY_AFTER));
        assert!(blink.shown(STEADY_AFTER * 3 + PHASE));
        assert_eq!(blink.deadline(STEADY_AFTER), None);
    }

    #[test]
    fn with_motion_reduced_it_never_blinks() {
        let blink = Blink::new(at(0), true);
        assert!(blink.shown(PHASE));
        assert_eq!(blink.deadline(at(0)), None);
    }

    #[test]
    fn it_keeps_time_by_the_systems_clock_as_well() {
        let start = Instant::now();
        let blink = Blink::new(start, false);
        assert!(!blink.shown(start + PHASE));
        assert_eq!(blink.deadline(start), Some(start + PHASE));
        // A clock that went back is taken as no time at all.
        assert!(Blink::new(start + PHASE, false).shown(start));
        assert_eq!(at(5).since(at(10)), Duration::ZERO);
    }
}
