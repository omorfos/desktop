//! A canvas to draw on, and text to draw on it.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which whoever is
//! drawing then hands to its renderer or its compositor. That keeps drawing
//! independent of where the pixels go, so the same code runs in the
//! compositor, in a shell component and in tests, and it only happens when
//! what is drawn changes, not every frame.

use cosmic_text::{
    Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    Style as Slant, SwashCache, Weight, Wrap,
};
use omorfos_config::{Color, Edges, Font, Style};

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

    /// Draws a part as a box filling `area`: its border along each side
    /// and its background within, with the corners rounded as far as the
    /// style says. Margin and padding are for whoever lays the box out;
    /// see [`inset`].
    ///
    /// Where two sides with borders meet at a rounded corner, the border
    /// goes round it as thick as the thicker of the two and in its colour.
    pub fn draw_box(&mut self, area: Area, style: &Style) {
        let (left, width) = span(self.scale, area.0, area.2);
        let (top, height) = span(self.scale, area.1, area.3);
        let pixels = |units: u32| span(self.scale, 0, units).1;
        let border = style.border;
        let lines = [border.top, border.right, border.bottom, border.left];
        let widths = lines.map(|line| pixels(line.width));
        if style.background.alpha == 0 && widths == [0; 4] {
            return;
        }
        let shape = Shape {
            width,
            height,
            radius: pixels(style.radius).min(width / 2).min(height / 2),
            widths,
        };
        let columns = visible(left, width, self.width);
        for row in visible(top, height, self.height) {
            for column in columns.clone() {
                let within = (column.abs_diff(left), row.abs_diff(top));
                let (side, edge, fill) = shape.at(within);
                let line = lines.get(side).map_or(style.background, |line| line.color);
                self.blend(column, row, line, edge);
                self.blend(column, row, style.background, fill);
            }
        }
    }

    /// Draws a picture in `area`, as large as fits with its shape kept, in
    /// the middle. A picture with more pixels than it is given room for is
    /// smoothed down; one with fewer is drawn blocky.
    pub fn draw_image(&mut self, area: Area, image: &Image) {
        let (left, width) = span(self.scale, area.0, area.2);
        let (top, height) = span(self.scale, area.1, area.3);
        let (wide, tall) = (u64::from(image.width), u64::from(image.height));
        // The largest size of the picture's shape that fits the room.
        let fits_across = u64::from(width) * tall <= u64::from(height) * wide;
        let (across, down) = if fits_across {
            (u64::from(width), u64::from(width) * tall / wide)
        } else {
            (u64::from(height) * wide / tall, u64::from(height))
        };
        let (across, down) = (
            u32::try_from(across).unwrap_or(width),
            u32::try_from(down).unwrap_or(height),
        );
        let left = left.saturating_add_unsigned((width - across) / 2);
        let top = top.saturating_add_unsigned((height - down) / 2);
        let columns = visible(left, across, self.width);
        for row in visible(top, down, self.height) {
            for column in columns.clone() {
                let within = (column.abs_diff(left), row.abs_diff(top));
                let (color, opacity) = image.sample(within, (across, down));
                self.blend(column, row, color, opacity);
            }
        }
    }

    /// Draws a picture made exactly `area` in size, counted in pixels and
    /// not in units, so that it may be any shape and need not be on the
    /// canvas; what is off it is not drawn. It is smoothed both when it is
    /// drawn smaller than it is, each pixel the average of those of the
    /// picture it covers, and when larger, each the blend of the two
    /// nearest along each side.
    pub fn draw_stretched(&mut self, (left, top, across, down): Area, image: &Image) {
        let columns = visible(left, across, self.width);
        let taps = |range: std::ops::Range<i32>, start: i32, own: u32, drawn: u32| {
            let within = range.map(move |at| at.abs_diff(start));
            within.map(move |at| Taps::new(own, drawn, at))
        };
        let across_taps: Vec<Taps> = taps(columns.clone(), left, image.width, across).collect();
        let rows = visible(top, down, self.height);
        for (row, down_taps) in rows.clone().zip(taps(rows, top, image.height, down)) {
            for (column, across_taps) in columns.clone().zip(&across_taps) {
                let (color, opacity) = image.mixed(across_taps, &down_taps);
                self.blend(column, row, color, opacity);
            }
        }
    }

    /// Draws a picture as it is, one of its pixels to each of the canvas's,
    /// with its top left corner at the pixel `(x, y)`, which may be off the
    /// canvas.
    pub fn draw_unscaled(&mut self, (x, y): (i32, i32), image: &Image) {
        let columns = visible(x, image.width, self.width);
        for row in visible(y, image.height, self.height) {
            for column in columns.clone() {
                let own = (column.abs_diff(x), row.abs_diff(y));
                let [red, green, blue, alpha] = image.pixel(own);
                self.blend(column, row, Color::rgb(red, green, blue), alpha);
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

/// A picture, such as an icon: rows of pixels, each a red, a green, a blue
/// and an opacity that the colours have not been multiplied by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    /// A picture `width` by `height` pixels from four bytes for each. It
    /// is nothing if it has no size or the bytes are not that many.
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let needed = u64::from(width) * u64::from(height) * 4;
        let sized = width > 0 && height > 0 && u64::try_from(pixels.len()) == Ok(needed);
        sized.then_some(Self {
            width,
            height,
            pixels,
        })
    }

    /// How many pixels wide and tall it is.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The same picture all in one colour: every pixel `color`, as opaque
    /// as it was, and no more opaque than the colour. For pictures that
    /// are a shape to be drawn in the colour of the text beside them.
    #[must_use]
    pub fn tinted(&self, color: Color) -> Self {
        let pixels = self.pixels.as_chunks::<4>().0.iter().flat_map(|pixel| {
            let [_, _, _, own] = *pixel;
            let alpha = u16::from(own) * u16::from(color.alpha) / u16::from(u8::MAX);
            let alpha = u8::try_from(alpha).unwrap_or(u8::MAX);
            [color.red, color.green, color.blue, alpha]
        });
        Self {
            width: self.width,
            height: self.height,
            pixels: pixels.collect(),
        }
    }

    /// The red, green, blue and opacity of the pixel at `(x, y)`, which is
    /// on the picture.
    fn pixel(&self, (x, y): (u32, u32)) -> [u8; 4] {
        let offset = (u64::from(y) * u64::from(self.width) + u64::from(x)) * 4;
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let pixel = self.pixels.get(offset..offset.saturating_add(4));
        pixel.map_or([0; 4], |pixel| {
            <[u8; 4]>::try_from(pixel).unwrap_or_default()
        })
    }

    /// The colour the pixels `across` and `down` say make, each counting
    /// for as much as both say, and how opaque it is.
    fn mixed(&self, across: &Taps, down: &Taps) -> (Color, u8) {
        let mut sums = [0_u64; 4];
        for (y, high) in down.each() {
            for (x, wide) in across.each() {
                let [red, green, blue, alpha] = self.pixel((x, y));
                let weight = high * wide;
                // Colours count for as much as they are opaque, so that a
                // clear pixel's colour does not tint its neighbours.
                let opaque = weight * u64::from(alpha);
                sums[0] += u64::from(red) * opaque;
                sums[1] += u64::from(green) * opaque;
                sums[2] += u64::from(blue) * opaque;
                sums[3] += opaque;
            }
        }
        let byte = |value: u64| u8::try_from(value).unwrap_or(u8::MAX);
        let channel = |sum: u64| byte(sum.checked_div(sums[3]).unwrap_or(0));
        let color = Color::rgb(channel(sums[0]), channel(sums[1]), channel(sums[2]));
        let whole = across.total * down.total;
        (color, byte(sums[3].checked_div(whole).unwrap_or(0)))
    }

    /// The colour of the picture at one pixel of it drawn `across` by
    /// `down` pixels, and how opaque it is there: the average of the
    /// picture's own pixels that fall on that one.
    fn sample(&self, (x, y): (u32, u32), (across, down): (u32, u32)) -> (Color, u8) {
        let from = |at: u32, of: u32, own: u32| u64::from(at) * u64::from(own) / u64::from(of);
        let run = |at: u32, of: u32, own: u32| {
            let start = from(at, of, own);
            // At least one pixel, where the picture is being drawn larger.
            start..from(at + 1, of, own).max(start + 1)
        };
        let (columns, rows) = (run(x, across, self.width), run(y, down, self.height));
        let places = rows.flat_map(|row| columns.clone().map(move |column| (column, row)));
        let mut sums = [0_u64; 4];
        let mut count = 0_u64;
        for (column, row) in places {
            let offset = (row * u64::from(self.width) + column) * 4;
            let offset = usize::try_from(offset).unwrap_or(usize::MAX);
            let pixel = self.pixels.get(offset..offset.saturating_add(4));
            let [red, green, blue, alpha] = pixel.map_or([0; 4], |pixel| {
                <[u8; 4]>::try_from(pixel).unwrap_or_default()
            });
            // Colours count for as much as they are opaque, so that a
            // clear pixel's colour does not tint its neighbours.
            let weight = u64::from(alpha);
            sums[0] += u64::from(red) * weight;
            sums[1] += u64::from(green) * weight;
            sums[2] += u64::from(blue) * weight;
            sums[3] += weight;
            count += 1;
        }
        let byte = |value: u64| u8::try_from(value).unwrap_or(u8::MAX);
        let channel = |sum: u64| byte(sum.checked_div(sums[3]).unwrap_or(0));
        let color = Color::rgb(channel(sums[0]), channel(sums[1]), channel(sums[2]));
        (color, byte(sums[3].checked_div(count).unwrap_or(0)))
    }
}

/// Which of a picture's pixels along one side make one pixel of it drawn
/// another size, and how much each counts for: as much as `weights` says
/// of `total`.
struct Taps {
    first: u32,
    weights: Vec<u64>,
    total: u64,
}

impl Taps {
    /// The pixels of a side `own` pixels long that make its pixel `at`,
    /// drawn `drawn` long.
    ///
    /// Everything is counted in parts of a pixel too small to need
    /// rounding: smaller, a pixel drawn covers `own` parts of the picture,
    /// each of whose pixels is `drawn` parts; larger, the two nearest are
    /// blended by how near each is, in parts twice as fine, so that the
    /// middle of a pixel falls on one.
    fn new(own: u32, drawn: u32, at: u32) -> Self {
        let (own, drawn, at) = (u64::from(own), u64::from(drawn), u64::from(at));
        if own > drawn {
            let (start, end) = (at * own, (at + 1) * own);
            let (first, last) = (start / drawn, end.div_ceil(drawn).min(own));
            let weights = (first..last).map(|pixel| {
                let (from, to) = (pixel * drawn, (pixel + 1) * drawn);
                to.min(end) - from.max(start)
            });
            Self::from(first, weights.collect(), own)
        } else {
            let part = 2 * drawn;
            // Where the middle of the pixel drawn falls on the picture,
            // from the middle of its first pixel, and no further than the
            // middle of its last.
            let middle = ((2 * at + 1) * own).saturating_sub(drawn);
            let middle = middle.min((own.saturating_sub(1)) * part);
            let (first, along) = (middle / part, middle % part);
            Self::from(first, vec![part - along, along], part)
        }
    }

    fn from(first: u64, weights: Vec<u64>, total: u64) -> Self {
        Self {
            first: u32::try_from(first).unwrap_or(u32::MAX),
            weights,
            total,
        }
    }

    /// Each pixel that counts, with how much it does. One that counts for
    /// nothing may be past the end, and is left out.
    fn each(&self) -> impl Iterator<Item = (u32, u64)> {
        let weights = self.weights.iter().copied().zip(self.first..);
        weights
            .filter(|(weight, _)| *weight > 0)
            .map(|(weight, pixel)| (pixel, weight))
    }
}

/// The outline of a box in pixels: its size, how far its corners are
/// rounded, and how thick its border is on the top, right, bottom and left.
struct Shape {
    width: u32,
    height: u32,
    radius: u32,
    widths: [u32; 4],
}

impl Shape {
    /// What is at a pixel of the box, counted from its top-left corner:
    /// which side's border, how much of the pixel that border covers, and
    /// how much the background does, each out of 255.
    fn at(&self, (x, y): (u32, u32)) -> (usize, u8, u8) {
        let [top, right, bottom, left] = self.widths;
        // How far in from each side the pixel is.
        let (from_right, from_bottom) = (self.width - 1 - x, self.height - 1 - y);
        // The corner it is in, if it is in one: how far in from the two
        // sides that meet there, and which sides they are.
        let (across, level) = if x < self.radius {
            (x, 3)
        } else {
            (from_right, 1)
        };
        let (down, upright) = if y < self.radius {
            (y, 0)
        } else {
            (from_bottom, 2)
        };
        let cornered = across < self.radius && down < self.radius;
        let outer = if cornered {
            corner_coverage(across, down, self.radius)
        } else {
            SAMPLES
        };
        let share = |samples: u32| u8::try_from(samples * 255 / SAMPLES).unwrap_or(u8::MAX);

        let banded = [y < top, from_right < right, from_bottom < bottom, x < left];
        // The top and bottom borders run the whole width.
        let band = [0, 2, 3, 1].into_iter().find(|side| banded[*side]);
        if let Some(side) = band {
            return (side, share(outer), 0);
        }
        let (beside, above) = (self.widths[level], self.widths[upright]);
        if !cornered || beside == 0 || above == 0 {
            return (upright, 0, share(outer));
        }
        // Round a corner where two borders meet, the border is a ring.
        let thick = beside.max(above);
        let side = if above >= beside { upright } else { level };
        let inner = self.radius.saturating_sub(thick);
        let (inward, downward) = (across - beside, down - above);
        let hollow = if inward < inner && downward < inner {
            corner_coverage(inward, downward, inner)
        } else {
            SAMPLES
        };
        let hollow = hollow.min(outer);
        (side, share(outer - hollow), share(hollow))
    }
}

/// The part of `area` left inside `edges`: a box less its border and
/// padding, or the room for a box less its margin.
#[must_use]
pub fn inset((x, y, width, height): Area, edges: Edges<u32>) -> Area {
    (
        x.saturating_add_unsigned(edges.left),
        y.saturating_add_unsigned(edges.top),
        width.saturating_sub(edges.across()),
        height.saturating_sub(edges.down()),
    )
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
        buffer.set_text(text, &attributes(font), Shaping::Advanced, None);
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

    /// Breaks `text` into lines no wider than `width` units of layout when
    /// set in `font`, at the spaces between its words. There are at most
    /// `most` lines, and at least one: what is left over stays on the last,
    /// to be cut short when it is drawn. A word wider than a line has a
    /// line to itself.
    pub fn wrap(&mut self, text: &str, width: u32, font: &Font, most: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let longer = format!("{line} {word}");
            let last = lines.len() + 1 >= most;
            if line.is_empty() {
                word.clone_into(&mut line);
            } else if last || self.measure(&longer, font) <= width {
                line = longer;
            } else {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            }
        }
        lines.extend((!line.is_empty()).then_some(line));
        lines
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
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, float(height)));
        buffer.set_wrap(Wrap::None);
        buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        buffer.set_size(Some(float(width)), Some(float(height)));
        buffer.set_text(text, &attributes(font), Shaping::Advanced, None);
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

/// How text in a font is to be set: its family, its weight and whether it
/// slants.
fn attributes(font: &Font) -> Attrs<'_> {
    let slant = if font.italic {
        Slant::Italic
    } else {
        Slant::Normal
    };
    let attributes = Attrs::new().family(family(font));
    attributes.weight(Weight(font.weight)).style(slant)
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
    fn text_is_broken_into_lines_at_spaces_to_fit_a_width() {
        let mut text = Text::new();
        let font = Font::default();
        let words = "one two three four five six seven eight nine ten eleven twelve";
        let width = text.measure("one two three four", &font);
        let lines = text.wrap(words, width, &font, 10);
        assert!(lines.len() >= 3, "{lines:?}");
        for line in &lines {
            assert!(text.measure(line, &font) <= width, "{line:?} in {width}");
        }
        assert_eq!(lines.join(" "), words, "nothing is lost or reordered");
        // Room for it all on one line leaves it on one.
        assert_eq!(text.wrap("  one   two ", 10_000, &font, 10), ["one two"]);
        assert_eq!(text.wrap("", 100, &font, 10), [""; 0]);
        assert_eq!(text.wrap("   ", 100, &font, 10), [""; 0]);
    }

    #[test]
    fn what_does_not_fit_the_lines_allowed_stays_on_the_last() {
        let mut text = Text::new();
        let font = Font::default();
        let words = "one two three four five six seven eight nine ten eleven twelve";
        let width = text.measure("one two three", &font);
        let lines = text.wrap(words, width, &font, 2);
        assert_eq!(lines.len(), 2);
        assert!(text.measure(&lines[0], &font) <= width);
        assert!(
            text.measure(&lines[1], &font) > width,
            "the rest is all there"
        );
        assert_eq!(lines.join(" "), words);
        assert_eq!(text.wrap(words, width, &font, 1), [words]);
        assert_eq!(
            text.wrap(words, width, &font, 0),
            [words],
            "never fewer than one"
        );
        // A word too wide for a line is not broken.
        let long = "supercalifragilisticexpialidocious";
        let lines = text.wrap(&format!("a {long} b"), 30, &font, 10);
        assert_eq!(lines, ["a", long, "b"]);
    }

    #[test]
    fn text_can_be_made_by_default() {
        let mut canvas = Canvas::new(60, 20);
        let mut text = Text::default();
        text.draw(&mut canvas, "Abc", (0, 0, 60, 20), RED, &Font::default());
        assert!((0..60).any(|x| (0..20).any(|y| canvas.at(x, y) != 0)));
    }

    const GREEN: Color = Color::rgb(0, 255, 0);
    const WHITE: Color = Color::rgb(255, 255, 255);

    fn line(color: Color, width: u32) -> omorfos_config::Line {
        omorfos_config::Line { color, width }
    }

    /// A style with a background and the same border on every side.
    fn boxed(background: Color, border: Color, width: u32, radius: u32) -> Style {
        Style {
            background,
            border: Edges::all(line(border, width)),
            radius,
            ..Style::NONE
        }
    }

    #[test]
    fn a_box_is_its_background_inside_a_border_on_each_side() {
        let style = Style {
            background: WHITE,
            border: Edges {
                top: line(RED, 1),
                right: line(GREEN, 2),
                bottom: line(BLUE, 3),
                left: line(Color::rgb(9, 9, 9), 4),
            },
            ..Style::NONE
        };
        let mut canvas = Canvas::new(40, 30);
        canvas.draw_box((5, 5, 30, 20), &style);

        assert_eq!(canvas.at(4, 15), 0, "nothing outside it");
        assert_eq!(canvas.at(35, 15), 0);
        assert_eq!(canvas.at(20, 4), 0);
        assert_eq!(canvas.at(20, 25), 0);
        assert_eq!(canvas.at(20, 5), argb(RED));
        assert_eq!(canvas.at(20, 6), argb(WHITE));
        assert_eq!(canvas.at(33, 15), argb(GREEN));
        assert_eq!(canvas.at(32, 15), argb(WHITE));
        assert_eq!(canvas.at(20, 22), argb(BLUE));
        assert_eq!(canvas.at(20, 21), argb(WHITE));
        assert_eq!(canvas.at(8, 15), argb(Color::rgb(9, 9, 9)));
        assert_eq!(canvas.at(9, 15), argb(WHITE));
        // The top and bottom run the whole width.
        assert_eq!(canvas.at(5, 5), argb(RED));
        assert_eq!(canvas.at(34, 5), argb(RED));
        assert_eq!(canvas.at(5, 24), argb(BLUE));
        assert_eq!(canvas.at(34, 24), argb(BLUE));
    }

    #[test]
    fn a_box_with_nothing_to_show_draws_nothing() {
        let mut canvas = Canvas::new(20, 20);
        canvas.fill((0, 0, 20, 20), RED);
        let before = canvas.clone();
        canvas.draw_box((2, 2, 10, 10), &Style::NONE);
        assert_eq!(canvas, before);
        // A border of no thickness is not one, whatever its colour.
        canvas.draw_box((2, 2, 10, 10), &boxed(Color::rgba(0, 0, 0, 0), BLUE, 0, 4));
        assert_eq!(canvas, before);
    }

    #[test]
    fn a_box_is_clipped_to_the_canvas() {
        let mut canvas = Canvas::new(10, 10);
        canvas.draw_box((-5, -5, 100, 100), &boxed(WHITE, RED, 2, 0));
        assert_eq!(canvas.at(0, 0), argb(WHITE), "its border is off the canvas");
        assert_eq!(canvas.at(9, 9), argb(WHITE));
        canvas.draw_box((50, 50, 5, 5), &boxed(BLUE, RED, 1, 0));
        assert_eq!(canvas.at(9, 9), argb(WHITE));
    }

    #[test]
    fn a_box_has_as_many_pixels_to_a_unit_as_the_canvas() {
        let mut canvas = Canvas::scaled(20, 20, Scale::new(2.0));
        canvas.draw_box((5, 5, 10, 10), &boxed(WHITE, RED, 1, 0));
        assert_eq!(canvas.at(9, 20), 0);
        assert_eq!(canvas.at(10, 20), argb(RED));
        assert_eq!(canvas.at(11, 20), argb(RED));
        assert_eq!(canvas.at(12, 20), argb(WHITE));
        assert_eq!(canvas.at(29, 20), argb(RED));
        assert_eq!(canvas.at(30, 20), 0);
    }

    #[test]
    fn a_rounded_box_has_smooth_corners_and_shows_what_is_under_them() {
        let mut canvas = Canvas::new(40, 40);
        canvas.fill((0, 0, 40, 40), BLUE);
        canvas.draw_box((0, 0, 40, 40), &boxed(WHITE, WHITE, 0, 10));

        for (x, y) in [(0, 0), (39, 0), (0, 39), (39, 39)] {
            assert_eq!(canvas.at(x, y), argb(BLUE), "the corner is left alone");
        }
        for (x, y) in [
            (20, 0),
            (0, 20),
            (39, 20),
            (20, 39),
            (20, 20),
            (4, 4),
            (35, 35),
        ] {
            assert_eq!(canvas.at(x, y), argb(WHITE), "({x}, {y})");
        }
        // Along the curve the two are mixed, the same in every corner.
        let mixed = canvas.at(1, 4);
        assert!(mixed != argb(BLUE) && mixed != argb(WHITE), "{mixed:#x}");
        assert_eq!(canvas.at(38, 4), mixed);
        assert_eq!(canvas.at(1, 35), mixed);
        assert_eq!(canvas.at(38, 35), mixed);
    }

    #[test]
    fn the_border_of_a_rounded_box_goes_round_its_corners() {
        let mut canvas = Canvas::new(40, 40);
        canvas.draw_box((0, 0, 40, 40), &boxed(WHITE, RED, 2, 12));

        assert_eq!(canvas.at(0, 0), 0, "outside the corner");
        assert_eq!(canvas.at(20, 0), argb(RED));
        assert_eq!(canvas.at(20, 1), argb(RED));
        assert_eq!(canvas.at(20, 2), argb(WHITE));
        assert_eq!(canvas.at(0, 20), argb(RED));
        // On the diagonal of the corner: nothing, then border, then
        // background, with no gap and no background reaching the outside.
        let diagonal: Vec<u32> = (0..12).map(|step| canvas.at(step, step)).collect();
        let red = diagonal
            .iter()
            .position(|pixel| *pixel == argb(RED))
            .unwrap();
        let white = diagonal.iter().position(|pixel| *pixel == argb(WHITE));
        let white = white.unwrap();
        assert!(red >= 2 && white > red, "{diagonal:x?}");
        assert!(
            white - red <= 3,
            "about as thick as the border: {diagonal:x?}"
        );
        assert!(diagonal[..red].iter().all(|pixel| *pixel != argb(WHITE)));
        // The same in the opposite corner.
        let opposite: Vec<u32> = (0..12)
            .map(|step| canvas.at(39 - step, 39 - step))
            .collect();
        assert_eq!(opposite, diagonal);
    }

    #[test]
    fn where_borders_of_two_thicknesses_meet_the_corner_takes_the_thicker() {
        let style = Style {
            background: WHITE,
            border: Edges {
                top: line(RED, 1),
                right: line(RED, 1),
                bottom: line(BLUE, 4),
                left: line(GREEN, 6),
            },
            radius: 12,
            ..Style::NONE
        };
        let mut canvas = Canvas::new(40, 40);
        canvas.draw_box((0, 0, 40, 40), &style);
        // Top left: the left is thicker. Bottom left: the left still.
        // Bottom right: the bottom.
        let found = |x: u32, y: u32, color: Color| {
            let around = (0..3).flat_map(|dx| (0..3).map(move |dy| (x + dx, y + dy)));
            around
                .into_iter()
                .any(|(x, y)| canvas.at(x, y) == argb(color))
        };
        assert!(found(6, 1, GREEN));
        assert!(found(3, 33, GREEN));
        assert!(found(33, 35, BLUE));
        assert_eq!(canvas.at(20, 20), argb(WHITE));
    }

    #[test]
    fn a_rounded_box_with_a_line_only_along_the_bottom_keeps_it_straight() {
        let style = Style {
            background: WHITE,
            border: Edges {
                bottom: line(RED, 3),
                ..Edges::all(line(RED, 0))
            },
            radius: 10,
            ..Style::NONE
        };
        let mut canvas = Canvas::new(40, 40);
        canvas.draw_box((0, 0, 40, 40), &style);
        assert_eq!(canvas.at(20, 38), argb(RED));
        assert_eq!(canvas.at(20, 36), argb(WHITE));
        // No ring up the sides of the corner.
        assert_eq!(canvas.at(1, 32), argb(WHITE));
        assert_eq!(canvas.at(0, 39), 0, "and the corner is still cut");
        assert_ne!(canvas.at(5, 38), 0, "with the line clipped to it");
    }

    #[test]
    fn rounding_is_limited_by_the_size_of_the_box() {
        let mut canvas = Canvas::new(40, 10);
        canvas.draw_box((0, 0, 40, 10), &boxed(WHITE, RED, 0, 30));
        // A pill: rounded as far as half its height and no further.
        assert_eq!(canvas.at(0, 0), 0);
        assert_eq!(canvas.at(5, 0), argb(WHITE));
        assert_eq!(canvas.at(0, 5), argb(WHITE));
        assert_eq!(canvas.at(20, 5), argb(WHITE));
    }

    /// A picture of one colour.
    fn plain(width: u32, height: u32, color: Color) -> Image {
        let pixel = [color.red, color.green, color.blue, color.alpha];
        let count = usize::try_from(width * height).unwrap();
        Image::new(width, height, pixel.repeat(count)).unwrap()
    }

    #[test]
    fn a_picture_needs_a_size_and_four_bytes_for_each_pixel() {
        assert_eq!(plain(3, 2, RED).size(), (3, 2));
        assert_eq!(Image::new(2, 2, vec![0; 15]), None);
        assert_eq!(Image::new(2, 2, vec![0; 17]), None);
        assert_eq!(Image::new(0, 2, Vec::new()), None);
        assert_eq!(Image::new(2, 0, Vec::new()), None);
    }

    #[test]
    fn a_picture_can_be_made_all_one_colour_keeping_its_shape() {
        let pixels = [
            [0, 0, 0, 255],
            [9, 9, 9, 128],
            [200, 0, 0, 0],
            [1, 2, 3, 255],
        ];
        let shape = Image::new(2, 2, pixels.concat()).unwrap();
        let red = shape.tinted(RED);
        assert_eq!(red.size(), (2, 2));
        let expected = [
            [255, 0, 0, 255],
            [255, 0, 0, 128],
            [255, 0, 0, 0],
            [255, 0, 0, 255],
        ];
        assert_eq!(red, Image::new(2, 2, expected.concat()).unwrap());
        // A colour that is itself see-through makes it more so.
        let faint = shape.tinted(Color::rgba(0, 0, 255, 128));
        let expected = [
            [0, 0, 255, 128],
            [0, 0, 255, 64],
            [0, 0, 255, 0],
            [0, 0, 255, 128],
        ];
        assert_eq!(faint, Image::new(2, 2, expected.concat()).unwrap());
    }

    #[test]
    fn a_picture_is_drawn_in_the_middle_of_its_room_with_its_shape_kept() {
        let mut canvas = Canvas::new(40, 40);
        // Twice as wide as tall, in a square: it fills the width.
        canvas.draw_image((10, 10, 20, 20), &plain(8, 4, RED));
        assert_eq!(canvas.at(10, 20), argb(RED));
        assert_eq!(canvas.at(29, 20), argb(RED));
        assert_eq!(canvas.at(20, 15), argb(RED));
        assert_eq!(canvas.at(20, 24), argb(RED));
        assert_eq!(canvas.at(20, 14), 0, "above it");
        assert_eq!(canvas.at(20, 25), 0, "below it");
        assert_eq!(canvas.at(9, 20), 0);
        assert_eq!(canvas.at(30, 20), 0);
        // Twice as tall as wide: it fills the height.
        let mut canvas = Canvas::new(40, 40);
        canvas.draw_image((10, 10, 20, 20), &plain(4, 8, BLUE));
        assert_eq!(canvas.at(20, 10), argb(BLUE));
        assert_eq!(canvas.at(20, 29), argb(BLUE));
        assert_eq!(canvas.at(14, 20), 0);
        assert_eq!(canvas.at(15, 20), argb(BLUE));
        assert_eq!(canvas.at(24, 20), argb(BLUE));
        assert_eq!(canvas.at(25, 20), 0);
    }

    #[test]
    fn a_picture_drawn_smaller_is_smoothed_and_one_drawn_larger_is_not() {
        // A chequerboard of red and blue.
        let mut pixels = Vec::new();
        for y in 0..8_u32 {
            for x in 0..8_u32 {
                let color = if (x + y) % 2 == 0 { RED } else { BLUE };
                pixels.extend([color.red, color.green, color.blue, 255]);
            }
        }
        let board = Image::new(8, 8, pixels).unwrap();
        let mut small = Canvas::new(4, 4);
        small.draw_image((0, 0, 4, 4), &board);
        assert_eq!(small.at(1, 1), 0xff7f_007f, "half of each");
        let mut large = Canvas::new(32, 32);
        large.draw_image((0, 0, 32, 32), &board);
        assert_eq!(large.at(0, 0), argb(RED));
        assert_eq!(large.at(3, 3), argb(RED));
        assert_eq!(large.at(4, 0), argb(BLUE));
    }

    #[test]
    fn a_picture_is_as_see_through_as_its_pixels_and_clear_ones_do_not_tint() {
        let mut canvas = Canvas::new(10, 10);
        canvas.fill((0, 0, 10, 10), BLUE);
        canvas.draw_image((0, 0, 10, 10), &plain(2, 2, Color::rgba(255, 0, 0, 0)));
        assert_eq!(canvas.at(5, 5), argb(BLUE), "a clear picture shows nothing");
        // One opaque red pixel and three clear green ones, drawn as one.
        let pixels = [
            [255, 0, 0, 255],
            [0, 255, 0, 0],
            [0, 255, 0, 0],
            [0, 255, 0, 0],
        ];
        let mixed = Image::new(2, 2, pixels.concat()).unwrap();
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_image((0, 0, 1, 1), &mixed);
        // A quarter red, and no green at all.
        assert_eq!(canvas.at(0, 0), 0x3f3f_0000);
    }

    #[test]
    fn a_picture_is_clipped_to_the_canvas_and_scaled_with_it() {
        let mut canvas = Canvas::scaled(10, 10, Scale::new(2.0));
        canvas.draw_image((5, 5, 10, 10), &plain(4, 4, RED));
        assert_eq!(canvas.at(9, 9), 0);
        assert_eq!(canvas.at(10, 10), argb(RED));
        assert_eq!(canvas.at(19, 19), argb(RED));
        canvas.draw_image((-50, -50, 10, 10), &plain(4, 4, BLUE));
        assert_eq!(canvas.at(0, 0), 0);
    }

    /// A picture of a row of pixels, each of a colour.
    fn row(colors: &[Color]) -> Image {
        let pixels = colors
            .iter()
            .flat_map(|c| [c.red, c.green, c.blue, c.alpha]);
        let width = u32::try_from(colors.len()).unwrap();
        Image::new(width, 1, pixels.collect()).unwrap()
    }

    /// The red and blue of the pixels along the top of a canvas.
    fn reds_and_blues(canvas: &Canvas) -> Vec<(u32, u32)> {
        let pixels = (0..canvas.width()).map(|x| canvas.at(x, 0));
        pixels.map(|p| ((p >> 16) & 0xff, p & 0xff)).collect()
    }

    #[test]
    fn a_picture_drawn_larger_is_blended_between_its_pixels() {
        let mut canvas = Canvas::new(4, 1);
        canvas.draw_stretched((0, 0, 4, 1), &row(&[RED, BLUE]));
        // The middle of each pixel drawn is a quarter or three quarters of
        // the way between the middles of the two; the outer two are each
        // past the middle of one, and are that one alone.
        assert_eq!(
            reds_and_blues(&canvas),
            [(255, 0), (191, 63), (63, 191), (0, 255)]
        );
        // Drawn its own size, it is as it is.
        let mut same = Canvas::new(2, 1);
        same.draw_stretched((0, 0, 2, 1), &row(&[RED, BLUE]));
        assert_eq!(reds_and_blues(&same), [(255, 0), (0, 255)]);
    }

    #[test]
    fn a_picture_drawn_smaller_is_the_average_of_what_each_pixel_covers() {
        let mut canvas = Canvas::new(2, 1);
        canvas.draw_stretched((0, 0, 2, 1), &row(&[RED, RED, BLUE, BLUE]));
        assert_eq!(reds_and_blues(&canvas), [(255, 0), (0, 255)]);
        // Three to two: the first covers one and a half of the picture's
        // pixels, a whole red one and half a blue one.
        let mut canvas = Canvas::new(2, 1);
        canvas.draw_stretched((0, 0, 2, 1), &row(&[RED, BLUE, BLUE]));
        assert_eq!(reds_and_blues(&canvas), [(170, 85), (0, 255)]);
        // Down the other side as along this one.
        let column = Image::new(1, 2, [[255, 0, 0, 255], [0, 0, 255, 255]].concat()).unwrap();
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_stretched((0, 0, 1, 1), &column);
        assert_eq!(canvas.at(0, 0), 0xff7f_007f);
    }

    #[test]
    fn a_picture_stretched_beyond_the_canvas_is_drawn_where_it_is_on_it() {
        let picture = row(&[RED, BLUE, RED, BLUE]);
        let mut whole = Canvas::new(8, 1);
        whole.draw_stretched((0, 0, 8, 1), &picture);
        // The same picture as large, half of it off to the left.
        let mut half = Canvas::new(4, 1);
        half.draw_stretched((-4, 0, 8, 1), &picture);
        assert_eq!(reds_and_blues(&half), reds_and_blues(&whole)[4..]);
        // Nothing of it on the canvas, nothing drawn.
        let mut none = Canvas::new(4, 4);
        none.draw_stretched((4, 0, 8, 1), &picture);
        none.draw_stretched((0, -2, 4, 2), &picture);
        assert!(none.bytes().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_clear_pixel_of_a_stretched_picture_does_not_tint_its_neighbours() {
        let clear_green = Color::rgba(0, 255, 0, 0);
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_stretched((0, 0, 1, 1), &row(&[RED, clear_green]));
        // Half as opaque, and red.
        assert_eq!(canvas.at(0, 0), 0x7f7f_0000);
        let mut canvas = Canvas::new(3, 1);
        canvas.draw_stretched((0, 0, 3, 1), &row(&[clear_green]));
        assert_eq!(canvas.at(1, 0), 0);
    }

    #[test]
    fn a_picture_drawn_as_it_is_has_a_pixel_to_each_and_goes_over_the_edge() {
        let pixels = [
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 255, 0, 0],
            [0, 0, 255, 128],
        ];
        let square = Image::new(2, 2, pixels.concat()).unwrap();
        let mut canvas = Canvas::new(2, 2);
        canvas.fill((0, 0, 2, 2), Color::rgb(0, 0, 0));
        canvas.draw_unscaled((0, 0), &square);
        assert_eq!(canvas.at(0, 0), argb(RED));
        assert_eq!(canvas.at(1, 0), argb(BLUE));
        assert_eq!(
            canvas.at(0, 1),
            0xff00_0000,
            "a clear pixel leaves what was there"
        );
        assert_eq!(canvas.at(1, 1), 0xff00_0080, "half clear, half drawn");
        // Its top left corner off the canvas: only its last pixel is on.
        let mut canvas = Canvas::new(2, 2);
        canvas.draw_unscaled((-1, -1), &square);
        assert_eq!(canvas.at(0, 0), 0x8000_0080);
        assert_eq!(canvas.at(1, 1), 0);
    }

    #[test]
    fn what_a_box_holds_starts_inside_its_edges() {
        let edges = Edges {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        };
        assert_eq!(inset((10, 20, 100, 50), edges), (14, 21, 94, 46));
        assert_eq!(inset((10, 20, 100, 50), Edges::NONE), (10, 20, 100, 50));
        assert_eq!(
            inset((0, 0, 5, 3), Edges::all(4)),
            (4, 4, 0, 0),
            "never less than nothing"
        );
    }

    #[test]
    fn text_can_be_bold_or_slanted() {
        let mut text = Text::new();
        for (weight, italic) in [(700, false), (400, true), (900, true)] {
            let font = Font {
                weight,
                italic,
                ..Font::default()
            };
            assert!(text.measure("heavy", &font) > 0);
            let mut canvas = Canvas::new(120, 24);
            text.draw(&mut canvas, "heavy", (0, 0, 120, 24), RED, &font);
            assert!(inked(&canvas, 0..120, 0) > 0);
        }
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
