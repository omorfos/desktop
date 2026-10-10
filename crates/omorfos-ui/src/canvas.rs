//! A canvas to draw on, and text to draw on it.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which whoever is
//! drawing then hands to its renderer or its compositor. That keeps drawing
//! independent of where the pixels go, so the same code runs in the
//! compositor, in a shell component and in tests, and it only happens when
//! what is drawn changes, not every frame.

use cosmic_text::{
    Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    SwashCache, Wrap,
};
use omorfos_config::{Color, Font};

use crate::Scale;

/// A rectangle on a canvas: left, top, width, height, in units of layout.
pub type Area = (i32, i32, u32, u32);

/// A rectangle of pixels being drawn, initially transparent.
///
/// What is drawn is described in units of layout, and the canvas has as
/// many pixels to a unit as its scale says, so the same drawing comes out
/// sharp on any screen.
///
/// Pixels are stored as ARGB with premultiplied alpha, in the byte order
/// renderers call `Argb8888`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    /// The size in pixels.
    width: u32,
    height: u32,
    /// The size in units of layout.
    units: (u32, u32),
    scale: Scale,
    pixels: Vec<u8>,
}

impl Canvas {
    /// A canvas with one pixel to a unit.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self::scaled(width, height, Scale::ONE)
    }

    /// A canvas `width` by `height` units, with `scale` pixels to a unit.
    #[must_use]
    pub fn scaled(width: u32, height: u32, scale: Scale) -> Self {
        let (_, pixels_wide) = span(scale, 0, width);
        let (_, pixels_high) = span(scale, 0, height);
        let bytes =
            usize::try_from(u64::from(pixels_wide) * u64::from(pixels_high) * 4).unwrap_or(0);
        Self {
            width: pixels_wide,
            height: pixels_high,
            units: (width, height),
            scale,
            pixels: vec![0; bytes],
        }
    }

    /// The size in units of layout.
    #[must_use]
    pub const fn units(&self) -> (u32, u32) {
        self.units
    }

    /// The width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, row by row from the top.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// The four bytes of the pixel at `(x, y)`, if it is on the canvas.
    fn pixel_mut(&mut self, x: i32, y: i32) -> Option<&mut [u8]> {
        let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (u64::from(y) * u64::from(self.width) + u64::from(x)) * 4;
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        self.pixels.get_mut(offset..offset.saturating_add(4))
    }

    /// Draws `color` over the pixel at `(x, y)`, counted in pixels, as
    /// opaque as `coverage` says (`255` for fully). Does nothing off the
    /// canvas.
    pub fn blend(&mut self, x: i32, y: i32, color: Color, coverage: u8) {
        let alpha = u32::from(color.alpha) * u32::from(coverage) / 255;
        if let Some(pixel) = self.pixel_mut(x, y) {
            let source = [color.blue, color.green, color.red, 255];
            for (channel, source) in pixel.iter_mut().zip(source) {
                let mixed =
                    (u32::from(source) * alpha + u32::from(*channel) * (255 - alpha) + 127) / 255;
                *channel = u8::try_from(mixed).unwrap_or(u8::MAX);
            }
        }
    }

    /// Fills a rectangle with `color`, clipped to the canvas.
    pub fn fill(&mut self, (x, y, width, height): Area, color: Color) {
        let (x, width) = span(self.scale, x, width);
        let (y, height) = span(self.scale, y, height);
        let end = |start: i32, length: u32, limit: u32| {
            let end = i64::from(start) + i64::from(length);
            i32::try_from(end.min(i64::from(limit))).unwrap_or(i32::MAX)
        };
        for row in y.max(0)..end(y, height, self.height) {
            for column in x.max(0)..end(x, width, self.width) {
                self.blend(column, row, color, 255);
            }
        }
    }

    /// Cuts the top two corners to a quarter circle of `radius`, smoothly.
    pub fn round_top_corners(&mut self, radius: u32) {
        let (_, radius) = span(self.scale, 0, radius);
        let radius = radius.min(self.height).min(self.width / 2);
        for y in 0..radius {
            for x in 0..radius {
                let keep = corner_coverage(x, y, radius);
                let mirrored = self.width - 1 - x;
                for column in [x, mirrored] {
                    let pixel = self.pixel_mut(column.cast_signed(), y.cast_signed());
                    for channel in pixel.into_iter().flatten() {
                        let scaled = u32::from(*channel) * keep / SAMPLES;
                        *channel = u8::try_from(scaled).unwrap_or(u8::MAX);
                    }
                }
            }
        }
    }

    /// Draws a quarter ring in `color` that fills the canvas: the border of
    /// a rounded bottom corner, `thickness` units thick and smooth at both
    /// edges. It curves round the bottom-left corner, or the bottom-right
    /// one if `mirrored`. The canvas should be square.
    pub fn quarter_ring(&mut self, color: Color, thickness: u32, mirrored: bool) {
        let (_, thickness) = span(self.scale, 0, thickness);
        let size = self.width.min(self.height);
        for y in 0..size {
            for x in 0..size {
                // The ring's centre is the square's top corner on the inner
                // side, so rows are counted up from the bottom.
                let lit = ring_coverage(x, size - 1 - y, size, size.saturating_sub(thickness));
                let column = if mirrored { size - 1 - x } else { x };
                let coverage = u8::try_from(lit * 255 / SAMPLES).unwrap_or(u8::MAX);
                self.blend(column.cast_signed(), y.cast_signed(), color, coverage);
            }
        }
    }

    /// The pixel at `(x, y)`, counted in pixels, as `0xAARRGGBB`, or nothing
    /// (transparent) off the canvas. Meant for tests that check what was
    /// drawn.
    #[must_use]
    pub fn at(&self, x: u32, y: u32) -> u32 {
        let within = x < self.width && y < self.height;
        let offset = (u64::from(y) * u64::from(self.width) + u64::from(x)) * 4;
        let offset = usize::try_from(offset).ok().filter(|_| within);
        let bytes = offset.and_then(|offset| self.pixels.get(offset..offset.saturating_add(4)));
        let bytes = bytes.and_then(|bytes| <[u8; 4]>::try_from(bytes).ok());
        bytes.map_or(0, u32::from_le_bytes)
    }
}

/// Where a length of `length` units starting at `start` falls in pixels:
/// its first pixel and how many it covers.
///
/// Both ends are rounded, not the length, so that two lengths that meet in
/// units still meet in pixels.
fn span(scale: Scale, start: i32, length: u32) -> (i32, u32) {
    let end = i64::from(start) + i64::from(length);
    let end = scale.pixels(i32::try_from(end).unwrap_or(i32::MAX));
    let start = scale.pixels(start);
    (start, end.abs_diff(start))
}

/// Sub-pixel samples taken per pixel when smoothing a corner: a 4 by 4 grid.
const SAMPLES: u32 = 16;

/// How many of a corner pixel's samples lie inside the rounded corner.
///
/// `(x, y)` is the pixel's position from the corner of a square of side
/// `radius`; the circle is centred on the square's opposite corner.
fn corner_coverage(x: u32, y: u32, radius: u32) -> u32 {
    // Work in eighths of a pixel so sample centres are whole numbers.
    let centre = u64::from(radius) * 8;
    let inside = |x: u32, y: u32| {
        let (dx, dy) = (centre.abs_diff(u64::from(x)), centre.abs_diff(u64::from(y)));
        dx * dx + dy * dy <= centre * centre
    };
    let samples = (0..4).flat_map(|row| (0..4).map(move |column| (column, row)));
    let count = samples
        .filter(|(column, row)| inside(x * 8 + column * 2 + 1, y * 8 + row * 2 + 1))
        .count();
    u32::try_from(count).unwrap_or(SAMPLES)
}

/// How many of a pixel's samples lie in a ring: within `outer` of the
/// centre and not within `inner`.
///
/// `(x, y)` is the pixel's position from the corner of a square of side
/// `outer`; the ring is centred on the square's opposite corner.
fn ring_coverage(x: u32, y: u32, outer: u32, inner: u32) -> u32 {
    // In eighths of a pixel, as in `corner_coverage`.
    let centre = u64::from(outer) * 8;
    let (far, near) = (centre * centre, (u64::from(inner) * 8).pow(2));
    let inside = |x: u32, y: u32| {
        let (dx, dy) = (centre.abs_diff(u64::from(x)), centre.abs_diff(u64::from(y)));
        let distance = dx * dx + dy * dy;
        distance <= far && distance > near
    };
    let samples = (0..4).flat_map(|row| (0..4).map(move |column| (column, row)));
    let count = samples
        .filter(|(column, row)| inside(x * 8 + column * 2 + 1, y * 8 + row * 2 + 1))
        .count();
    u32::try_from(count).unwrap_or(SAMPLES)
}

/// Lays out and rasterises text. Holds the system's fonts, so it is created
/// once and kept.
pub struct Text {
    fonts: FontSystem,
    glyphs: SwashCache,
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Text").finish_non_exhaustive()
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl Text {
    /// Loads the fonts installed on the system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            glyphs: SwashCache::new(),
        }
    }

    /// How wide one line of `text` is when set in `font`, in units of
    /// layout, rounded up. For laying out what is then drawn.
    pub fn measure(&mut self, text: &str, font: &Font) -> u32 {
        // Font sizes are in points, at the conventional 96 units per inch.
        let size = f32::from(u16::try_from(font.size.get()).unwrap_or(u16::MAX)) * 96.0 / 72.0;
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size));
        buffer.set_wrap(Wrap::None);
        let attributes = Attrs::new().family(family(font));
        buffer.set_text(text, &attributes, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        let widest = buffer.layout_runs().map(|run| run.line_w);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a line of text is a small, positive number of units wide"
        )]
        let width = widest.fold(0.0, f32::max).ceil() as u32;
        width
    }

    /// Draws one line of `text` in `area`, vertically centred and cut short
    /// with an ellipsis if it does not fit.
    pub fn draw(&mut self, canvas: &mut Canvas, text: &str, area: Area, color: Color, font: &Font) {
        let (left, width) = span(canvas.scale, area.0, area.2);
        let (top, height) = span(canvas.scale, area.1, area.3);
        let float = |length: u32| f32::from(u16::try_from(length).unwrap_or(u16::MAX));
        // Font sizes are in points, at the conventional 96 units per inch.
        // A scale is a few quarters, which an `f32` holds exactly.
        #[expect(clippy::cast_possible_truncation, reason = "a scale is at most four")]
        let scale = canvas.scale.factor() as f32;
        let size = float(font.size.get()) * 96.0 / 72.0 * scale;
        let family = family(font);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, float(height)));
        buffer.set_wrap(Wrap::None);
        buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        buffer.set_size(Some(float(width)), Some(float(height)));
        buffer.set_text(text, &Attrs::new().family(family), Shaping::Advanced, None);
        let ink = cosmic_text::Color::rgba(color.red, color.green, color.blue, color.alpha);
        buffer.draw(
            &mut self.fonts,
            &mut self.glyphs,
            ink,
            |x, y, glyph_width, glyph_height, ink| {
                let ink_color = Color::rgba(ink.r(), ink.g(), ink.b(), 255);
                // Glyphs may overhang the line they are set on, by a pixel
                // of side bearing or more for an ellipsis in a tiny area.
                // Only the part inside the area is drawn.
                let columns = visible(x, glyph_width, width);
                for row in visible(y, glyph_height, height) {
                    for column in columns.clone() {
                        canvas.blend(left + column, top + row, ink_color, ink.a());
                    }
                }
            },
        );
    }
}

/// The font family a font names: one of the generic ones, or a family by
/// its own name.
fn family(font: &Font) -> Family<'_> {
    match font.family.as_str() {
        "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" => Family::Monospace,
        name => Family::Name(name),
    }
}

/// The part of the span `start..start + length` that lies within
/// `0..limit`, which is empty if none of it does.
fn visible(start: i32, length: u32, limit: u32) -> std::ops::Range<i32> {
    let clamp = |position: i64| i32::try_from(position.clamp(0, i64::from(limit))).unwrap_or(0);
    let start = i64::from(start);
    clamp(start)..clamp(start + i64::from(length))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    /// The number of pixels in a row range that are not exactly `background`.
    fn inked(canvas: &Canvas, columns: std::ops::Range<u32>, background: u32) -> usize {
        (0..canvas.height())
            .flat_map(|y| columns.clone().map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) != background)
            .count()
    }

    fn font(family: &str) -> Font {
        Font {
            family: family.to_owned(),
            ..Font::default()
        }
    }

    #[test]
    fn a_scaled_canvas_has_as_many_pixels_to_a_unit_as_its_scale_says() {
        let mut canvas = Canvas::scaled(10, 4, Scale::new(1.5));
        assert_eq!(canvas.units(), (10, 4));
        assert_eq!((canvas.width(), canvas.height()), (15, 6));

        // Two halves that meet in units meet in pixels, with no gap and no
        // overlap, although five units is seven and a half pixels.
        canvas.fill((0, 0, 5, 4), RED);
        canvas.fill((5, 0, 5, 4), BLUE);
        let row: Vec<u32> = (0..15).map(|x| canvas.at(x, 5)).collect();
        assert_eq!(row[..8], [0xffff_0000; 8]);
        assert_eq!(row[8..], [0xff00_00ff; 7]);
    }

    #[test]
    fn a_pixel_off_the_canvas_reads_as_nothing() {
        let mut canvas = Canvas::new(4, 3);
        canvas.fill((0, 0, 4, 3), RED);
        assert_eq!(canvas.at(3, 2), 0xffff_0000);
        assert_eq!(canvas.at(4, 2), 0, "past the right");
        assert_eq!(canvas.at(0, 3), 0, "past the bottom");
    }

    #[test]
    fn text_can_be_measured_before_it_is_drawn() {
        let mut text = Text::new();
        let font = Font::default();
        let short = text.measure("web", &font);
        let long = text.measure("web and more", &font);
        assert!(short > 0);
        assert!(long > short * 2, "{long} against {short}");
        assert_eq!(text.measure("", &font), 0, "nothing has no width");

        // Drawn in an area that wide, it fits: nothing is cut off, so the
        // last column or two hold ink that an ellipsis would not put there.
        let mut canvas = Canvas::new(long + 10, 24);
        text.draw(&mut canvas, "web and more", (0, 0, long, 24), RED, &font);
        let ink = |columns: std::ops::Range<u32>| inked(&canvas, columns, 0);
        assert!(ink(0..long) > 0);
        assert_eq!(ink(long..long + 10), 0, "and none of it lies beyond");
    }

    #[test]
    fn text_can_be_made_by_default() {
        let mut canvas = Canvas::new(60, 20);
        let mut text = Text::default();
        text.draw(&mut canvas, "Abc", (0, 0, 60, 20), RED, &Font::default());
        assert!((0..60).any(|x| (0..20).any(|y| canvas.at(x, y) != 0)));
    }

    #[test]
    fn a_new_canvas_is_transparent() {
        let canvas = Canvas::new(4, 3);
        assert_eq!((canvas.width(), canvas.height()), (4, 3));
        assert_eq!(canvas.bytes().len(), 48);
        assert_eq!(canvas.at(3, 2), 0);
    }

    #[test]
    fn fill_colours_a_rectangle_and_nothing_else() {
        let mut canvas = Canvas::new(8, 8);
        canvas.fill((2, 3, 4, 2), RED);
        assert_eq!(canvas.at(2, 3), 0xffff_0000);
        assert_eq!(canvas.at(5, 4), 0xffff_0000);
        assert_eq!(canvas.at(1, 3), 0);
        assert_eq!(canvas.at(6, 3), 0);
        assert_eq!(canvas.at(2, 5), 0);
    }

    #[test]
    fn fill_is_clipped_to_the_canvas() {
        let mut canvas = Canvas::new(4, 4);
        canvas.fill((-10, -10, 12, 12), RED);
        canvas.fill((3, 3, 100, 100), BLUE);
        assert_eq!(canvas.at(0, 0), 0xffff_0000);
        assert_eq!(canvas.at(1, 1), 0xffff_0000);
        assert_eq!(canvas.at(2, 2), 0);
        assert_eq!(canvas.at(3, 3), 0xff00_00ff);
    }

    #[test]
    fn blending_mixes_by_coverage_and_alpha() {
        let mut canvas = Canvas::new(2, 1);
        canvas.fill((0, 0, 2, 1), BLUE);
        canvas.blend(0, 0, RED, 128);
        assert_eq!(canvas.at(0, 0), 0xff80_007f, "half red over blue");
        canvas.blend(1, 0, Color::rgba(255, 0, 0, 0), 255);
        assert_eq!(
            canvas.at(1, 0),
            0xff00_00ff,
            "a transparent colour changes nothing"
        );
    }

    #[test]
    fn a_translucent_colour_on_nothing_is_stored_premultiplied() {
        let mut canvas = Canvas::new(1, 1);
        canvas.blend(0, 0, Color::rgba(255, 255, 255, 128), 255);
        assert_eq!(canvas.at(0, 0), 0x8080_8080);
    }

    #[test]
    fn blending_off_the_canvas_does_nothing() {
        let mut canvas = Canvas::new(2, 2);
        for (x, y) in [(-1, 0), (0, -1), (2, 0), (0, 2)] {
            canvas.blend(x, y, RED, 255);
        }
        assert_eq!(canvas, Canvas::new(2, 2));
    }

    #[test]
    fn rounding_clears_the_top_corners_smoothly_and_leaves_the_bottom_square() {
        let mut canvas = Canvas::new(40, 20);
        canvas.fill((0, 0, 40, 20), RED);
        canvas.round_top_corners(8);

        assert_eq!(canvas.at(0, 0), 0, "the very corner is cut away");
        assert_eq!(canvas.at(39, 0), 0);
        assert_eq!(
            canvas.at(8, 0),
            0xffff_0000,
            "past the radius the edge is intact"
        );
        assert_eq!(canvas.at(0, 8), 0xffff_0000);
        assert_eq!(canvas.at(20, 10), 0xffff_0000);
        assert_eq!(
            canvas.at(0, 19),
            0xffff_0000,
            "the bottom corners stay square"
        );
        assert_eq!(canvas.at(39, 19), 0xffff_0000);
        let partly = (1..255).contains(&(canvas.at(2, 2) >> 24));
        assert!(partly, "the curve's edge is anti-aliased");
        assert_eq!(
            canvas.at(2, 2),
            canvas.at(37, 2),
            "the corners mirror each other"
        );
    }

    #[test]
    fn rounding_is_limited_by_the_size_of_the_canvas() {
        let mut canvas = Canvas::new(6, 2);
        canvas.fill((0, 0, 6, 2), RED);
        canvas.round_top_corners(100);
        assert!(
            canvas.at(0, 0) >> 24 < 255,
            "the corner is cut, with a radius of 2"
        );
        assert_eq!(canvas.at(2, 1), 0xffff_0000);

        let mut square = Canvas::new(4, 4);
        square.fill((0, 0, 4, 4), RED);
        let before = square.clone();
        square.round_top_corners(0);
        assert_eq!(square, before, "a radius of zero changes nothing");
    }

    #[test]
    fn text_is_drawn_in_its_area_in_the_requested_colour() {
        let mut text = Text::new();
        let mut canvas = Canvas::new(200, 28);
        canvas.fill((0, 0, 200, 28), BLUE);
        text.draw(
            &mut canvas,
            "Terminal",
            (50, 0, 150, 28),
            RED,
            &Font::default(),
        );

        assert!(
            inked(&canvas, 50..200, argb(BLUE)) > 20,
            "glyphs were drawn"
        );
        assert_eq!(
            inked(&canvas, 0..50, argb(BLUE)),
            0,
            "nothing left of the area"
        );
        let reddest = (50..200).flat_map(|x| (0..28).map(move |y| (x, y)));
        assert!(
            reddest
                .map(|(x, y)| canvas.at(x, y))
                .any(|pixel| pixel == argb(RED))
        );
        assert_eq!(format!("{text:?}"), "Text { .. }");
    }

    #[test]
    fn only_the_part_of_a_span_inside_the_limit_is_visible() {
        assert_eq!(visible(3, 4, 10), 3..7, "wholly inside");
        assert_eq!(visible(-2, 5, 10), 0..3, "overhanging the start");
        assert_eq!(visible(8, 5, 10), 8..10, "overhanging the end");
        assert!(visible(-9, 4, 10).is_empty(), "wholly before");
        assert!(visible(12, 4, 10).is_empty(), "wholly after");
        assert!(visible(i32::MAX, u32::MAX, 10).is_empty(), "no overflow");
    }

    #[test]
    fn text_never_leaves_its_area_even_when_the_area_is_tiny() {
        let mut text = Text::new();
        let mut canvas = Canvas::new(100, 40);
        // Three pixels cannot hold even an ellipsis, in any font.
        text.draw(
            &mut canvas,
            "Wide title",
            (50, 10, 3, 12),
            RED,
            &Font::default(),
        );
        assert_eq!(inked(&canvas, 0..50, 0), 0, "left of the area");
        assert_eq!(inked(&canvas, 53..100, 0), 0, "right of the area");
        let above_or_below = (0..100).flat_map(|x| (0..10).chain(22..40).map(move |y| (x, y)));
        assert_eq!(
            above_or_below
                .filter(|(x, y)| canvas.at(*x, *y) != 0)
                .count(),
            0
        );
    }

    #[test]
    fn text_that_does_not_fit_is_cut_short_rather_than_spilling_over() {
        let mut text = Text::new();
        let mut canvas = Canvas::new(300, 28);
        let long = "A very long window title that cannot possibly fit in a narrow tab";
        text.draw(&mut canvas, long, (0, 0, 80, 28), RED, &Font::default());
        assert!(inked(&canvas, 0..80, 0) > 20);
        assert_eq!(inked(&canvas, 90..300, 0), 0);
    }

    #[test]
    fn generic_and_named_font_families_all_draw() {
        let mut text = Text::new();
        for family in [
            "sans-serif",
            "serif",
            "monospace",
            "A Font That Is Not Installed",
        ] {
            let mut canvas = Canvas::new(100, 28);
            text.draw(&mut canvas, "Abc", (0, 0, 100, 28), RED, &font(family));
            assert!(inked(&canvas, 0..100, 0) > 5, "{family} drew nothing");
        }
    }
}
