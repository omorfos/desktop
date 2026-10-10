//! Drawing the compositor's own pixels: titlebars and, later, menus.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which is then uploaded
//! as a texture. That keeps drawing independent of the renderer, so the same
//! code runs on a GPU and in tests, and it only happens when what is drawn
//! changes, not every frame.

use omorfos_config::{Color, Theme};
use omorfos_core::{Decoration, Item, Menu, Panel};
pub(crate) use omorfos_ui::{Canvas, Text};

use crate::Scale;

/// Space between a tab's edge and its title, in pixels.
const PADDING: u32 = 10;

/// The thickness of the line marking the shown tab, in pixels.
const UNDERLINE: u32 = 2;

/// The width set aside on a menu row for the arrow that marks a submenu.
const ARROW: u32 = 14;

/// The height of the pointer arrow, in pixels.
const POINTER: u32 = 16;

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
    canvas.quarter_ring(color, border, mirrored);
    canvas
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

    /// The number of pixels in a row range that are not exactly `background`.
    fn inked(canvas: &Canvas, columns: std::ops::Range<u32>, background: u32) -> usize {
        (0..canvas.height())
            .flat_map(|y| columns.clone().map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) != background)
            .count()
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
