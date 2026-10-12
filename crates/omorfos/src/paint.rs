//! Drawing the compositor's own pixels: titlebars and, later, menus.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which is then uploaded
//! as a texture. That keeps drawing independent of the renderer, so the same
//! code runs on a GPU and in tests, and it only happens when what is drawn
//! changes, not every frame.

use omorfos_config::{Color, Edges, Line, Part, State, States, Theme};
use omorfos_core::{Decoration, Item, Menu, Panel};
use omorfos_ui::inset;
pub(crate) use omorfos_ui::{Canvas, Text};

use crate::Scale;

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
    let states = States::NONE.when(State::Focused, focused);
    let frame = theme.style(Part::Window, states);
    // A frame's border is the same all round; the top stands for it.
    let outline = frame.border.top;
    let border = outline.width;
    let edge = border.cast_signed();
    let mut canvas = Canvas::scaled(width, height, scale);

    canvas.fill((0, 0, width, height), outline.color);
    let inner_height = height.saturating_sub(border);
    canvas.fill(
        (edge, edge, width.saturating_sub(border * 2), inner_height),
        theme.style(Part::Titlebar, states).background,
    );

    let tabbed = titles.len() > 1;
    for (index, title) in titles.iter().enumerate() {
        let tab = decoration.tab(index);
        let left = tab.origin.x - bar.origin.x;
        let tab_states = states.when(State::Active, index == active);
        let mut style = theme.style(Part::Tab, tab_states);
        // The line on a tab's left is the line between it and the tab
        // before, which the first has none of. A title alone in its
        // titlebar is not set apart as a tab at all.
        if index == 0 {
            style.border.left.width = 0;
        }
        if !tabbed {
            style.border = Edges::all(Line::NONE);
        }
        let area = (left, edge, tab.size.width, inner_height);
        if tabbed {
            canvas.draw_box(area, &style);
        }
        let font = theme.font(Part::Tab, tab_states);
        text.draw(
            &mut canvas,
            title,
            inset(area, style.inset()),
            style.color,
            font,
        );
    }

    canvas.round_top_corners(frame.radius);
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
    let style = theme.style(Part::Menu, States::NONE);
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    // Rows run from one side of the menu to the other, inside its border.
    let (edge, _, inner, _) = inset(whole, style.border.widths());

    for (index, (item, row)) in panel.items().iter().zip(menu.rows(panel)).enumerate() {
        let top = row.origin.y - bounds.origin.y;
        let area = (edge, top, inner, row.size.height);
        let label = match item {
            Item::Separator => {
                // As long as the labels of the rows around it.
                let plain = theme.style(Part::MenuItem, States::NONE);
                let (left, _, room, _) = inset(area, plain.inset());
                let middle = top + (row.size.height / 2).cast_signed();
                let line = theme.style(Part::MenuSeparator, States::NONE);
                canvas.fill((left, middle, room, 1), line.color);
                continue;
            }
            Item::Entry { label, .. } | Item::Submenu { label, .. } => label,
        };
        let highlighted = panel.highlighted() == Some(index);
        let states = States::NONE.when(State::Hovered, highlighted);
        let style = theme.style(Part::MenuItem, states);
        let font = theme.font(Part::MenuItem, states);
        canvas.draw_box(area, &style);
        let (left, top, room, tall) = inset(area, style.inset());
        let arrow = if matches!(item, Item::Submenu { .. }) {
            ARROW
        } else {
            0
        };
        let room = room.saturating_sub(arrow);
        text.draw(
            &mut canvas,
            label,
            (left, top, room, tall),
            style.color,
            font,
        );
        if arrow > 0 {
            let left = left.saturating_add_unsigned(room);
            let area = (left, top, ARROW, tall);
            text.draw(&mut canvas, "\u{203a}", area, style.color, font);
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

    let style = theme.style(Part::Notice, States::NONE);
    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = rows.saturating_mul(NOTICE_ROW);
    let height = height.saturating_add(style.inset().down());
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left, mut top, room, _) = inset(whole, style.inset());
    for (index, line) in lines.into_iter().enumerate() {
        // The first line is the heading.
        let part = if index == 0 {
            Part::NoticeHeading
        } else {
            Part::Notice
        };
        let ink = theme.style(part, States::NONE).color;
        let font = theme.font(part, States::NONE);
        text.draw(&mut canvas, line, (left, top, room, NOTICE_ROW), ink, font);
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

    /// The colours of a theme, by what the built-in theme uses them for.
    struct Palette {
        accent: Color,
        focused: Tone,
        unfocused: Tone,
    }

    /// The colours of a frame and its titlebar, with focus or without.
    #[derive(Clone, Copy)]
    struct Tone {
        border: Color,
        titlebar: Color,
    }

    fn palette(theme: &Theme) -> Palette {
        let tone = |states| Tone {
            border: theme.style(Part::Window, states).border.top.color,
            titlebar: theme.style(Part::Titlebar, states).background,
        };
        Palette {
            accent: theme.style(Part::NoticeHeading, States::NONE).color,
            focused: tone(States::NONE.with(State::Focused)),
            unfocused: tone(States::NONE),
        }
    }

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

        let background = argb(palette(&theme).focused.titlebar);
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
        let colors = palette(&theme);
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
        let colors = palette(&theme);
        let mut text = Text::new();
        let problems = vec![
            "theme.css:2:13: `nope` is not a color".to_owned(),
            "keys.yaml: no such key".to_owned(),
        ];

        let canvas = notice(&mut text, &theme, &problems, 400, Scale::ONE);

        // A heading and two problems, 22 pixels each, inside the padding
        // and the border.
        assert_eq!((canvas.width(), canvas.height()), (400, 88));
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
        assert_eq!(exactly.height(), 7 * 22 + 22, "a heading and six problems");
        assert_eq!(
            capped.height(),
            8 * 22 + 22,
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
            argb(palette(&theme).focused.border),
            "outline along the top"
        );
        assert_eq!(
            canvas.at(0, 20),
            argb(palette(&theme).focused.border),
            "and down the side"
        );
        assert_eq!(canvas.at(399, 20), argb(palette(&theme).focused.border));
        assert_eq!(canvas.at(300, 14), argb(palette(&theme).focused.titlebar));
        assert_eq!(
            canvas.at(300, 27),
            argb(palette(&theme).focused.titlebar),
            "no underline"
        );
        assert!(
            inked(&canvas, 10..120, argb(palette(&theme).focused.titlebar)) > 20,
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
        assert_eq!(canvas.at(200, 0), argb(palette(&theme).unfocused.border));
        assert_eq!(canvas.at(300, 14), argb(palette(&theme).unfocused.titlebar));
    }

    #[test]
    fn tabs_are_separated_and_the_shown_one_is_marked() {
        let theme = Theme::default();
        let colors = palette(&theme);
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
