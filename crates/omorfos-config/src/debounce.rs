//! Coalescing bursts of change notifications.

use std::time::{Duration, Instant};

/// Turns a burst of triggers into a single action once things go quiet.
///
/// Editors often save a file in several steps, and users often save several
/// files in a row. Reloading on every event would parse half-written files,
/// so the reload waits until no change has been seen for a short delay.
///
/// The debouncer never reads the clock itself: callers pass the current time
/// in, which keeps it deterministic and lets an event loop own the timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Debouncer {
    delay: Duration,
    deadline: Option<Instant>,
}

impl Debouncer {
    /// Creates an idle debouncer that fires `delay` after the last trigger.
    #[must_use]
    pub const fn new(delay: Duration) -> Self {
        Self {
            delay,
            deadline: None,
        }
    }

    /// Records a change at `now`, pushing the deadline back.
    pub fn trigger(&mut self, now: Instant) {
        self.deadline = now.checked_add(self.delay);
    }

    /// When [`Debouncer::fire`] should next be called, if a change is pending.
    #[must_use]
    pub const fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Returns `true` exactly once per burst, when the deadline has passed.
    pub fn fire(&mut self, now: Instant) -> bool {
        let due = self.deadline.is_some_and(|deadline| now >= deadline);
        if due {
            self.deadline = None;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELAY: Duration = Duration::from_millis(100);

    #[test]
    fn an_idle_debouncer_has_no_deadline_and_never_fires() {
        let mut debouncer = Debouncer::new(DELAY);
        assert_eq!(debouncer.deadline(), None);
        assert!(!debouncer.fire(Instant::now()));
    }

    #[test]
    fn fires_once_after_the_delay_has_elapsed() {
        let start = Instant::now();
        let mut debouncer = Debouncer::new(DELAY);
        debouncer.trigger(start);
        assert_eq!(debouncer.deadline(), Some(start + DELAY));
        assert!(!debouncer.fire(start + DELAY / 2));
        assert!(debouncer.fire(start + DELAY));
        assert!(!debouncer.fire(start + DELAY * 2));
        assert_eq!(debouncer.deadline(), None);
    }

    #[test]
    fn a_new_trigger_pushes_the_deadline_back() {
        let start = Instant::now();
        let mut debouncer = Debouncer::new(DELAY);
        debouncer.trigger(start);
        debouncer.trigger(start + DELAY / 2);
        assert!(!debouncer.fire(start + DELAY));
        assert!(debouncer.fire(start + DELAY + DELAY / 2));
    }
}
