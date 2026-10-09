//! Drawing the compositor's own pixels: titlebars and, later, menus.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which is then uploaded
//! as a texture. That keeps drawing independent of the renderer, so the same
//! code runs on a GPU and in tests, and it only happens when what is drawn
//! changes, not every frame.

use cosmic_text::{
    Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    SwashCache, Wrap,
};
use omorfos_config::{Color, Font, Theme};
use omorfos_core::{Decoration, Item, Menu, Panel};

use crate::Scale;

/// Space between a tab's edge and its title, in pixels.
const PADDING: u32 = 10;

/// The thickness of the line marking the shown tab, in pixels.
const UNDERLINE: u32 = 2;

/// The width set aside on a menu row for the arrow that marks a submenu.
const ARROW: u32 = 14;

/// The height of the pointer arrow, in pixels.
const POINTER: u32 = 16;

/// A rectangle on a canvas: left, top, width, height, in units of layout.
type Area = (i32, i32, u32, u32);

/// A rectangle of pixels being drawn, initially transparent.
///
/// What is drawn is described in units of layout, and the canvas has as
/// many pixels to a unit as its scale says, so the same drawing comes out
/// sharp on any screen.
///
/// Pixels are stored as ARGB with premultiplied alpha, in the byte order
/// renderers call `Argb8888`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
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
    #[cfg(test)]
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self::scaled(width, height, Scale::ONE)
    }

    /// A canvas `width` by `height` units, with `scale` pixels to a unit.
    pub(crate) fn scaled(width: u32, height: u32, scale: Scale) -> Self {
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
    pub(crate) const fn units(&self) -> (u32, u32) {
        self.units
    }

    pub(crate) const fn width(&self) -> u32 {
        self.width
    }

    pub(crate) const fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, row by row from the top.
    pub(crate) fn bytes(&self) -> &[u8] {
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
    pub(crate) fn blend(&mut self, x: i32, y: i32, color: Color, coverage: u8) {
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
    pub(crate) fn fill(&mut self, (x, y, width, height): Area, color: Color) {
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
    pub(crate) fn round_top_corners(&mut self, radius: u32) {
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

    /// The pixel at `(x, y)` as `0xAARRGGBB`.
    #[cfg(test)]
    pub(crate) fn at(&self, x: u32, y: u32) -> u32 {
        let offset = usize::try_from((y * self.width + x) * 4).unwrap();
        u32::from_le_bytes(self.pixels[offset..offset + 4].try_into().unwrap())
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

/// Paints the border where it turns a bottom corner of a frame: a quarter
/// ring `radius` across and `border` thick, smooth at both edges. It is the
/// left-hand corner, or the right-hand one if `mirrored`.
pub(crate) fn corner(
    color: Color,
    radius: u32,
    border: u32,
    mirrored: bool,
    scale: Scale,
) -> Canvas {
    let mut canvas = Canvas::scaled(radius, radius, scale);
    let (_, thickness) = span(scale, 0, border);
    let size = canvas.width().min(canvas.height());
    for y in 0..size {
        for x in 0..size {
            // The ring's centre is the square's top corner on the inner
            // side, so rows are counted up from the bottom.
            let lit = ring_coverage(x, size - 1 - y, size, size.saturating_sub(thickness));
            let column = if mirrored { size - 1 - x } else { x };
            let coverage = u8::try_from(lit * 255 / SAMPLES).unwrap_or(u8::MAX);
            canvas.blend(column.cast_signed(), y.cast_signed(), color, coverage);
        }
    }
    canvas
}

/// Lays out and rasterises text. Holds the system's fonts, so it is created
/// once and kept.
pub(crate) struct Text {
    fonts: FontSystem,
    glyphs: SwashCache,
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Text").finish_non_exhaustive()
    }
}

impl Text {
    /// Loads the fonts installed on the system.
    pub(crate) fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            glyphs: SwashCache::new(),
        }
    }

    /// Draws one line of `text` in `area`, vertically centred and cut short
    /// with an ellipsis if it does not fit.
    pub(crate) fn draw(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        area: Area,
        color: Color,
        font: &Font,
    ) {
        let (left, width) = span(canvas.scale, area.0, area.2);
        let (top, height) = span(canvas.scale, area.1, area.3);
        let float = |length: u32| f32::from(u16::try_from(length).unwrap_or(u16::MAX));
        // Font sizes are in points, at the conventional 96 units per inch.
        // A scale is a few quarters, which an `f32` holds exactly.
        #[expect(clippy::cast_possible_truncation, reason = "a scale is at most four")]
        let scale = canvas.scale.factor() as f32;
        let size = float(font.size.get()) * 96.0 / 72.0 * scale;
        let family = match font.family.as_str() {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };
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

/// The part of the span `start..start + length` that lies within
/// `0..limit`, which is empty if none of it does.
fn visible(start: i32, length: u32, limit: u32) -> std::ops::Range<i32> {
    let clamp = |position: i64| i32::try_from(position.clamp(0, i64::from(limit))).unwrap_or(0);
    let start = i64::from(start);
    clamp(start)..clamp(start + i64::from(length))
}

/// Paints a frame's titlebar: one tab per window with its title, and the
/// outline that continues the border around the top.
///
/// `titles` are the tabs' titles in order, `active` is the tab being shown,
/// and `focused` is whether the frame has keyboard focus.
pub(crate) fn titlebar(
    text: &mut Text,
    theme: &Theme,
    decoration: &Decoration,
    titles: &[&str],
    active: usize,
    focused: bool,
    scale: Scale,
) -> Canvas {
    let bar = decoration.titlebar();
    let (width, height) = (bar.size.width, bar.size.height);
    let colors = &theme.colors;
    let palette = if focused {
        colors.focused
    } else {
        colors.unfocused
    };
    let border = theme.border.width.get();
    let edge = border.cast_signed();
    let mut canvas = Canvas::scaled(width, height, scale);

    canvas.fill((0, 0, width, height), palette.border);
    let inner_height = height.saturating_sub(border);
    canvas.fill(
        (edge, edge, width.saturating_sub(border * 2), inner_height),
        palette.titlebar,
    );

    let tabbed = titles.len() > 1;
    for (index, title) in titles.iter().enumerate() {
        let tab = decoration.tab(index);
        let left = tab.origin.x - bar.origin.x;
        let shown = index == active;
        if tabbed && !shown {
            canvas.fill(
                (left, edge, tab.size.width, inner_height),
                colors.unfocused.titlebar,
            );
        }
        if tabbed && shown {
            let line = if focused {
                colors.accent
            } else {
                palette.border
            };
            let top = height.saturating_sub(UNDERLINE).cast_signed();
            canvas.fill((left, top, tab.size.width, UNDERLINE), line);
        }
        if index > 0 {
            canvas.fill((left, edge, 1, inner_height), palette.border);
        }
        let ink = if shown {
            palette.text
        } else {
            colors.unfocused.text
        };
        let room = tab.size.width.saturating_sub(PADDING * 2);
        text.draw(
            &mut canvas,
            title,
            (left + PADDING.cast_signed(), 0, room, height),
            ink,
            &theme.font,
        );
    }

    canvas.round_top_corners(theme.border.radius.get());
    canvas
}

/// Paints one panel of an open menu: its rows, the highlight on the row
/// under the pointer, and an arrow on rows that open a submenu.
pub(crate) fn menu_panel<A: Clone>(
    text: &mut Text,
    theme: &Theme,
    menu: &Menu<A>,
    panel: &Panel<A>,
    scale: Scale,
) -> Canvas {
    let bounds = menu.panel_rect(panel);
    let (width, height) = (bounds.size.width, bounds.size.height);
    let colors = &theme.colors;
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.fill((0, 0, width, height), colors.unfocused.border);
    canvas.fill(
        (1, 1, width.saturating_sub(2), height.saturating_sub(2)),
        colors.focused.titlebar,
    );

    let padding = PADDING.cast_signed();
    let room = width.saturating_sub(PADDING * 2);
    for (index, (item, row)) in panel.items().iter().zip(menu.rows(panel)).enumerate() {
        let top = row.origin.y - bounds.origin.y;
        let label = match item {
            Item::Separator => {
                let middle = top + (row.size.height / 2).cast_signed();
                canvas.fill((padding, middle, room, 1), colors.unfocused.border);
                continue;
            }
            Item::Entry { label, .. } | Item::Submenu { label, .. } => label,
        };
        let highlighted = panel.highlighted() == Some(index);
        if highlighted {
            canvas.fill(
                (1, top, width.saturating_sub(2), row.size.height),
                colors.accent,
            );
        }
        let ink = if highlighted {
            colors.background
        } else {
            colors.focused.text
        };
        let arrow = if matches!(item, Item::Submenu { .. }) {
            ARROW
        } else {
            0
        };
        let area = (padding, top, room.saturating_sub(arrow), row.size.height);
        text.draw(&mut canvas, label, area, ink, &theme.font);
        if arrow > 0 {
            let left = (width - PADDING - ARROW).cast_signed();
            text.draw(
                &mut canvas,
                "\u{203a}",
                (left, top, ARROW, row.size.height),
                ink,
                &theme.font,
            );
        }
    }
    canvas
}

/// The height of one line of the problem notice, in pixels.
const NOTICE_ROW: u32 = 22;

/// The most problems the notice lists before summarising the rest.
const NOTICE_LINES: usize = 6;

/// Paints the notice that tells the user something is wrong: a heading and
/// one line per problem, each cut short if it does not fit.
pub(crate) fn notice(
    text: &mut Text,
    theme: &Theme,
    problems: &[String],
    width: u32,
    scale: Scale,
) -> Canvas {
    let hidden = problems.len().saturating_sub(NOTICE_LINES);
    let heading = "Problems (click to dismiss)".to_owned();
    let more = (hidden > 0).then(|| format!("\u{2026} and {hidden} more"));
    let lines: Vec<&String> = std::iter::once(&heading)
        .chain(problems.iter().take(NOTICE_LINES))
        .chain(more.iter())
        .collect();

    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = rows.saturating_mul(NOTICE_ROW).saturating_add(PADDING * 2);
    let colors = &theme.colors;
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.fill((0, 0, width, height), colors.accent);
    canvas.fill(
        (1, 1, width.saturating_sub(2), height.saturating_sub(2)),
        colors.focused.titlebar,
    );
    let room = width.saturating_sub(PADDING * 2);
    let mut top = PADDING.cast_signed();
    for (index, line) in lines.into_iter().enumerate() {
        let ink = if index == 0 {
            colors.accent
        } else {
            colors.focused.text
        };
        text.draw(
            &mut canvas,
            line,
            (PADDING.cast_signed(), top, room, NOTICE_ROW),
            ink,
            &theme.font,
        );
        top += NOTICE_ROW.cast_signed();
    }
    canvas
}

/// Paints the pointer: a white arrow with a dark outline, whose tip is the
/// top-left pixel, so that it shows on any background.
pub(crate) fn pointer(scale: Scale) -> Canvas {
    let outline = Color::rgb(0x14, 0x16, 0x1b);
    let inside = Color::rgb(0xff, 0xff, 0xff);
    // The arrow widens by a pixel a row down its left edge, then closes
    // twice as fast.
    let width = |row: u32| (row + 1).min((POINTER - row) * 2);
    let mut canvas = Canvas::scaled(POINTER * 3 / 4, POINTER, scale);
    for row in 0..POINTER {
        canvas.fill((0, row.cast_signed(), width(row), 1), outline);
    }
    for row in 2..POINTER - 2 {
        let inner = width(row).min(width(row + 1)).saturating_sub(2);
        canvas.fill((1, row.cast_signed(), inner, 1), inside);
    }
    canvas
}

#[cfg(test)]
mod tests {
    use omorfos_core::Metrics as FrameMetrics;
    use omorfos_core::geometry::{Point, Rect, Size};

    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    #[test]
    fn the_pointer_is_a_light_arrow_with_a_dark_edge_and_its_tip_at_the_corner() {
        let arrow = pointer(Scale::ONE);
        let dark = 0xff14_161b;
        assert_eq!(arrow.at(0, 0), dark, "the tip");
        assert_eq!(arrow.at(0, 8), dark, "its left edge");
        assert_eq!(arrow.at(2, 8), 0xffff_ffff, "its inside");
        assert_eq!(arrow.at(8, 8), dark, "its slanted edge");
        assert_eq!(arrow.at(9, 8), 0, "clear beyond it");
        assert_eq!(arrow.at(5, 2), 0, "clear above the slant");
        assert_eq!(arrow.at(1, 15), dark, "closed at the bottom");
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
    fn text_and_corners_grow_with_the_scale() {
        let theme = Theme::default();
        let mut text = Text::new();
        let mut paint =
            |scale| titlebar(&mut text, &theme, &frame(1), &["Terminal"], 0, true, scale);
        let plain = paint(Scale::ONE);
        let doubled = paint(Scale::new(2.0));
        assert_eq!((doubled.width(), doubled.height()), (800, 56));
        assert_eq!(doubled.units(), (400, 28));

        let background = argb(theme.colors.focused.titlebar);
        let ink = |canvas: &Canvas, columns| inked(canvas, columns, background);
        assert!(
            ink(&doubled, 20..240) > ink(&plain, 10..120) * 3,
            "letters twice as tall and twice as wide cover about four times as much"
        );
        assert_eq!(plain.at(1, 1), 0, "inside the rounded corner");
        assert_eq!(doubled.at(3, 3), 0, "which is twice the size");
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

    #[test]
    fn a_menu_panel_shows_its_rows_with_the_hovered_one_highlighted() {
        use omorfos_core::MenuMetrics;

        let theme = Theme::default();
        let colors = &theme.colors;
        let mut text = Text::new();
        let items = vec![
            Item::Entry {
                label: "Terminal".to_owned(),
                action: (),
            },
            Item::Separator,
            Item::Submenu {
                label: "Session".to_owned(),
                items: Vec::new(),
            },
        ];
        let metrics = MenuMetrics {
            width: 200,
            row: 28,
            separator: 9,
            padding: 4,
        };
        let area = Rect::new(Point::new(0, 0), Size::new(1000, 800));
        let mut menu = Menu::open(items, Point::new(300, 200), area, metrics);

        // Rows: Terminal at 4..32, the separator at 32..41, Session at 41..69.
        let plain = menu_panel(&mut text, &theme, &menu, &menu.panels()[0], Scale::ONE);
        assert_eq!((plain.width(), plain.height()), (200, 73));
        assert_eq!(plain.at(0, 30), argb(colors.unfocused.border), "outlined");
        assert_eq!(plain.at(190, 10), argb(colors.focused.titlebar));
        assert_eq!(
            plain.at(100, 36),
            argb(colors.unfocused.border),
            "the separator's line"
        );
        assert!(
            inked(&plain, 10..100, argb(colors.focused.titlebar)) > 40,
            "labels"
        );
        let arrow = (176..190).flat_map(|x| (41..69).map(move |y| (x, y)));
        let arrow_ink = arrow
            .filter(|(x, y)| plain.at(*x, *y) != argb(colors.focused.titlebar))
            .count();
        assert!(arrow_ink > 3, "the submenu row has an arrow");

        menu.hover(Point::new(350, 215));
        let lit = menu_panel(&mut text, &theme, &menu, &menu.panels()[0], Scale::ONE);
        assert_eq!(
            lit.at(190, 10),
            argb(colors.accent),
            "the hovered row is highlighted"
        );
        assert_eq!(
            lit.at(190, 50),
            argb(colors.focused.titlebar),
            "the others are not"
        );
    }

    #[test]
    fn a_notice_lists_each_problem_under_a_heading() {
        let theme = Theme::default();
        let colors = &theme.colors;
        let mut text = Text::new();
        let problems = vec![
            "theme.yaml:2:11: invalid color".to_owned(),
            "keys.yaml: no such key".to_owned(),
        ];

        let canvas = notice(&mut text, &theme, &problems, 400, Scale::ONE);

        // A heading and two problems, 22 pixels each, plus padding.
        assert_eq!((canvas.width(), canvas.height()), (400, 86));
        assert_eq!(
            canvas.at(0, 40),
            argb(colors.accent),
            "outlined in the accent colour"
        );
        assert_eq!(canvas.at(395, 40), argb(colors.focused.titlebar));
        let row = |top: u32| (10..300).flat_map(move |x| (top..top + 22).map(move |y| (x, y)));
        let ink = |top: u32| {
            row(top)
                .filter(|(x, y)| canvas.at(*x, *y) != argb(colors.focused.titlebar))
                .count()
        };
        assert!(
            ink(10) > 30 && ink(32) > 30 && ink(54) > 30,
            "three lines of text"
        );
    }

    #[test]
    fn a_long_list_of_problems_is_summarised() {
        let mut text = Text::new();
        let few: Vec<String> = (0..6).map(|n| format!("problem {n}")).collect();
        let many: Vec<String> = (0..40).map(|n| format!("problem {n}")).collect();
        let exactly = notice(&mut text, &Theme::default(), &few, 400, Scale::ONE);
        let capped = notice(&mut text, &Theme::default(), &many, 400, Scale::ONE);
        assert_eq!(exactly.height(), 7 * 22 + 20, "a heading and six problems");
        assert_eq!(
            capped.height(),
            8 * 22 + 20,
            "the same, and a line counting the rest"
        );
    }

    const METRICS: FrameMetrics = FrameMetrics {
        border: 1,
        titlebar: 28,
    };

    fn frame(tabs: usize) -> Decoration {
        let outer = Rect::new(Point::new(300, 200), Size::new(400, 300));
        Decoration::new(outer, METRICS, tabs)
    }

    #[test]
    fn a_titlebar_spans_the_frame_with_an_outline_and_rounded_corners() {
        let theme = Theme::default();
        let mut text = Text::new();
        let canvas = titlebar(
            &mut text,
            &theme,
            &frame(1),
            &["Terminal"],
            0,
            true,
            Scale::ONE,
        );

        assert_eq!((canvas.width(), canvas.height()), (400, 28));
        assert_eq!(canvas.at(0, 0), 0, "rounded");
        assert_eq!(
            canvas.at(200, 0),
            argb(theme.colors.focused.border),
            "outline along the top"
        );
        assert_eq!(
            canvas.at(0, 20),
            argb(theme.colors.focused.border),
            "and down the side"
        );
        assert_eq!(canvas.at(399, 20), argb(theme.colors.focused.border));
        assert_eq!(canvas.at(300, 14), argb(theme.colors.focused.titlebar));
        assert_eq!(
            canvas.at(300, 27),
            argb(theme.colors.focused.titlebar),
            "no underline"
        );
        assert!(
            inked(&canvas, 10..120, argb(theme.colors.focused.titlebar)) > 20,
            "the title"
        );
    }

    #[test]
    fn an_unfocused_titlebar_uses_the_unfocused_colours() {
        let theme = Theme::default();
        let mut text = Text::new();
        let canvas = titlebar(
            &mut text,
            &theme,
            &frame(1),
            &["Terminal"],
            0,
            false,
            Scale::ONE,
        );
        assert_eq!(canvas.at(200, 0), argb(theme.colors.unfocused.border));
        assert_eq!(canvas.at(300, 14), argb(theme.colors.unfocused.titlebar));
    }

    #[test]
    fn tabs_are_separated_and_the_shown_one_is_marked() {
        let theme = Theme::default();
        let colors = &theme.colors;
        let mut text = Text::new();
        let titles = ["one", "two"];
        let focused = titlebar(&mut text, &theme, &frame(2), &titles, 1, true, Scale::ONE);

        // Tab 0 covers columns 0..200, tab 1 covers 200..400.
        assert_eq!(
            focused.at(150, 14),
            argb(colors.unfocused.titlebar),
            "the hidden tab is dimmed"
        );
        assert_eq!(focused.at(350, 14), argb(colors.focused.titlebar));
        assert_eq!(
            focused.at(200, 14),
            argb(colors.focused.border),
            "a line separates the tabs"
        );
        assert_eq!(
            focused.at(350, 27),
            argb(colors.accent),
            "the shown tab is underlined"
        );
        assert_eq!(focused.at(150, 27), argb(colors.unfocused.titlebar));

        let unfocused = titlebar(&mut text, &theme, &frame(2), &titles, 0, false, Scale::ONE);
        assert_eq!(
            unfocused.at(150, 27),
            argb(colors.unfocused.border),
            "marked without the accent"
        );
        assert_eq!(unfocused.at(350, 27), argb(colors.unfocused.titlebar));
    }
}
