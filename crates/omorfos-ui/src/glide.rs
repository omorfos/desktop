//! Something on its way from one place to another, as a notification is
//! when the one above it goes and it moves up into the room: where it is
//! at each moment, easing in and out over the time `docs/DESIGN.md` gives
//! something moving to where it was sent.
//!
//! Time is whatever whoever asks says it is.

use std::time::Duration;

use omorfos_config::Easing;

/// Where something is, along one line, and where it is going.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glide {
    /// Where it set out from.
    from: f64,
    /// Where it is going, and will stay.
    to: i32,
    /// When it set out.
    began: Duration,
}

impl Glide {
    /// How long it takes to get where it is going.
    pub const TAKES: Duration = Duration::from_millis(240);

    /// Something at `place`, going nowhere.
    #[must_use]
    pub fn at(place: i32) -> Self {
        Self {
            from: f64::from(place),
            to: place,
            began: Duration::ZERO,
        }
    }

    /// Where it is going.
    #[must_use]
    pub const fn to(&self) -> i32 {
        self.to
    }

    /// Where it is at `now`: on its way, or where it was going once it has
    /// got there.
    #[must_use]
    pub fn now(&self, now: Duration) -> i32 {
        let gone = now.saturating_sub(self.began).as_secs_f32();
        let share = (gone / Self::TAKES.as_secs_f32()).min(1.0);
        let along = f64::from(Easing::EaseInOut.at(share));
        let place = (f64::from(self.to) - self.from).mul_add(along, self.from);
        whole(place)
    }

    /// Whether it is still on its way at `now`.
    #[must_use]
    pub fn moving(&self, now: Duration) -> bool {
        self.now(now) != self.to
    }

    /// Sends it to `place` from wherever it is at `now`, if it `moves`; and
    /// otherwise puts it there at once. Sent where it is going already, it
    /// goes on as it was.
    pub fn go(&mut self, place: i32, now: Duration, moves: bool) {
        if place == self.to {
            return;
        }
        let here = if moves { self.now(now) } else { place };
        *self = Self {
            from: f64::from(here),
            to: place,
            began: now,
        };
    }
}

/// The whole number nearest to `place`, which is between two places that
/// are whole numbers.
#[expect(
    clippy::cast_possible_truncation,
    reason = "it lies between two whole numbers that are i32s"
)]
fn whole(place: f64) -> i32 {
    place.round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(count: u64) -> Duration {
        Duration::from_millis(count)
    }

    #[test]
    fn something_put_somewhere_stays_there() {
        let glide = Glide::at(40);
        assert_eq!((glide.now(ms(0)), glide.now(ms(1000))), (40, 40));
        assert_eq!(glide.to(), 40);
        assert!(!glide.moving(ms(0)));
    }

    #[test]
    fn sent_elsewhere_it_eases_there_over_the_time_it_takes() {
        let mut glide = Glide::at(100);
        glide.go(0, ms(1000), true);
        assert_eq!(glide.to(), 0);
        assert_eq!(glide.now(ms(1000)), 100, "setting out");
        assert!(glide.moving(ms(1000)));
        let early = glide.now(ms(1060));
        assert!(early > 75, "slow at first: {early}");
        assert_eq!(glide.now(ms(1120)), 50, "half way in half the time");
        let late = glide.now(ms(1180));
        assert!(late < 25, "slow at the end: {late}");
        assert_eq!(glide.now(ms(1240)), 0, "there");
        assert!(!glide.moving(ms(1240)));
        assert_eq!(glide.now(ms(5000)), 0, "and stays");
    }

    #[test]
    fn sent_again_on_its_way_it_sets_out_from_where_it_has_got_to() {
        let mut glide = Glide::at(100);
        glide.go(0, ms(0), true);
        let there = glide.now(ms(120));
        glide.go(200, ms(120), true);
        assert_eq!(glide.now(ms(120)), there, "from where it was");
        assert_eq!(glide.now(ms(360)), 200);
        // Sent where it is going already, it keeps its pace.
        let mut glide = Glide::at(100);
        glide.go(0, ms(0), true);
        glide.go(0, ms(120), true);
        assert_eq!(glide.now(ms(120)), 50);
    }

    #[test]
    fn what_does_not_move_is_put_there_at_once() {
        let mut glide = Glide::at(100);
        glide.go(0, ms(1000), false);
        assert_eq!(glide.now(ms(1000)), 0);
        assert!(!glide.moving(ms(1000)));
    }
}
