//! How big things are drawn: the number of screen pixels to one unit of
//! layout.
//!
//! Windows are placed and sized in units that mean the same on any screen.
//! A [`Scale`] turns them into pixels. On a dense screen it is more than
//! one, so that text and controls stay a readable size.

/// The steps a scale is kept to: quarters, which every application that can
/// scale at all can match exactly.
const STEP: u32 = 30;

/// How many pixels there are to a unit of layout.
///
/// Kept as a whole number of units of 1/120, which the fractional-scale protocol
/// tells applications in, so that the compositor and an application
/// rounding the same length always agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scale(u32);

impl Scale {
    /// One pixel to a unit.
    pub const ONE: Self = Self(120);
    /// The most a scale can be.
    pub const MAX: Self = Self(480);

    /// The scale nearest to `factor`: a multiple of a quarter, at least one
    /// and at most four. Anything that is not a number counts as one.
    #[must_use]
    pub fn new(factor: f64) -> Self {
        let steps = (factor * f64::from(Self::ONE.0 / STEP)).round();
        // `max` then `min`, not `clamp`, because these turn something that
        // is not a number into a bound and `clamp` would pass it through.
        #[expect(
            clippy::manual_clamp,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is limited to a small positive range first"
        )]
        let steps = steps.max(4.0).min(16.0) as u32;
        Self(steps * STEP)
    }

    /// The scale in units of 1/120.
    #[must_use]
    pub const fn numerator(self) -> u32 {
        self.0
    }

    /// The scale as a factor, such as `1.5`.
    #[must_use]
    pub fn factor(self) -> f64 {
        f64::from(self.0) / f64::from(Self::ONE.0)
    }

    /// The whole number of pixels to a unit that an application which
    /// cannot draw at a fraction should use: the next one up, so that it is
    /// shrunk a little, not stretched.
    #[must_use]
    pub fn whole(self) -> i32 {
        i32::try_from(self.0.div_ceil(Self::ONE.0)).unwrap_or(1)
    }

    /// The pixel a position in units falls on, to the nearest.
    #[must_use]
    pub fn pixels(self, units: i32) -> i32 {
        let scaled = i64::from(units) * i64::from(self.0);
        let one = i64::from(Self::ONE.0);
        i32::try_from((scaled + one / 2).div_euclid(one)).unwrap_or(i32::MAX)
    }

    /// The length in units that a number of pixels comes to, to the
    /// nearest.
    #[must_use]
    pub fn units(self, pixels: i32) -> i32 {
        let scaled = i64::from(pixels) * i64::from(Self::ONE.0);
        let scale = i64::from(self.0);
        i32::try_from((scaled + scale / 2).div_euclid(scale)).unwrap_or(i32::MAX)
    }

    /// A scale that suits a screen of `pixels` across that is `millimetres`
    /// wide, so that things come out about the size they are on an ordinary
    /// desktop monitor. A screen that does not say how wide it is gets one.
    #[must_use]
    pub fn for_screen(pixels: u32, millimetres: u32) -> Self {
        if millimetres == 0 {
            return Self::ONE;
        }
        // Dots per inch, against the 120 at which a scale of one is still
        // comfortable. Most desktop monitors are between 90 and 110.
        let density = f64::from(pixels) * 25.4 / f64::from(millimetres);
        Self::new(density / 120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_is_kept_to_quarters_between_one_and_four() {
        assert_eq!(Scale::new(1.0), Scale::ONE);
        assert_eq!(Scale::new(1.5).numerator(), 180);
        assert_eq!(Scale::new(1.6).numerator(), 180, "to the nearest quarter");
        assert_eq!(Scale::new(1.7).numerator(), 210);
        assert_eq!(Scale::new(0.2), Scale::ONE, "never less than one");
        assert_eq!(Scale::new(-3.0), Scale::ONE);
        assert_eq!(Scale::new(9.0), Scale::MAX, "never more than four");
        assert_eq!(Scale::new(f64::NAN), Scale::ONE, "not a number");
        assert_eq!(Scale::new(f64::INFINITY), Scale::MAX);
    }

    #[test]
    fn a_scale_reads_back_as_a_factor() {
        assert!((Scale::new(1.25).factor() - 1.25).abs() < f64::EPSILON);
        assert!((Scale::ONE.factor() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn an_application_that_cannot_draw_at_a_fraction_is_given_the_next_whole_scale() {
        assert_eq!(Scale::ONE.whole(), 1);
        assert_eq!(Scale::new(1.25).whole(), 2);
        assert_eq!(Scale::new(2.0).whole(), 2);
        assert_eq!(Scale::new(2.75).whole(), 3);
    }

    #[test]
    fn units_become_pixels_to_the_nearest_and_back() {
        let half_again = Scale::new(1.5);
        assert_eq!(half_again.pixels(10), 15);
        assert_eq!(half_again.pixels(1), 2, "a half rounds up");
        assert_eq!(half_again.pixels(-10), -15);
        assert_eq!(half_again.pixels(0), 0);
        assert_eq!(half_again.units(15), 10);
        assert_eq!(half_again.units(2880), 1920);
        assert_eq!(Scale::ONE.pixels(37), 37);
        assert_eq!(Scale::ONE.units(37), 37);
        assert_eq!(Scale::new(2.0).units(5), 3, "a half rounds up");
    }

    #[test]
    fn a_dense_screen_is_given_a_larger_scale() {
        assert_eq!(Scale::for_screen(1920, 527), Scale::ONE, "24 inch, 1080p");
        assert_eq!(Scale::for_screen(2560, 597), Scale::ONE, "27 inch, 1440p");
        assert_eq!(
            Scale::for_screen(3840, 597),
            Scale::new(1.25),
            "27 inch, 4K"
        );
        assert_eq!(
            Scale::for_screen(1920, 302),
            Scale::new(1.25),
            "14 inch laptop, 1200p"
        );
        assert_eq!(
            Scale::for_screen(2880, 302),
            Scale::new(2.0),
            "14 inch laptop, 2880 across"
        );
        assert_eq!(
            Scale::for_screen(3840, 344),
            Scale::new(2.25),
            "16 inch laptop, 4K"
        );
    }

    #[test]
    fn a_screen_that_does_not_say_how_wide_it_is_is_not_scaled() {
        assert_eq!(Scale::for_screen(3840, 0), Scale::ONE);
    }
}
