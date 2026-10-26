//! What the holder does when the compositor ends, which is all it decides.

use std::time::{Duration, Instant};

use omorfos_ipc::hold::{Told, Word};

/// How many times the compositor is started again after failing, within
/// [`LATELY`], before the version held open is tried, and after that
/// before the session is given up.
const MOST: usize = 3;

/// How long a failure counts against the next.
const LATELY: Duration = Duration::from_secs(60);

/// How long after failing the compositor is started again: long enough for
/// the seat, which a compositor that crashed may not yet have let go of.
pub(crate) const PAUSE: Duration = Duration::from_secs(1);

/// Which file of the compositor to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The one installed now.
    Installed,
    /// The one that last came up, held open: the one installed would not.
    Held,
}

/// What the holder is to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Start the compositor, this one, once `after` has passed.
    Start {
        /// Which file of it.
        which: Which,
        /// How long to wait first.
        after: Duration,
    },
    /// End the session: as the compositor did, or `failed`, having given
    /// it up.
    End {
        /// Whether it was given up.
        failed: bool,
    },
}

/// What the holder knows of the session, and of the compositor running.
#[derive(Debug)]
pub struct Policy {
    /// What the next compositor is to be told.
    told: Told,
    /// Whether a compositor has been started in this login yet.
    started: bool,
    /// Which file the compositor running was started from.
    which: Which,
    /// Whether the compositor running has said it is up, or stepping down.
    up: bool,
    stepping_down: bool,
    /// When the compositor failed, lately.
    failures: Vec<Instant>,
    pause: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self::with_pause(PAUSE)
    }
}

impl Policy {
    /// Knows nothing yet, and waits `pause` after a failure before starting
    /// the compositor again.
    #[must_use]
    pub fn with_pause(pause: Duration) -> Self {
        Self {
            told: Told::default(),
            started: false,
            which: Which::Installed,
            up: false,
            stepping_down: false,
            failures: Vec::new(),
            pause,
        }
    }

    /// Takes up from another version of the holder what it was `told` by
    /// the compositors before, in a login in which one has been started.
    #[must_use]
    pub fn resumed(told: Told) -> Self {
        Self {
            told: Told {
                again: false,
                older: false,
                ..told
            },
            started: true,
            ..Self::default()
        }
    }

    /// What the compositors before have told, for the next.
    #[must_use]
    pub const fn told(&self) -> Told {
        self.told
    }

    /// What the compositor about to be started from `which` is to be told.
    pub fn starting(&mut self, which: Which) -> Told {
        let told = Told {
            again: self.started,
            older: which == Which::Held,
            ..self.told
        };
        self.started = true;
        self.which = which;
        self.up = false;
        self.stepping_down = false;
        told
    }

    /// Takes in what the compositor running said.
    pub fn heard(&mut self, word: Word) {
        match word {
            Word::Up => self.up = true,
            Word::Locked => self.told.locked = true,
            Word::Unlocked => self.told.locked = false,
            Word::Display(display) => self.told.display = Some(display),
            Word::SteppingDown => self.stepping_down = true,
        }
    }

    /// Whether the compositor running has said it is up.
    #[must_use]
    pub const fn is_up(&self) -> bool {
        self.up
    }

    /// What to do now that the compositor running has ended at `now`,
    /// having `failed` or not. `other` says whether the file held open is
    /// another than the one installed now.
    pub fn ended(&mut self, failed: bool, now: Instant, other: bool) -> Next {
        // One stepping down is started again at once, in its newest
        // version, which is why it stepped down.
        if self.stepping_down {
            return Next::Start {
                which: Which::Installed,
                after: Duration::ZERO,
            };
        }
        // One that came up and ended without failing ended the session, as
        // logging out does.
        if self.up && !failed {
            return Next::End { failed: false };
        }
        self.failures
            .retain(|at| now.saturating_duration_since(*at) < LATELY);
        self.failures.push(now);
        let after = self.pause;
        if self.failures.len() <= MOST {
            return Next::Start {
                which: self.which,
                after,
            };
        }
        // The one installed keeps failing: the one that came up before is
        // tried, if it is another, as often.
        if self.which == Which::Installed && other {
            self.failures.clear();
            return Next::Start {
                which: Which::Held,
                after,
            };
        }
        Next::End { failed: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> Instant {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *START.get_or_init(Instant::now) + Duration::from_secs(seconds)
    }

    fn again(which: Which) -> Next {
        Next::Start {
            which,
            after: PAUSE,
        }
    }

    #[test]
    fn the_first_compositor_is_told_nothing_and_the_next_that_it_is_not_the_first() {
        let mut policy = Policy::default();
        assert_eq!(policy.starting(Which::Installed), Told::default());
        policy.heard(Word::Up);
        assert!(policy.is_up());
        let told = policy.starting(Which::Installed);
        assert!(told.again);
        assert!(!policy.is_up(), "the new one has yet to say");
    }

    #[test]
    fn what_a_compositor_says_is_told_to_the_next() {
        let mut policy = Policy::default();
        policy.starting(Which::Installed);
        policy.heard(Word::Display(2));
        policy.heard(Word::Locked);
        let told = policy.starting(Which::Installed);
        assert_eq!((told.locked, told.display), (true, Some(2)));
        policy.heard(Word::Unlocked);
        policy.heard(Word::Display(5));
        let told = policy.starting(Which::Installed);
        assert_eq!((told.locked, told.display), (false, Some(5)));
    }

    #[test]
    fn one_that_steps_down_is_started_again_at_once_in_its_newest_version() {
        let mut policy = Policy::default();
        policy.starting(Which::Held);
        policy.heard(Word::Up);
        policy.heard(Word::SteppingDown);
        let started = Next::Start {
            which: Which::Installed,
            after: Duration::ZERO,
        };
        assert_eq!(policy.ended(false, at(0), true), started);
        // Even failing as it goes: it said what it was doing.
        policy.starting(Which::Installed);
        policy.heard(Word::SteppingDown);
        assert_eq!(policy.ended(true, at(1), true), started);
    }

    #[test]
    fn one_that_came_up_and_ends_without_failing_ends_the_session() {
        let mut policy = Policy::default();
        policy.starting(Which::Installed);
        policy.heard(Word::Up);
        assert_eq!(
            policy.ended(false, at(0), true),
            Next::End { failed: false }
        );
    }

    #[test]
    fn one_that_fails_is_started_again_after_a_pause_three_times_in_a_minute() {
        let mut policy = Policy::default();
        for second in 0..3 {
            policy.starting(Which::Installed);
            policy.heard(Word::Up);
            let next = policy.ended(true, at(second), false);
            assert_eq!(next, again(Which::Installed), "{second}");
        }
        policy.starting(Which::Installed);
        assert_eq!(policy.ended(true, at(3), false), Next::End { failed: true });
    }

    #[test]
    fn one_that_ends_before_it_is_up_did_not_come_up_and_counts_as_failing() {
        let mut policy = Policy::with_pause(Duration::from_millis(5));
        policy.starting(Which::Installed);
        let next = policy.ended(false, at(0), false);
        let paused = Next::Start {
            which: Which::Installed,
            after: Duration::from_millis(5),
        };
        assert_eq!(next, paused);
    }

    #[test]
    fn failures_long_ago_are_forgotten() {
        let mut policy = Policy::default();
        for second in [0, 1, 2] {
            policy.starting(Which::Installed);
            policy.ended(true, at(second), false);
        }
        policy.starting(Which::Installed);
        assert_eq!(policy.ended(true, at(61), false), again(Which::Installed));
    }

    #[test]
    fn a_new_version_that_keeps_failing_gives_way_to_the_one_that_came_up_before() {
        let mut policy = Policy::default();
        for second in 0..3 {
            policy.starting(Which::Installed);
            policy.ended(true, at(second), true);
        }
        policy.starting(Which::Installed);
        assert_eq!(policy.ended(true, at(3), true), again(Which::Held));
        // Which is told it is older, and tried as often in turn.
        assert!(policy.starting(Which::Held).older);
        for second in 4..6 {
            assert_eq!(policy.ended(true, at(second), true), again(Which::Held));
            policy.starting(Which::Held);
        }
        assert_eq!(policy.ended(true, at(6), true), again(Which::Held));
        policy.starting(Which::Held);
        assert_eq!(policy.ended(true, at(7), true), Next::End { failed: true });
    }
}
