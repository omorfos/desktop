//! How a part gets from one look to another when its state changes.
//!
//! Only colours change over time. Sizes do not, so that nothing moves
//! under the pointer while it is on its way to somewhere.

use std::time::Duration;

use crate::value::byte;
use crate::{Color, Edges, Line, Style};

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
}

impl Easing {
    /// Every way there is.
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
        }
    }

    /// The two points that bend the curve, as CSS defines them.
    const fn curve(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
            Self::Ease => (0.25, 0.1, 0.25, 1.0),
            Self::EaseIn => (0.42, 0.0, 1.0, 1.0),
            Self::EaseOut => (0.0, 0.0, 0.58, 1.0),
            Self::EaseInOut => (0.42, 0.0, 0.58, 1.0),
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
    /// How it gathers and loses speed.
    pub easing: Easing,
}

impl Transition {
    /// A change that happens at once.
    pub const NONE: Self = Self {
        duration: 0,
        easing: Easing::Ease,
    };

    /// The longest a change may take, in milliseconds.
    pub const LONGEST: u32 = 10_000;

    /// How long the change takes.
    #[must_use]
    pub const fn lasts(&self) -> Duration {
        Duration::from_millis(self.duration as u64)
    }

    /// How far along the change is once `elapsed` has gone by since it
    /// began: from 0, not begun, to 1, done.
    #[must_use]
    pub fn progress(&self, elapsed: Duration) -> f32 {
        let lasts = self.lasts();
        if elapsed >= lasts {
            return 1.0;
        }
        self.easing.at(elapsed.as_secs_f32() / lasts.as_secs_f32())
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
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
