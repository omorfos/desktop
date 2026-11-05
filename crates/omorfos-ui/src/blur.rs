//! The blur behind glass (`backdrop-filter`, ADR 33), made on the CPU: what
//! the tests draw, and the specification the shader that makes it on the
//! GPU is held to, tap for tap.
//!
//! It is a dual-filter blur, the kind known as dual Kawase. The picture is
//! made half as large, again and again, each pixel taking five samples of
//! the larger; then twice as large, back to its own size, each pixel taking
//! eight samples of the smaller. Most of the work is on small pictures, so
//! a wide blur costs little more than a narrow one. To the eye it is close
//! to a Gaussian, and how wide it is comes from how many times it is made
//! smaller and how far apart its samples are.
//!
//! Exactly, so that a shader can do the same:
//!
//! - Pixels are red, green, blue and alpha, premultiplied, a byte each;
//!   each pass is rounded to bytes, as one drawn into a texture is.
//! - A pass down makes a picture `w` by `h` half as large, rounded up, and
//!   at least a pixel; a pass up makes it the size it was before.
//! - Pixel `x, y` of what a pass makes samples around the point
//!   `((x + ½) · w / W, (y + ½) · h / H)` of the picture it is made from,
//!   which is `w` by `h`, where `W` by `H` is what is made; a pixel `i`
//!   covers `i` to `i + 1`.
//! - A sample is bilinear, between the four pixels whose middles are
//!   nearest; beyond an edge, the pixel at the edge is taken.
//! - Down: the point four times and the four points `offset` away
//!   diagonally, over eight.
//! - Up: the four points `2 · offset` away across and down, once each, and
//!   the four `offset` away diagonally, twice each, over twelve.
//! - The last pass up then gives the colour more saturation: each channel
//!   is moved away from the pixel's luminance by [`Blur::SATURATION`], and
//!   kept from nothing to the pixel's alpha.

/// How a blur is made: how many times the picture is halved, and how far
/// apart, in the pixels of the picture sampled, its samples are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blur {
    passes: u32,
    offset: f32,
    deviation: u32,
}

/// One pass of a blur, the picture it makes, as numbers from 0 to 255.
#[derive(Debug, Clone)]
struct Layer {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Blur {
    /// How much more saturated the last pass makes what it blurs: a little,
    /// so that glass over grey is not mud.
    pub const SATURATION: f32 = 1.2;

    /// The most times a picture is halved: enough for the widest blur a
    /// theme can ask for on the densest screen.
    const MOST: u32 = 8;

    /// The blur whose deviation is `deviation` pixels, as a Gaussian's,
    /// which is what CSS's `blur()` says; none for 0.
    #[must_use]
    pub fn new(deviation: u32) -> Option<Self> {
        if deviation == 0 {
            return None;
        }
        let wanted = float(deviation);
        // The fewest passes that reach it with samples no further apart
        // than two pixels, and how far apart they are to reach it exactly.
        // Each pass doubles how wide it is; how far its samples are apart
        // widens each pass as `spread` says, which was measured.
        let mut passes = 1;
        while passes < Self::MOST && widened(passes) * spread(2.0) < wanted {
            passes += 1;
        }
        let each = wanted / widened(passes);
        let offset = ((each * each - 0.5) / 1.8).max(0.0).sqrt().min(2.0);
        Some(Self {
            passes,
            offset,
            deviation,
        })
    }

    /// How many times the picture is halved, and made larger again.
    #[must_use]
    pub const fn passes(&self) -> u32 {
        self.passes
    }

    /// How far apart its samples are, in the pixels of the picture sampled.
    #[must_use]
    pub const fn offset(&self) -> f32 {
        self.offset
    }

    /// How far round a sheet of glass, in pixels, what is behind it is
    /// taken in to be blurred: three deviations, beyond which what there is
    /// counts for too little to be seen. Beyond that, the edge is taken.
    #[must_use]
    pub const fn reach(&self) -> u32 {
        self.deviation.saturating_mul(3)
    }

    /// `pixels`, a picture `width` by `height`, blurred. A picture short of
    /// pixels is taken to be see-through where they are missing.
    #[must_use]
    pub fn apply(&self, pixels: &[u8], (width, height): (u32, u32)) -> Vec<u8> {
        let (width, height) = (size(width), size(height));
        let (whole, _) = pixels.as_chunks::<4>();
        let mut start: Vec<[f32; 4]> = whole.iter().map(|pixel| pixel.map(f32::from)).collect();
        start.resize(width * height, [0.0; 4]);
        let mut picture = Layer {
            width,
            height,
            pixels: start,
        };
        // The sizes it is made smaller from, to be made that large again.
        let mut sizes = Vec::new();
        for _ in 0..self.passes {
            sizes.push((picture.width, picture.height));
            picture = picture.down(self.offset);
        }
        while let Some(size) = sizes.pop() {
            picture = picture.up(self.offset, size);
        }
        picture
            .pixels
            .iter()
            .map(|pixel| saturated(*pixel))
            .flat_map(|pixel| pixel.map(byte))
            .collect()
    }
}

impl Layer {
    /// It sampled bilinearly at `(x, y)`, in its pixels, the edge taken
    /// beyond it.
    fn sample(&self, (x, y): (f32, f32)) -> [f32; 4] {
        let (x, y) = (x - 0.5, y - 0.5);
        let (left, top) = (x.floor(), y.floor());
        let (across, down) = (x - left, y - top);
        let column = |at: f32| index(at, self.width);
        let row = |at: f32| index(at, self.height);
        let pixel = |column: usize, row: usize| {
            let at = row * self.width + column;
            self.pixels.get(at).copied().unwrap_or_default()
        };
        let (left, right) = (column(left), column(left + 1.0));
        let (top, bottom) = (row(top), row(top + 1.0));
        let corners = [
            (pixel(left, top), (1.0 - across) * (1.0 - down)),
            (pixel(right, top), across * (1.0 - down)),
            (pixel(left, bottom), (1.0 - across) * down),
            (pixel(right, bottom), across * down),
        ];
        weighed(&corners)
    }

    /// What a pass makes of it, `size` large: each pixel the taps, each at
    /// how far it is from the pixel's point and how much it counts.
    fn pass(&self, (width, height): (usize, usize), taps: &[((f32, f32), f32)]) -> Self {
        let across = float_of(self.width) / float_of(width);
        let down = float_of(self.height) / float_of(height);
        let total: f32 = taps.iter().map(|(_, weight)| weight).sum();
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let point = ((float_of(x) + 0.5) * across, (float_of(y) + 0.5) * down);
                let mut sum = [0.0_f32; 4];
                for ((dx, dy), weight) in taps {
                    let sample = self.sample((point.0 + dx, point.1 + dy));
                    for (to, from) in sum.iter_mut().zip(sample) {
                        *to = from.mul_add(weight / total, *to);
                    }
                }
                pixels.push(sum.map(f32::round));
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// It made half as large.
    fn down(&self, offset: f32) -> Self {
        let half = |length: usize| length.div_ceil(2).max(1);
        let taps = [
            ((0.0, 0.0), 4.0),
            ((-offset, -offset), 1.0),
            ((offset, -offset), 1.0),
            ((-offset, offset), 1.0),
            ((offset, offset), 1.0),
        ];
        self.pass((half(self.width), half(self.height)), &taps)
    }

    /// It made `size` large, twice as large as it is.
    fn up(&self, offset: f32, size: (usize, usize)) -> Self {
        let far = 2.0 * offset;
        let taps = [
            ((-far, 0.0), 1.0),
            ((far, 0.0), 1.0),
            ((0.0, -far), 1.0),
            ((0.0, far), 1.0),
            ((-offset, -offset), 2.0),
            ((offset, -offset), 2.0),
            ((-offset, offset), 2.0),
            ((offset, offset), 2.0),
        ];
        self.pass(size, &taps)
    }
}

/// How much wider a blur is, its samples `offset` apart, each time a
/// picture is halved: as measured of lines blurred, to within a few in a
/// hundred.
fn spread(offset: f32) -> f32 {
    offset.mul_add(offset * 1.8, 0.5).sqrt()
}

/// How many times wider `passes` make a blur than its samples' spread: two
/// to the power of the passes, but for a single pass, which was measured to
/// reach seven eighths of that.
fn widened(passes: u32) -> f32 {
    let doubled = float(1_u32 << passes);
    if passes == 1 { doubled * 0.88 } else { doubled }
}

/// The sum of `samples`, each times how much it counts.
fn weighed(samples: &[([f32; 4], f32); 4]) -> [f32; 4] {
    let mut sum = [0.0; 4];
    for (sample, weight) in samples {
        for (to, from) in sum.iter_mut().zip(sample) {
            *to = from.mul_add(*weight, *to);
        }
    }
    sum
}

/// A pixel, premultiplied, with its colour moved away from its luminance by
/// [`Blur::SATURATION`], within what its alpha allows.
fn saturated([red, green, blue, alpha]: [f32; 4]) -> [f32; 4] {
    let luminance = 0.0722_f32.mul_add(blue, 0.2126_f32.mul_add(red, 0.7152 * green));
    let moved = |channel: f32| {
        (channel - luminance)
            .mul_add(Blur::SATURATION, luminance)
            .clamp(0.0, alpha)
    };
    [moved(red), moved(green), moved(blue), alpha]
}

/// Which of `length` pixels is at `at`: the nearest at the edge beyond it.
fn index(at: f32, length: usize) -> usize {
    let last = float_of(length.saturating_sub(1));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "kept from nothing to the last pixel, a whole number"
    )]
    let index = at.clamp(0.0, last) as usize;
    index
}

/// `length` pixels, as many as a picture can have.
fn size(length: u32) -> usize {
    usize::try_from(length).unwrap_or(0)
}

/// A count of pixels as a number to work with.
fn float(value: u32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a picture is some thousands of pixels across at most"
    )]
    let value = value as f32;
    value
}

/// A count of pixels as a number to work with.
fn float_of(value: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a picture is some thousands of pixels across at most"
    )]
    let value = value as f32;
    value
}

/// The nearest byte to a value from 0 to 255.
fn byte(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is from 0 to 255"
    )]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture `width` by `height` of `pixel` everywhere.
    fn flat(pixel: [u8; 4], (width, height): (u32, u32)) -> Vec<u8> {
        pixel.repeat(size(width) * size(height))
    }

    /// How far, in pixels, what `row` holds is spread round its middle: its
    /// deviation, weighed by alpha.
    fn deviation(row: &[u8]) -> f32 {
        let alpha: Vec<f32> = row
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| f32::from(pixel[3]))
            .collect();
        let total: f32 = alpha.iter().sum();
        let at = |index: usize| float_of(index) + 0.5;
        let middle: f32 = alpha
            .iter()
            .enumerate()
            .map(|(i, a)| at(i) * a)
            .sum::<f32>()
            / total;
        let spread: f32 = alpha
            .iter()
            .enumerate()
            .map(|(i, a)| (at(i) - middle).powi(2) * a)
            .sum::<f32>();
        (spread / total).sqrt()
    }

    /// A picture `width` across and 16 down of nothing but a line down the
    /// middle, `thick` pixels wide, an even number, blurred by `blur`; its
    /// middle row.
    fn line(blur: &Blur, width: u32, thick: u32) -> Vec<u8> {
        let mut pixels = flat([0; 4], (width, 16));
        for row in 0..16 {
            for column in width / 2 - thick / 2..width / 2 + thick / 2 {
                let at = size((row * width + column) * 4);
                pixels[at..at + 4].copy_from_slice(&[255; 4]);
            }
        }
        let blurred = blur.apply(&pixels, (width, 16));
        let row = size(8 * width * 4);
        blurred[row..row + size(width * 4)].to_vec()
    }

    #[test]
    fn no_blur_is_none() {
        assert_eq!(Blur::new(0), None);
    }

    #[test]
    fn a_blur_is_as_wide_as_it_is_asked_to_be() {
        for asked in [2, 4, 8, 16, 24, 32, 48] {
            let blur = Blur::new(asked).unwrap();
            // A line as wide as the blur, so that what it spreads to is
            // not lost in rounding, less how much it was spread already.
            let thick = asked.max(2);
            let spread = deviation(&line(&blur, 1024, thick));
            let made = (spread * spread - float(thick * thick) / 12.0).sqrt();
            let asked = float(asked);
            let (passes, offset) = (blur.passes(), blur.offset());
            assert!(
                (made - asked).abs() <= asked * 0.12,
                "{asked}: {made}, in {passes} passes {offset} apart"
            );
        }
    }

    #[test]
    fn a_wider_blur_halves_the_picture_more_times() {
        let passes = |deviation| Blur::new(deviation).unwrap().passes();
        assert_eq!(passes(1), 1);
        assert!(passes(8) < passes(24));
        assert!(passes(24) < passes(64));
        assert!(passes(4096) <= Blur::MOST, "and no more than it can");
        for deviation in [1, 3, 8, 24, 40, 64, 256] {
            let blur = Blur::new(deviation).unwrap();
            assert!((0.0..=2.0).contains(&blur.offset()), "{deviation}");
        }
    }

    #[test]
    fn a_line_is_blurred_evenly_to_each_side() {
        let row = line(&Blur::new(8).unwrap(), 256, 2);
        let pixel = |index: usize| &row[index * 4..index * 4 + 4];
        for step in 0..60 {
            assert_eq!(pixel(127 - step), pixel(128 + step), "{step} from it");
        }
        // Falling away from it, and gone far from it.
        for step in 0..40 {
            assert!(pixel(128 + step)[3] >= pixel(129 + step)[3], "{step}");
        }
        assert!(pixel(128)[3] > pixel(140)[3]);
        assert_eq!(pixel(250)[3], 0);
    }

    #[test]
    fn grey_all_over_stays_as_it_was() {
        for pixel in [[0, 0, 0, 0], [80, 80, 80, 255], [64, 64, 64, 128]] {
            let blur = Blur::new(24).unwrap();
            let picture = flat(pixel, (97, 61));
            assert_eq!(blur.apply(&picture, (97, 61)), picture, "{pixel:?}");
        }
    }

    #[test]
    fn a_colour_all_over_stays_all_over_and_is_made_more_saturated() {
        let picture = flat([200, 100, 50, 255], (40, 30));
        let blurred = Blur::new(8).unwrap().apply(&picture, (40, 30));
        let (pixels, _) = blurred.as_chunks::<4>();
        let first = pixels[0];
        assert!(pixels.iter().all(|pixel| *pixel == first));
        assert!(first[0] > 200 && first[2] < 50, "{first:?}");
        assert_eq!(first[3], 255);
        // As far as what can be seen through allows, and no further.
        let faint = flat([60, 10, 0, 64], (40, 30));
        let blurred = Blur::new(8).unwrap().apply(&faint, (40, 30));
        assert_eq!(&blurred[..4], &[64, 8, 0, 64]);
    }

    #[test]
    fn an_edge_is_made_a_slope_as_wide_as_the_blur() {
        // Black on the left, white on the right.
        let (width, height) = (200_u32, 8_u32);
        let mut picture = flat([0, 0, 0, 255], (width, height));
        for (at, pixel) in picture.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if size(width) / 2 <= at % size(width) {
                *pixel = [255; 4];
            }
        }
        let blurred = Blur::new(8).unwrap().apply(&picture, (width, height));
        let red = |column: usize| blurred[(4 * size(width) + column) * 4];
        for column in 0..size(width) - 1 {
            assert!(red(column) <= red(column + 1), "rising at {column}");
        }
        assert_eq!((red(60), red(140)), (0, 255), "the slope ends");
        assert!((100..156).contains(&red(99)) && (100..156).contains(&red(100)));
        assert!(red(92) > 0 && red(108) < 255, "and is wide");
    }

    #[test]
    fn a_picture_short_of_pixels_is_see_through_where_they_are_missing() {
        let blurred = Blur::new(2).unwrap().apply(&[255; 4], (64, 64));
        assert_eq!(blurred.len(), 64 * 64 * 4);
        assert_eq!(&blurred[blurred.len() - 4..], &[0; 4]);
        assert_eq!(Blur::new(2).unwrap().apply(&[], (0, 0)), Vec::<u8>::new());
    }

    #[test]
    fn it_takes_in_three_deviations_round_what_it_blurs() {
        assert_eq!(Blur::new(24).unwrap().reach(), 72);
    }
}
