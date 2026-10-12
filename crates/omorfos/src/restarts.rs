//! Starting again what has stopped, but not for ever.
//!
//! The X server, the panel and the notifications are each started again
//! if they stop when they should not have. Something that stops as soon as
//! it is started would otherwise be started over and over, so each is
//! allowed only so many restarts in a while before it is given up on.

use std::time::{Duration, Instant};

/// How many times something is started again, within [`LATELY`], before it
/// is given up on.
const MOST: usize = 3;

/// How long a restart counts against the next.
const LATELY: Duration = Duration::from_secs(60);

/// When something was started again after stopping, lately.
#[derive(Debug, Default)]
pub(crate) struct Restarts(Vec<Instant>);

impl Restarts {
    /// Whether what has just stopped, at `now`, may be started again. If
    /// so, this counts as one more time that it was.
    pub(crate) fn allow(&mut self, now: Instant) -> bool {
        self.0
            .retain(|at| now.saturating_duration_since(*at) < LATELY);
        let allowed = self.0.len() < MOST;
        if allowed {
            self.0.push(now);
        }
        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_stops_is_started_again_three_times_in_a_minute_and_no_more() {
        let start = Instant::now();
        let after = |seconds| start + Duration::from_secs(seconds);
        let mut restarts = Restarts::default();
        assert!(restarts.allow(after(0)));
        assert!(restarts.allow(after(1)));
        assert!(restarts.allow(after(2)));
        assert!(!restarts.allow(after(3)), "given up on");
        assert!(!restarts.allow(after(59)), "and that does not count");
    }

    #[test]
    fn what_has_gone_a_while_without_stopping_is_trusted_again() {
        let start = Instant::now();
        let after = |seconds| start + Duration::from_secs(seconds);
        let mut restarts = Restarts::default();
        for second in 0..3 {
            assert!(restarts.allow(after(second)));
        }
        assert!(!restarts.allow(after(30)));
        assert!(restarts.allow(after(60)), "the first is forgotten");
        assert!(!restarts.allow(after(60)), "the others are not yet");
        assert!(restarts.allow(after(200)));
    }
}
