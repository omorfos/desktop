//! Fingers on a touchpad carrying something from one way of being to
//! another: the workspaces from one to the next, the overview open or
//! closed, the windows aside or back. Each follows the fingers with no
//! easing while they are down, and once they lift, goes on or goes back by
//! one rule for all, so that no gesture feels unlike another: on, if the
//! fingers took it more than half way, however slowly, or flicked it on,
//! however short a way, but for a twitch; back, otherwise, or if they were
//! flicking it back, which is how a gesture is taken back half way.
//!
//! How far the fingers go for the whole way is each gesture's own, as is
//! how long what they let go of takes to settle, which `docs/DESIGN.md`
//! gives each kind of movement. Speeds here are in shares of the whole way.

/// How fast the fingers going on when they lift is a flick, in shares of
/// the whole way a millisecond: a whole way in about two thirds of a
/// second.
const FLICK: f64 = 0.0015;

/// How much of the way a flick has to have gone, so that the fingers
/// settling as they lift is not one.
const TWITCH: f64 = 0.1;

/// How much of the speed the fingers were going at before each movement
/// is kept, beside that of the movement itself: the last few count, so
/// that the lighter touch of lifting does not hide a flick.
const KEPT: f64 = 0.5;

/// How fast fingers have been going lately.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fingers {
    /// In shares of the whole way a millisecond, on being more.
    speed: f64,
    /// When they last moved, in milliseconds.
    last: u32,
}

impl Fingers {
    /// Fingers that came down at `time`, in milliseconds, and have not
    /// moved.
    pub(crate) const fn down(time: u32) -> Self {
        Self {
            speed: 0.0,
            last: time,
        }
    }

    /// Notes the fingers going `by` a share of the whole way at `time`.
    /// Another movement told at the same time adds nothing to how fast.
    pub(crate) fn went(&mut self, by: f64, time: u32) {
        let gone = f64::from(time.wrapping_sub(self.last));
        if gone > 0.0 {
            self.speed = self.speed.mul_add(KEPT, (1.0 - KEPT) * by / gone);
            self.last = time;
        }
    }

    /// Whether what they let go of `reached` of the way on goes on the
    /// rest of it, or back the way it came: `onwards` if their speed being
    /// more is their going on, as it is not where the way on is the way
    /// they were noted going less.
    pub(crate) fn go_on(&self, reached: f64, onwards: bool) -> bool {
        let speed = if onwards { self.speed } else { -self.speed };
        if speed.abs() >= FLICK && reached >= TWITCH {
            speed > 0.0
        } else {
            reached > 0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fingers that went `by` in each of `steps` moves ten milliseconds
    /// apart.
    fn going(by: f64, steps: u32) -> Fingers {
        let mut fingers = Fingers::down(0);
        for step in 1..=steps {
            fingers.went(by, step * 10);
        }
        fingers
    }

    #[test]
    fn slow_fingers_carry_it_on_only_past_half_way() {
        let slow = going(0.001, 3);
        assert!(!slow.go_on(0.4, true));
        assert!(slow.go_on(0.6, true));
        // Going back, as slowly, from past half way.
        assert!(slow.go_on(0.6, false));
    }

    #[test]
    fn a_flick_carries_it_on_or_back_however_far_but_for_a_twitch() {
        let flick = going(0.05, 3);
        assert!(flick.go_on(0.15, true), "on, a short way");
        assert!(!flick.go_on(0.9, false), "back, from nearly there");
        assert!(!flick.go_on(0.05, true), "a twitch is not one");
    }

    #[test]
    fn the_lighter_touch_of_lifting_does_not_hide_a_flick() {
        let mut fingers = going(0.05, 3);
        fingers.went(0.0, 40);
        assert!(fingers.go_on(0.15, true));
        // And a move told at the same time as the last adds nothing.
        fingers.went(-5.0, 40);
        assert!(fingers.go_on(0.15, true));
    }
}
