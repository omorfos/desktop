//! How a part gets from one look to another when its state changes.
//!
//! Only colours change over time. Sizes do not, so that nothing moves
//! under the pointer while it is on its way to somewhere.

use std::time::Duration;

use crate::value::byte;
use crate::{Color, Edges, Line, Shadow, Style};

/// How a change gathers and loses speed on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// At the same speed throughout.
    Linear,
    /// Quick to start and slow to finish, which is what CSS does when it
    /// is told nothing.
    #[default]
    Ease,
    /// Slow to start.
    EaseIn,
    /// Slow to finish.
    EaseOut,
    /// Slow to start and to finish.
    EaseInOut,
    /// Bent by two points a theme gives, as `cubic-bezier()` does, each
    /// in thousandths: how far along its time, from 0 to 1000, and how far
    /// along the change, which may go before its start or past its end.
    CubicBezier {
        /// How far along its time the first point is.
        x1: u16,
        /// How far along the change the first point is.
        y1: i16,
        /// How far along its time the second point is.
        x2: u16,
        /// How far along the change the second point is.
        y2: i16,
    },
}

impl Easing {
    /// Every way there is by a name of its own.
    pub const ALL: [Self; 5] = [
        Self::Linear,
        Self::Ease,
        Self::EaseIn,
        Self::EaseOut,
        Self::EaseInOut,
    ];

    /// What it is called in a stylesheet.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Ease => "ease",
            Self::EaseIn => "ease-in",
            Self::EaseOut => "ease-out",
            Self::EaseInOut => "ease-in-out",
            Self::CubicBezier { .. } => "cubic-bezier",
        }
    }

    /// The two points that bend the curve, as CSS defines them.
    fn curve(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
            Self::Ease => (0.25, 0.1, 0.25, 1.0),
            Self::EaseIn => (0.42, 0.0, 1.0, 1.0),
            Self::EaseOut => (0.0, 0.0, 0.58, 1.0),
            Self::EaseInOut => (0.42, 0.0, 0.58, 1.0),
            Self::CubicBezier { x1, y1, x2, y2 } => {
                let point = |thousandths: f32| thousandths / 1000.0;
                (
                    point(f32::from(x1)),
                    point(f32::from(y1)),
                    point(f32::from(x2)),
                    point(f32::from(y2)),
                )
            }
        }
    }

    /// How far the change has got when `share` of its time has gone by,
    /// both from 0 to 1.
    #[must_use]
    pub fn at(self, share: f32) -> f32 {
        let (x1, y1, x2, y2) = self.curve();
        // A point on the curve between 0 and 1, bent by two others.
        let along = |first: f32, second: f32, t: f32| {
            let rest = 1.0 - t;
            (3.0 * rest * rest * t).mul_add(first, (3.0 * rest * t * t).mul_add(second, t * t * t))
        };
        // Time only goes forwards along the curve, so where it has got to
        // is found by halving.
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        for _ in 0..16 {
            let middle = f32::midpoint(low, high);
            if along(x1, x2, middle) < share {
                low = middle;
            } else {
                high = middle;
            }
        }
        along(y1, y2, f32::midpoint(low, high)).clamp(0.0, 1.0)
    }
}

/// How a part's colours change when the state it is in does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Transition {
    /// How long the change takes, in milliseconds. None is at once.
    pub duration: u32,
    /// How long it waits before it begins, in milliseconds.
    pub delay: u32,
    /// How it gathers and loses speed.
    pub easing: Easing,
}

impl Transition {
    /// A change that happens at once.
    pub const NONE: Self = Self {
        duration: 0,
        delay: 0,
        easing: Easing::Ease,
    };

    /// The longest a change may take, or wait, in milliseconds.
    pub const LONGEST: u32 = 10_000;

    /// How long until the change is done: the wait and the change.
    #[must_use]
    pub const fn lasts(&self) -> Duration {
        Duration::from_millis(self.delay as u64 + self.duration as u64)
    }

    /// How far along the change is once `elapsed` has gone by since it
    /// was asked for: from 0, not begun, which it is while it waits, to 1,
    /// done.
    #[must_use]
    pub fn progress(&self, elapsed: Duration) -> f32 {
        if elapsed >= self.lasts() {
            return 1.0;
        }
        let delay = Duration::from_millis(u64::from(self.delay));
        let Some(changing) = elapsed.checked_sub(delay) else {
            return 0.0;
        };
        // Not done, so the change takes some time.
        let takes = Duration::from_millis(u64::from(self.duration));
        self.easing.at(changing.as_secs_f32() / takes.as_secs_f32())
    }
}

impl Color {
    /// The colour `share` of the way from this one to `other`, where 0 is
    /// this one and 1 the other.
    ///
    /// A colour that is on its way to or from being see-through keeps its
    /// hue: it is the light it gives that is mixed, not its numbers.
    #[must_use]
    pub fn toward(self, other: Self, share: f32) -> Self {
        let share = share.clamp(0.0, 1.0);
        let mix = |from: f32, to: f32| (to - from).mul_add(share, from);
        let (from, to) = (f32::from(self.alpha), f32::from(other.alpha));
        let alpha = mix(from, to);
        let channel = |one: u8, two: u8| {
            let light = mix(f32::from(one) * from, f32::from(two) * to);
            // What is wholly see-through gives no light and has no colour.
            byte(light / alpha.max(1.0))
        };
        Self::rgba(
            channel(self.red, other.red),
            channel(self.green, other.green),
            channel(self.blue, other.blue),
            byte(alpha),
        )
    }
}

impl Style {
    /// This look with its colours `share` of the way from those of `from`
    /// to its own. Everything else is its own from the start.
    #[must_use]
    pub fn blended(&self, from: &Self, share: f32) -> Self {
        let line = |old: Line, new: Line| Line {
            color: old.color.toward(new.color, share),
            width: new.width,
        };
        Self {
            background: from.background.toward(self.background, share),
            color: from.color.toward(self.color, share),
            border: Edges {
                top: line(from.border.top, self.border.top),
                right: line(from.border.right, self.border.right),
                bottom: line(from.border.bottom, self.border.bottom),
                left: line(from.border.left, self.border.left),
            },
            shadow: self.shadow.blended(&from.shadow, share),
            ..*self
        }
    }
}

impl Shadow {
    /// This shadow, `share` of the way here from `from`. One that comes or
    /// goes keeps its place and only fades; between two, everything of it
    /// is between them.
    #[must_use]
    pub fn blended(&self, from: &Self, share: f32) -> Self {
        let share = share.clamp(0.0, 1.0);
        let color = from.color.toward(self.color, share);
        let placed = match (from.is_none(), self.is_none()) {
            (true, _) => *self,
            (false, true) => *from,
            (false, false) => {
                let between = |from: i32, to: i32| {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "the sizes of a shadow are small, and what is between them too"
                    )]
                    let between = (to - from) as f32 * share + from as f32;
                    #[expect(clippy::cast_possible_truncation, reason = "between two small sizes")]
                    let between = between.round() as i32;
                    between
                };
                let blur = |blur: u32| i32::try_from(blur).unwrap_or(i32::MAX);
                Self {
                    x: between(from.x, self.x),
                    y: between(from.y, self.y),
                    blur: between(blur(from.blur), blur(self.blur)).unsigned_abs(),
                    spread: between(from.spread, self.spread),
                    color,
                }
            }
        };
        Self { color, ..placed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Shadow;
    use crate::style::CLEAR;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);

    #[test]
    fn every_way_of_changing_starts_at_nothing_and_ends_with_all_of_it() {
        for easing in Easing::ALL {
            let name = easing.name();
            assert!(easing.at(0.0) < 0.01, "{name}");
            assert!(easing.at(1.0) > 0.99, "{name}");
            let mut last = 0.0;
            for step in 0..=20_u8 {
                let now = easing.at(f32::from(step) / 20.0);
                assert!(now >= last, "{name} never goes back");
                last = now;
            }
        }
    }

    #[test]
    fn each_way_of_changing_is_quick_and_slow_where_its_name_says() {
        let half = |easing: Easing| easing.at(0.5);
        assert!((half(Easing::Linear) - 0.5).abs() < 0.01);
        assert!(half(Easing::EaseIn) < 0.4, "slow to start");
        assert!(half(Easing::EaseOut) > 0.6, "slow to finish");
        assert!((half(Easing::EaseInOut) - 0.5).abs() < 0.01);
        assert!(half(Easing::Ease) > 0.7, "quick to start");
        assert_eq!(Easing::default(), Easing::Ease);
        let names: Vec<&str> = Easing::ALL.iter().map(|easing| easing.name()).collect();
        assert_eq!(
            names,
            ["linear", "ease", "ease-in", "ease-out", "ease-in-out"]
        );
    }

    #[test]
    fn a_change_is_done_when_its_time_is_up_and_at_once_if_it_takes_none() {
        let linear = Transition {
            duration: 200,
            delay: 0,
            easing: Easing::Linear,
        };
        let after = |millis| linear.progress(Duration::from_millis(millis));
        assert!(after(0) < 0.01);
        assert!((after(100) - 0.5).abs() < 0.01);
        assert!((after(200) - 1.0).abs() < f32::EPSILON);
        assert!((after(5000) - 1.0).abs() < f32::EPSILON);
        assert_eq!(linear.lasts(), Duration::from_millis(200));

        assert_eq!(Transition::default(), Transition::NONE);
        let at_once = Transition::NONE.progress(Duration::ZERO);
        assert!((at_once - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_change_that_waits_does_not_begin_until_it_has_and_is_done_after_both() {
        let waiting = Transition {
            duration: 200,
            delay: 100,
            easing: Easing::Linear,
        };
        let after = |millis| waiting.progress(Duration::from_millis(millis));
        assert!(after(0) < f32::EPSILON);
        assert!(after(100) < 0.01, "waited");
        assert!((after(200) - 0.5).abs() < 0.01, "half way");
        assert!((after(300) - 1.0).abs() < f32::EPSILON);
        assert_eq!(waiting.lasts(), Duration::from_millis(300), "both");
        // One that only waits changes all at once when it is done.
        let then = Transition {
            duration: 0,
            ..waiting
        };
        assert!(then.progress(Duration::from_millis(99)) < f32::EPSILON);
        assert!((then.progress(Duration::from_millis(100)) - 1.0).abs() < f32::EPSILON);
        assert_eq!(then.lasts(), Duration::from_millis(100));
    }

    #[test]
    fn a_curve_of_a_themes_own_is_bent_by_the_points_it_gives() {
        let curve = |x1, y1, x2, y2| Easing::CubicBezier { x1, y1, x2, y2 };
        // The one CSS calls `ease`, given by its points.
        let ease = curve(250, 100, 250, 1000);
        for step in 0..=10_u8 {
            let share = f32::from(step) / 10.0;
            assert!((ease.at(share) - Easing::Ease.at(share)).abs() < 0.001);
        }
        assert!(
            (curve(0, 0, 1000, 1000).at(0.3) - 0.3).abs() < 0.01,
            "straight"
        );
        // One that goes past its end and comes back goes no further than it.
        let past = curve(300, 1500, 700, 1500);
        assert!((past.at(0.5) - 1.0).abs() < f32::EPSILON);
        assert!(curve(300, -500, 700, -500).at(0.5) < f32::EPSILON);
        assert_eq!(ease.name(), "cubic-bezier");
    }

    #[test]
    fn a_colour_between_two_others_is_part_of_each() {
        assert_eq!(RED.toward(BLUE, 0.0), RED);
        assert_eq!(RED.toward(BLUE, 1.0), BLUE);
        assert_eq!(RED.toward(BLUE, 0.5), Color::rgb(128, 0, 128));
        assert_eq!(
            RED.toward(BLUE, -3.0),
            RED,
            "no further back than the start"
        );
        assert_eq!(RED.toward(BLUE, 7.0), BLUE, "and no further than the end");
    }

    #[test]
    fn a_colour_keeps_its_hue_as_it_comes_out_of_nothing() {
        assert_eq!(CLEAR.toward(RED, 0.5), Color::rgba(255, 0, 0, 128));
        assert_eq!(RED.toward(CLEAR, 0.5), Color::rgba(255, 0, 0, 128));
        assert_eq!(CLEAR.toward(CLEAR, 0.5), CLEAR);
    }

    #[test]
    fn a_look_on_its_way_from_another_has_its_own_sizes_and_colours_between() {
        let line = |color, width| Line { color, width };
        let old = Style {
            background: RED,
            color: RED,
            border: Edges::all(line(RED, 1)),
            radius: 2,
            ..Style::NONE
        };
        let new = Style {
            background: BLUE,
            color: BLUE,
            border: Edges::all(line(BLUE, 3)),
            radius: 8,
            ..Style::NONE
        };
        let purple = Color::rgb(128, 0, 128);
        let between = new.blended(&old, 0.5);
        assert_eq!((between.background, between.color), (purple, purple));
        assert_eq!(between.border, Edges::all(line(purple, 3)));
        assert_eq!(between.radius, 8);
        assert_eq!(new.blended(&old, 0.0).background, RED);
        assert_eq!(new.blended(&old, 1.0), new);
    }

    #[test]
    fn a_shadow_on_its_way_from_another_falls_and_softens_between_them() {
        let shadow = |y: i32, blur, spread, color| Shadow {
            x: -y,
            y,
            blur,
            spread,
            color,
        };
        let old = Style {
            shadow: shadow(4, 12, -2, RED),
            ..Style::NONE
        };
        let new = Style {
            shadow: shadow(8, 24, 2, BLUE),
            ..Style::NONE
        };
        let purple = Color::rgb(128, 0, 128);
        assert_eq!(new.blended(&old, 0.5).shadow, shadow(6, 18, 0, purple));
        assert_eq!(new.blended(&old, 0.0).shadow, old.shadow);
        // From none, it comes in where it is to be.
        let none = Style::NONE;
        let coming = new.blended(&none, 0.5).shadow;
        assert_eq!((coming.y, coming.blur), (8, 24));
        assert_eq!(coming.color.alpha, 128);
        // To none, it goes where it was.
        let going = none.blended(&new, 0.5).shadow;
        assert_eq!((going.y, going.blur), (8, 24));
        assert_eq!(going.color.alpha, 128);
    }
}
