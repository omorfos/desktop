//! How big things are drawn: the number of screen pixels to one unit of
//! layout.
//!
//! Windows are placed and sized in units that mean the same on any screen.
//! A [`Scale`] turns them into pixels. On a dense screen it is more than
//! one, so that text and controls stay a readable size.

/// The steps a scale is kept to: quarters, which every application that can
/// scale at all can match exactly.
const STEP: u32 = 30;

/// Millimetres to the inch.
const MILLIMETRES_TO_THE_INCH: f64 = 25.4;
/// Pixels to the inch a screen read from near is aimed at, as a laptop's own.
const NEAR: f64 = 135.0;
/// Pixels to the inch a screen on a desk is aimed at.
const ON_A_DESK: f64 = 110.0;
/// Under how many inches along the diagonal a screen is read from near.
const NEAR_UNDER: f64 = 20.0;
/// Over how many millimetres on its shorter side a screen is a television.
const A_TELEVISION_OVER: u32 = 560;
/// How near 1 or 2 a sum is taken for them.
const WHOLE_WITHIN: f64 = 0.15;
/// How far over 2 a sum is still taken for 2: further than below it, as
/// a dense laptop is sharper at 2 than at 2.5 and large enough, as one
/// of 14 inches and 3840 by 2160, at 2.34, is (ADR 100).
const TWO_WITHIN_OVER: f64 = 0.35;
/// The lines to a whole scale, where a screen's size is not gone by.
const LINES_TO_A_SCALE: u32 = 1080;
/// The units no scale leaves fewer of on a screen's shorter side.
const KEPT_BACK: u32 = 720;
/// The shapes some screens give where their size should be, in
/// millimetres: these say nothing of how large the screen is.
const SHAPES: [(u32, u32); 6] = [
    (16, 9),
    (16, 10),
    (160, 90),
    (160, 100),
    (1600, 900),
    (1600, 1000),
];

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

    /// The scale nearest to `numerator` units of 1/120, as the fractional
    /// scale protocol tells one, kept to quarters as [`Scale::new`] keeps
    /// a factor.
    #[must_use]
    pub fn from_numerator(numerator: u32) -> Self {
        Self::new(f64::from(numerator) / f64::from(Self::ONE.0))
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

    /// A scale that suits a screen `pixels` across and down that says it
    /// is `millimetres` wide and tall, by how dense it is and how near it is
    /// read (ADR 100). One under 20 inches along the diagonal is read from
    /// near, as a laptop's own is, and aimed at 135 pixels to the inch; a
    /// larger one stands on a desk, and is aimed at 110. A television, and
    /// a screen that says no size or one that cannot be its own, go by
    /// their lines alone. No scale leaves fewer than 720 units on the
    /// shorter side.
    #[must_use]
    pub fn for_screen(pixels: (u32, u32), millimetres: (u32, u32)) -> Self {
        let lines = pixels.0.min(pixels.1);
        let (wide, tall) = millimetres;
        let says_nothing_true = wide == 0
            || tall == 0
            || SHAPES.contains(&(wide, tall))
            || SHAPES.contains(&(tall, wide));
        let guessed = if says_nothing_true || wide.min(tall) > A_TELEVISION_OVER {
            Self((lines / LINES_TO_A_SCALE).clamp(1, 4) * Self::ONE.0)
        } else {
            let inches = f64::from(wide).hypot(f64::from(tall)) / MILLIMETRES_TO_THE_INCH;
            let density = f64::from(pixels.0).hypot(f64::from(pixels.1)) / inches;
            let aim = if inches < NEAR_UNDER { NEAR } else { ON_A_DESK };
            Self::rounded(density / aim)
        };
        guessed.kept_back(lines)
    }

    /// The scale a sum of how dense a screen is, over what it is aimed at,
    /// comes to: a quarter up to 2 and a half beyond, and 1 or 2 where it
    /// is within 0.15 of them, or up to 0.35 over 2, those being the
    /// sharpest.
    fn rounded(sum: f64) -> Self {
        if (sum - 1.0).abs() <= WHOLE_WITHIN {
            Self::ONE
        } else if (2.0 - WHOLE_WITHIN..=2.0 + TWO_WITHIN_OVER).contains(&sum) {
            Self::new(2.0)
        } else if sum > 2.0 {
            Self::new((sum * 2.0).round() / 2.0)
        } else {
            Self::new(sum)
        }
    }

    /// This scale, or a smaller one where it would leave fewer than 720
    /// units on a screen of `lines` on its shorter side.
    fn kept_back(self, lines: u32) -> Self {
        let mut steps = self.0 / STEP;
        // In 64 bits, as a screen may say it has any number of lines.
        let quarters = u64::from(lines) * u64::from(Self::ONE.0 / STEP);
        while steps > Self::ONE.0 / STEP && quarters < u64::from(KEPT_BACK * steps) {
            steps -= 1;
        }
        Self(steps * STEP)
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
    fn a_scale_told_in_parts_of_120_is_the_scale_of_that_factor() {
        assert_eq!(Scale::from_numerator(180), Scale::new(1.5));
        assert_eq!(
            Scale::from_numerator(140),
            Scale::new(1.25),
            "to the nearest quarter"
        );
        assert_eq!(Scale::from_numerator(0), Scale::ONE);
        assert_eq!(Scale::from_numerator(u32::MAX), Scale::MAX);
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

    /// A screen of a diagonal in inches, which says the size that makes it
    /// to the nearest millimetre each way, as ADR 100's tables have them.
    fn screen(inches: f64, across: u32, down: u32) -> Scale {
        let diagonal = f64::from(across).hypot(f64::from(down));
        let side = |pixels: u32| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a few hundred millimetres"
            )]
            let millimetres = (inches * 25.4 * f64::from(pixels) / diagonal).round() as u32;
            millimetres
        };
        Scale::for_screen((across, down), (side(across), side(down)))
    }

    #[track_caller]
    fn gives(scale: Scale, factor: f64) {
        assert_eq!(scale, Scale::new(factor));
    }

    #[test]
    fn a_screen_under_20_inches_is_aimed_at_135_to_the_inch() {
        gives(screen(13.3, 1920, 1080), 1.25);
        gives(screen(14.0, 1920, 1080), 1.25);
        gives(screen(14.0, 1920, 1200), 1.25);
        gives(screen(15.6, 1920, 1080), 1.0);
        gives(screen(16.0, 1920, 1200), 1.0);
        gives(screen(17.3, 1920, 1080), 1.0);
        gives(screen(13.5, 2256, 1504), 1.5);
        gives(screen(13.3, 2560, 1600), 1.75);
        gives(screen(14.0, 2560, 1600), 1.5);
        gives(screen(16.0, 2560, 1600), 1.5);
        gives(screen(14.0, 2880, 1800), 1.75);
        gives(screen(16.0, 3200, 2000), 1.75);
        gives(screen(13.4, 3840, 2400), 2.5);
        gives(screen(14.0, 3840, 2400), 2.5);
        gives(screen(15.6, 3840, 2160), 2.0);
        gives(screen(16.0, 3840, 2400), 2.0);
        gives(screen(17.3, 3840, 2160), 2.0);
    }

    #[test]
    fn the_maintainers_laptop_is_given_2_at_the_size_its_panel_says() {
        // A BOE NV140QUM-N53, 14 inches, 3840 by 2160: 316 to the inch,
        // which over what is aimed at is 2.34.
        gives(Scale::for_screen((3840, 2160), (309, 174)), 2.0);
    }

    #[test]
    fn the_maintainers_laptop_is_given_2_whatever_width_its_panel_says() {
        for width in 309..=390 {
            let height = (width * 9 + 8) / 16;
            gives(Scale::for_screen((3840, 2160), (width, height)), 2.0);
        }
        gives(Scale::for_screen((3840, 2160), (340, 190)), 2.0);
    }

    #[test]
    fn a_screen_is_read_from_near_under_20_inches_and_from_a_desk_from_there() {
        // As dense, the one read from near has things drawn smaller.
        gives(screen(19.9, 2560, 1440), 1.0);
        gives(screen(20.1, 2560, 1440), 1.25);
    }

    #[test]
    fn a_screen_of_20_inches_or_more_is_aimed_at_110_to_the_inch() {
        // Either side of the line at 1.15.
        gives(screen(23.4, 2560, 1440), 1.0);
        gives(screen(23.0, 2560, 1440), 1.25);
        gives(screen(21.5, 1920, 1080), 1.0);
        gives(screen(23.8, 1920, 1080), 1.0);
        gives(screen(27.0, 1920, 1080), 1.0);
        gives(screen(23.8, 2560, 1440), 1.0);
        gives(screen(27.0, 2560, 1440), 1.0);
        gives(screen(31.5, 2560, 1440), 1.0);
        gives(screen(34.0, 3440, 1440), 1.0);
        gives(screen(49.0, 5120, 1440), 1.0);
        gives(screen(23.8, 3840, 2160), 1.75);
        gives(screen(27.0, 3840, 2160), 1.5);
        gives(screen(31.5, 3840, 2160), 1.25);
        gives(screen(42.5, 3840, 2160), 1.0);
        gives(screen(27.0, 5120, 2880), 2.0);
        gives(screen(31.5, 6016, 3384), 2.0);
    }

    #[test]
    fn a_screen_over_560_millimetres_on_its_shorter_side_is_a_television_scaled_by_its_lines() {
        // Either side of the line, and upright.
        gives(Scale::for_screen((2160, 3840), (560, 996)), 1.0);
        gives(Scale::for_screen((3840, 2160), (998, 561)), 2.0);
        gives(screen(32.0, 1920, 1080), 1.0);
        gives(screen(42.0, 3840, 2160), 1.0);
        gives(screen(43.0, 1920, 1080), 1.0);
        gives(screen(43.0, 3840, 2160), 1.0);
        gives(screen(48.0, 3840, 2160), 2.0);
        gives(screen(55.0, 3840, 2160), 2.0);
        gives(screen(65.0, 3840, 2160), 2.0);
        gives(screen(77.0, 3840, 2160), 2.0);
        gives(screen(85.0, 7680, 4320), 4.0);
    }

    #[test]
    fn a_screen_that_says_no_size_or_a_shape_is_scaled_by_its_lines() {
        // 2000 lines, short of 2160, is 1, whichever shape it says, either
        // way up.
        for (wide, tall) in [
            (0, 0),
            (0, 194),
            (16, 9),
            (16, 10),
            (160, 90),
            (160, 100),
            (1600, 900),
            (1600, 1000),
        ] {
            gives(Scale::for_screen((3200, 2000), (wide, tall)), 1.0);
            gives(Scale::for_screen((2000, 3200), (tall, wide)), 1.0);
        }
        gives(Scale::for_screen((u32::MAX, u32::MAX), (0, 0)), 4.0);
        gives(Scale::for_screen((3840, 2160), (1600, 900)), 2.0);
        gives(Scale::for_screen((2160, 3840), (900, 1600)), 2.0);
        gives(Scale::for_screen((3840, 2160), (0, 0)), 2.0);
        gives(Scale::for_screen((3840, 2160), (344, 0)), 2.0);
        gives(Scale::for_screen((1920, 1080), (0, 0)), 1.0);
        gives(Scale::for_screen((1920, 1200), (0, 0)), 1.0);
        gives(Scale::for_screen((2560, 1440), (16, 9)), 1.0);
        gives(Scale::for_screen((3840, 2400), (160, 100)), 2.0);
    }

    #[test]
    fn a_sum_is_rounded_to_a_quarter_up_to_2_and_a_half_beyond_and_to_1_or_2_near_them() {
        for (sum, factor) in [
            (0.5, 1.0),
            (1.14, 1.0),
            (1.16, 1.25),
            (1.37, 1.25),
            (1.38, 1.5),
            (1.62, 1.5),
            (1.63, 1.75),
            (1.84, 1.75),
            (1.86, 2.0),
            (2.24, 2.0),
            (2.35, 2.0),
            (2.36, 2.5),
            (2.74, 2.5),
            (2.76, 3.0),
            (3.24, 3.0),
            (3.26, 3.5),
            (3.74, 3.5),
            (3.76, 4.0),
            (9.0, 4.0),
        ] {
            assert_eq!((sum, Scale::rounded(sum)), (sum, Scale::new(factor)));
        }
    }

    #[test]
    fn no_guess_leaves_fewer_than_720_units_on_the_shorter_side() {
        // Kept back a quarter at a time, and never under 1.
        gives(screen(7.0, 1680, 1050), 1.25);
        gives(screen(7.0, 800, 480), 1.0);
        gives(screen(7.0, 1280, 800), 1.0);
        gives(screen(6.8, 1920, 1080), 1.5);
        gives(Scale::for_screen((3840, 2160), (10, 10)), 3.0);
        gives(Scale::for_screen((1280, 720), (0, 0)), 1.0);
    }
}
