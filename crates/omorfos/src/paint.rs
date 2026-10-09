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

/// Space between a tab's edge and its title, in pixels.
const PADDING: u32 = 10;

/// The thickness of the line marking the shown tab, in pixels.
const UNDERLINE: u32 = 2;

/// The width set aside on a menu row for the arrow that marks a submenu.
const ARROW: u32 = 14;

/// A rectangle on a canvas: left, top, width, height.
type Area = (i32, i32, u32, u32);

/// A rectangle of pixels being drawn, initially transparent.
///
/// Pixels are stored as ARGB with premultiplied alpha, in the byte order
/// renderers call `Argb8888`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let bytes = usize::try_from(u64::from(width) * u64::from(height) * 4).unwrap_or(0);
        Self {
            width,
            height,
            pixels: vec![0; bytes],
        }
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

    /// Draws `color` over the pixel at `(x, y)`, as opaque as `coverage`
    /// says (`255` for fully). Does nothing off the canvas.
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
        let (left, top, width, height) = area;
        let float = |length: u32| f32::from(u16::try_from(length).unwrap_or(u16::MAX));
        // Font sizes are in points, at the conventional 96 pixels per inch.
        let size = float(font.size.get()) * 96.0 / 72.0;
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
                for row in 0..glyph_height.cast_signed() {
                    for column in 0..glyph_width.cast_signed() {
                        canvas.blend(left + x + column, top + y + row, ink_color, ink.a());
                    }
                }
            },
        );
    }
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
    let mut canvas = Canvas::new(width, height);

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
) -> Canvas {
    let bounds = menu.panel_rect(panel);
    let (width, height) = (bounds.size.width, bounds.size.height);
    let colors = &theme.colors;
    let mut canvas = Canvas::new(width, height);
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
        let plain = menu_panel(&mut text, &theme, &menu, &menu.panels()[0]);
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
        let lit = menu_panel(&mut text, &theme, &menu, &menu.panels()[0]);
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
        let canvas = titlebar(&mut text, &theme, &frame(1), &["Terminal"], 0, true);

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
        let canvas = titlebar(&mut text, &theme, &frame(1), &["Terminal"], 0, false);
        assert_eq!(canvas.at(200, 0), argb(theme.colors.unfocused.border));
        assert_eq!(canvas.at(300, 14), argb(theme.colors.unfocused.titlebar));
    }

    #[test]
    fn tabs_are_separated_and_the_shown_one_is_marked() {
        let theme = Theme::default();
        let colors = &theme.colors;
        let mut text = Text::new();
        let titles = ["one", "two"];
        let focused = titlebar(&mut text, &theme, &frame(2), &titles, 1, true);

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

        let unfocused = titlebar(&mut text, &theme, &frame(2), &titles, 0, false);
        assert_eq!(
            unfocused.at(150, 27),
            argb(colors.unfocused.border),
            "marked without the accent"
        );
        assert_eq!(unfocused.at(350, 27), argb(colors.unfocused.titlebar));
    }
}
