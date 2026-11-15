//! When a tooltip shows: once the pointer has rested a moment on something
//! that has one, at once on the next while one is showing, and not again on
//! what was clicked until the pointer has left it.
//!
//! What has a tooltip is told by whatever the component takes it to be,
//! such as where it is and what it says; [`Resting`] only says which of
//! them is to be shown, and when. A component shows that in a popover of
//! its own ([`crate::popover::Popovers::tip`]).

use std::time::Duration;

/// How long the pointer rests on something before its tooltip shows.
pub const DELAY: Duration = Duration::from_millis(500);

/// Where the pointer rests, and so which tooltip is shown, if any.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resting<K> {
    on: On<K>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum On<K> {
    /// On nothing that has a tooltip.
    #[default]
    Nothing,
    /// On `K` since then, its tooltip not yet shown.
    Waiting(K, Duration),
    /// On `K`, its tooltip shown.
    Shown(K),
    /// On `K`, which was clicked: its tooltip stays away.
    Clicked(K),
}

impl<K: PartialEq> Resting<K> {
    /// Nothing rested on.
    #[must_use]
    pub const fn new() -> Self {
        Self { on: On::Nothing }
    }

    /// Takes in that the pointer is over `over`, at `now`, or over nothing
    /// with a tooltip. Says whether what is shown changed: a tooltip gone,
    /// or another shown at once because one was.
    pub fn over(&mut self, over: Option<K>, now: Duration) -> bool {
        let on = std::mem::take(&mut self.on);
        let showing = matches!(on, On::Shown(_));
        let (on, changed) = match (on, over) {
            (_, None) => (On::Nothing, showing),
            (On::Waiting(was, since), Some(over)) if was == over => {
                (On::Waiting(was, since), false)
            }
            (On::Shown(was), Some(over)) if was == over => (On::Shown(was), false),
            (On::Clicked(was), Some(over)) if was == over => (On::Clicked(was), false),
            // Moving from one tooltip to the next, the next needs no wait.
            (On::Shown(_), Some(over)) => (On::Shown(over), true),
            (_, Some(over)) => (On::Waiting(over, now), false),
        };
        self.on = on;
        changed
    }

    /// Takes in that it is `now`. Says whether a tooltip waited for is
    /// now to be shown.
    pub fn at(&mut self, now: Duration) -> bool {
        let on = std::mem::take(&mut self.on);
        let (on, due) = match on {
            On::Waiting(over, since) if now >= since.saturating_add(DELAY) => {
                (On::Shown(over), true)
            }
            on => (on, false),
        };
        self.on = on;
        due
    }

    /// Takes in a click, which takes the tooltip away until the pointer
    /// leaves what it was on. Says whether one was shown.
    pub fn clicked(&mut self) -> bool {
        let on = std::mem::take(&mut self.on);
        let showing = matches!(on, On::Shown(_));
        self.on = match on {
            On::Waiting(over, _) | On::Shown(over) | On::Clicked(over) => On::Clicked(over),
            On::Nothing => On::Nothing,
        };
        showing
    }

    /// When a tooltip waited for is to be shown, if one is.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        match self.on {
            On::Waiting(_, since) => Some(since.saturating_add(DELAY)),
            _ => None,
        }
    }

    /// What the tooltip shown is of, if one is.
    #[must_use]
    pub const fn shown(&self) -> Option<&K> {
        match &self.on {
            On::Shown(over) => Some(over),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn a_tooltip_shows_once_the_pointer_has_rested_and_goes_as_it_leaves() {
        let mut resting = Resting::new();
        assert!(!resting.over(Some("clock"), ms(100)));
        assert_eq!(resting.deadline(), Some(ms(600)));
        assert!(!resting.at(ms(599)));
        assert_eq!(resting.shown(), None);
        assert!(resting.at(ms(600)));
        assert_eq!(resting.shown(), Some(&"clock"));
        assert_eq!(resting.deadline(), None);
        assert!(!resting.at(ms(900)), "shown once");

        assert!(!resting.over(Some("clock"), ms(1000)), "moving on it");
        assert_eq!(resting.shown(), Some(&"clock"));
        assert!(resting.over(None, ms(1100)));
        assert_eq!(resting.shown(), None);
        assert!(!resting.over(None, ms(1200)));
    }

    #[test]
    fn moving_on_restarts_the_wait_until_one_shows_and_then_the_next_shows_at_once() {
        let mut resting = Resting::new();
        resting.over(Some("clock"), ms(0));
        assert!(!resting.over(Some("clock"), ms(300)), "the wait goes on");
        assert!(!resting.over(Some("window"), ms(400)));
        assert!(!resting.at(ms(600)), "waited for anew");
        assert!(resting.at(ms(900)));
        assert_eq!(resting.shown(), Some(&"window"));

        assert!(resting.over(Some("clock"), ms(1000)), "at once");
        assert_eq!(resting.shown(), Some(&"clock"));
        assert_eq!(resting.deadline(), None);
    }

    #[test]
    fn a_click_takes_the_tooltip_away_until_the_pointer_leaves_what_was_clicked() {
        let mut resting = Resting::new();
        assert!(!resting.clicked(), "nothing to take away");
        resting.over(Some("clock"), ms(0));
        resting.at(ms(500));
        assert!(resting.clicked());
        assert_eq!(resting.shown(), None);
        assert!(!resting.over(Some("clock"), ms(600)));
        assert!(!resting.at(ms(2000)), "not again");
        assert!(!resting.clicked(), "clicked again");

        resting.over(Some("window"), ms(2100));
        assert!(resting.at(ms(2600)), "on another, it shows again");

        // A click before it shows keeps it away, too.
        resting.over(None, ms(2700));
        resting.over(Some("clock"), ms(2800));
        assert!(!resting.clicked());
        assert!(!resting.at(ms(4000)));
        assert_eq!(resting.deadline(), None);
    }
}
