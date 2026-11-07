//! A background that changes colour across a part, as `linear-gradient()`
//! gives it.
//!
//! It runs along a line through the middle of the part, which way its
//! direction says, from where the line meets the part at one corner to
//! where it meets it at the other, as CSS has it: a gradient from red to
//! blue to the right is red at the left edge and blue at the right
//! whatever the part's shape. Between two stops its colour is part of
//! each, mixed as a colour on its way from one to the other is.

use crate::Color;

/// Which way a gradient runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// At an angle, in degrees clockwise from up, from 0 to 359: `90deg`
    /// runs to the right, as `to right` does.
    Angle(u16),
    /// Towards a corner, `to top right` and the like: at whatever angle
    /// has the other two corners the same colour, which depends on the
    /// part's shape.
    Corner {
        /// Towards a right corner, and not a left one.
        right: bool,
        /// Towards a bottom corner, and not a top one.
        down: bool,
    },
}

/// A colour at a place along a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stop {
    /// The colour there.
    pub color: Color,
    /// How far along, in thousandths of the way.
    pub at: u16,
}

/// A background that changes colour across a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gradient {
    /// Which way it runs.
    pub direction: Direction,
    /// Its stops, as far as `count`, each no nearer the start than the one
    /// before.
    stops: [Stop; Self::MOST],
    count: u8,
}

impl Gradient {
    /// The most stops a gradient may have.
    pub const MOST: usize = 8;

    /// A gradient running `direction` through `stops`, which are put in
    /// order, each no nearer the start than the one before, as CSS does;
    /// none if there are fewer than two or more than [`Self::MOST`].
    #[must_use]
    pub fn new(direction: Direction, stops: &[Stop]) -> Option<Self> {
        let [first, _, ..] = stops else {
            return None;
        };
        let count = u8::try_from(stops.len()).ok();
        let count = count.filter(|count| usize::from(*count) <= Self::MOST)?;
        let mut kept = [*first; Self::MOST];
        let mut furthest = 0;
        for (into, stop) in kept.iter_mut().zip(stops) {
            furthest = stop.at.clamp(furthest, 1000);
            *into = Stop {
                at: furthest,
                ..*stop
            };
        }
        Some(Self {
            direction,
            stops: kept,
            count,
        })
    }

    /// Its stops, in order.
    #[must_use]
    pub fn stops(&self) -> &[Stop] {
        self.stops
            .get(..usize::from(self.count))
            .unwrap_or_default()
    }

    /// The same gradient with each of its colours made what `change`
    /// makes of it.
    #[must_use]
    pub fn recolored(mut self, change: impl Fn(Color) -> Color) -> Self {
        for stop in &mut self.stops {
            stop.color = change(stop.color);
        }
        self
    }

    /// Whether any of it can be seen through.
    #[must_use]
    pub fn see_through(&self) -> bool {
        self.stops().iter().any(|stop| stop.color.alpha < u8::MAX)
    }

    /// The colour `along` of the way, from 0 at its start to 1 at its end:
    /// the first stop's before it, the last's after it, and between two
    /// stops part of each.
    #[must_use]
    pub fn at(&self, along: f32) -> Color {
        let along = along.clamp(0.0, 1.0) * 1000.0;
        let stops = self.stops();
        let after = stops.iter().position(|stop| f32::from(stop.at) > along);
        let (Some(after), Some(first)) = (after, stops.first()) else {
            return stops
                .last()
                .map_or(Color::rgba(0, 0, 0, 0), |last| last.color);
        };
        let Some(before) = after.checked_sub(1).and_then(|before| stops.get(before)) else {
            return first.color;
        };
        let next = stops.get(after).unwrap_or(before);
        let (from, to) = (f32::from(before.at), f32::from(next.at));
        before
            .color
            .toward(next.color, (along - from) / (to - from))
    }

    /// How far along the gradient the point `(x, y)` of a part `width` by
    /// `height` is, from 0 at its start to 1 at its end; a point beyond
    /// either is before the start or past the end.
    #[must_use]
    pub fn along(&self, point: (f32, f32), size: (f32, f32)) -> f32 {
        self.run(size).along(point)
    }

    /// How it runs across a part `width` by `height`, to find how far
    /// along it each point is without working that out again.
    #[must_use]
    pub fn run(&self, (width, height): (f32, f32)) -> Run {
        let (across, down) = match self.direction {
            Direction::Angle(degrees) => {
                let (sine, cosine) = f32::from(degrees).to_radians().sin_cos();
                (sine, -cosine)
            }
            // Square to the line between the other two corners.
            Direction::Corner { right, down } => {
                let sign = |positive: bool| if positive { 1.0 } else { -1.0 };
                let length = width.hypot(height).max(f32::EPSILON);
                (sign(right) * height / length, sign(down) * width / length)
            }
        };
        let length = (width * across).abs() + (height * down).abs();
        Run {
            middle: (width / 2.0, height / 2.0),
            across: across / length.max(f32::EPSILON),
            down: down / length.max(f32::EPSILON),
        }
    }
}

/// How a gradient runs across a part of one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    middle: (f32, f32),
    /// How far along it each step across is, and each step down.
    across: f32,
    down: f32,
}

impl Run {
    /// How far along the gradient the point `(x, y)` is, from 0 at its
    /// start to 1 at its end.
    #[must_use]
    pub fn along(&self, (x, y): (f32, f32)) -> f32 {
        let (from_x, from_y) = (x - self.middle.0, y - self.middle.1);
        from_x.mul_add(self.across, from_y * self.down) + 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);
    const GREEN: Color = Color::rgb(0, 255, 0);

    fn stop(color: Color, at: u16) -> Stop {
        Stop { color, at }
    }

    fn to_right(stops: &[Stop]) -> Gradient {
        Gradient::new(Direction::Angle(90), stops).unwrap()
    }

    #[test]
    fn a_gradient_is_each_stops_colour_at_it_and_part_of_each_between() {
        let gradient = to_right(&[stop(RED, 0), stop(BLUE, 500), stop(GREEN, 1000)]);
        assert_eq!(gradient.at(0.0), RED);
        assert_eq!(gradient.at(0.25), Color::rgb(128, 0, 128));
        assert_eq!(gradient.at(0.5), BLUE);
        assert_eq!(gradient.at(1.0), GREEN);
        assert_eq!(gradient.at(-1.0), RED, "before the start");
        assert_eq!(gradient.at(2.0), GREEN, "past the end");
        // Stops short of either end are as far as the gradient goes.
        let inner = to_right(&[stop(RED, 200), stop(BLUE, 800)]);
        assert_eq!(inner.at(0.1), RED);
        assert_eq!(inner.at(0.9), BLUE);
        assert_eq!(inner.at(0.5), Color::rgb(128, 0, 128));
    }

    #[test]
    fn a_stop_nearer_the_start_than_the_one_before_is_where_that_one_is() {
        let gradient = to_right(&[stop(RED, 600), stop(BLUE, 300), stop(GREEN, 1200)]);
        let at: Vec<u16> = gradient.stops().iter().map(|stop| stop.at).collect();
        assert_eq!(at, [600, 600, 1000]);
        // Where two stops meet, it turns from one colour to the other at once.
        assert_eq!(gradient.at(0.59), RED);
        assert_eq!(gradient.at(0.61), Color::rgb(0, 6, 249));
    }

    #[test]
    fn a_gradient_has_from_two_to_eight_stops() {
        let stops = [stop(RED, 0); Gradient::MOST + 1];
        assert!(Gradient::new(Direction::Angle(0), &stops[..1]).is_none());
        assert!(Gradient::new(Direction::Angle(0), &stops[..2]).is_some());
        assert!(Gradient::new(Direction::Angle(0), &stops[..Gradient::MOST]).is_some());
        assert!(Gradient::new(Direction::Angle(0), &stops).is_none());
    }

    #[test]
    fn a_gradient_runs_from_one_side_of_a_part_to_the_other_whichever_way_it_goes() {
        let size = (200.0, 100.0);
        let along = |direction, point| {
            let gradient = Gradient::new(direction, &[stop(RED, 0), stop(BLUE, 1000)]);
            gradient.unwrap().along(point, size)
        };
        let near = |value: f32, expected: f32| (value - expected).abs() < 0.001;
        // To the right: the left edge is its start and the right its end.
        assert!(near(along(Direction::Angle(90), (0.0, 50.0)), 0.0));
        assert!(near(along(Direction::Angle(90), (200.0, 0.0)), 1.0));
        // Up, from the bottom.
        assert!(near(along(Direction::Angle(0), (100.0, 100.0)), 0.0));
        assert!(near(along(Direction::Angle(0), (0.0, 0.0)), 1.0));
        assert!(near(along(Direction::Angle(180), (0.0, 0.0)), 0.0), "down");
        // At an angle, from one corner to the other.
        assert!(near(along(Direction::Angle(135), (0.0, 0.0)), 0.0));
        assert!(near(along(Direction::Angle(135), (200.0, 100.0)), 1.0));
        // To a corner, from the other, with the two between of one colour.
        let corner = Direction::Corner {
            right: true,
            down: false,
        };
        assert!(near(along(corner, (0.0, 100.0)), 0.0));
        assert!(near(along(corner, (200.0, 0.0)), 1.0));
        assert!(near(along(corner, (0.0, 0.0)), 0.5));
        assert!(near(along(corner, (200.0, 100.0)), 0.5));
        let down_left = Direction::Corner {
            right: false,
            down: true,
        };
        assert!(near(along(down_left, (200.0, 0.0)), 0.0));
        assert!(near(along(down_left, (0.0, 100.0)), 1.0));
    }

    #[test]
    fn a_gradient_can_be_seen_through_where_a_stop_can() {
        let clear = Color::rgba(0, 0, 0, 0);
        assert!(!to_right(&[stop(RED, 0), stop(BLUE, 1000)]).see_through());
        assert!(to_right(&[stop(RED, 0), stop(clear, 1000)]).see_through());
    }
}
